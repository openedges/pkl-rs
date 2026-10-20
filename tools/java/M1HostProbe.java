// Development-only Java 0.32 lifecycle observations; never a production Host.
import java.io.IOException;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.FutureTask;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.regex.Pattern;
import org.pkl.core.Evaluator;
import org.pkl.core.EvaluatorBuilder;
import org.pkl.core.ModuleSource;
import org.pkl.core.resource.ResourceReader;

public final class M1HostProbe {
  static Path output;
  interface Action { String run() throws Exception; }
  record Outcome(String type, String value) {}

  static synchronized void event(String name, Object value) {
    System.out.println(name + "\t" + value);
  }
  static Outcome call(String name, Action action) throws IOException {
    try {
      String value = action.run();
      Files.writeString(output.resolve(name + ".result.bin"), value, StandardCharsets.UTF_8);
      event(name, "ok");
      return new Outcome("ok", value);
    } catch (Throwable error) {
      String message = error.getMessage();
      Files.writeString(output.resolve(name + ".error.bin"), message == null ? "" : message, StandardCharsets.UTF_8);
      StringWriter trace = new StringWriter();
      error.printStackTrace(new PrintWriter(trace));
      Files.writeString(output.resolve(name + ".trace.bin"), trace.toString(), StandardCharsets.UTF_8);
      event(name, error.getClass().getName());
      return new Outcome(error.getClass().getName(), message);
    }
  }
  static FutureTask<Outcome> start(String name, Action action) {
    var future = new FutureTask<Outcome>(() -> call(name, action));
    var thread = new Thread(future, "m1-" + name);
    thread.setDaemon(true);
    thread.start();
    return future;
  }
  static ModuleSource source(String suffix, String body) {
    Path path = output.resolve(suffix + ".pkl");
    try { Files.writeString(path, body, StandardCharsets.UTF_8); }
    catch (IOException error) { throw new java.io.UncheckedIOException(error); }
    return ModuleSource.path(path);
  }
  static final class Reader implements ResourceReader {
    final AtomicInteger calls = new AtomicInteger();
    final AtomicInteger interrupted = new AtomicInteger();
    final CountDownLatch entered = new CountDownLatch(1);
    final CountDownLatch release = new CountDownLatch(1);
    boolean gated;
    Evaluator reenter;
    public String getUriScheme() { return "m1"; }
    public boolean hasHierarchicalUris() { return false; }
    public boolean isGlobbable() { return false; }
    public Optional<Object> read(URI uri) throws IOException {
      int count = calls.incrementAndGet();
      entered.countDown();
      if (gated) {
        try {
          if (!release.await(10, TimeUnit.SECONDS)) throw new IOException("M1 callback watchdog expired");
        } catch (InterruptedException error) {
          interrupted.incrementAndGet();
          Thread.currentThread().interrupt();
          throw new IOException("M1 callback interrupted", error);
        }
      }
      if (reenter != null) {
        try {
          Outcome outcome = call("reentrant", () -> reenter.evaluateExpressionString(source("inner", "result = 41"), "result"));
          if (outcome.type().equals("ok")) return Optional.of(outcome.value());
        } catch (IOException error) { throw error; }
      }
      return Optional.of("payload-" + count);
    }
    void awaitEntered() throws Exception {
      if (!entered.await(10, TimeUnit.SECONDS)) throw new IllegalStateException("M1 callback was never entered");
    }
  }
  static Evaluator evaluator(Reader reader, Duration timeout) {
    return EvaluatorBuilder.preconfigured().setColor(false).setModuleCacheDir(null)
        .setEnvironmentVariables(Map.of()).setExternalProperties(Map.of())
        .setAllowedResources(List.of(Pattern.compile("m1:.*")))
        .setResourceReaders(List.of(reader)).setTimeout(timeout).build();
  }
  static String evaluate(Evaluator evaluator, String suffix) {
    return evaluator.evaluateExpressionString(source(suffix, "result = read(\"m1:value\")"), "result");
  }
  public static void main(String[] args) throws Exception {
    output = Path.of(args[1]);
    Files.createDirectories(output);
    String mode = args[0];
    Reader reader = new Reader();
    Duration timeout = mode.equals("timeout") ? Duration.ofSeconds(2) : null;
    try (Evaluator evaluator = evaluator(reader, timeout)) {
      // Initialize the engine before timing begins on the resource-reading call.
      call("warmup", () -> evaluator.evaluateExpressionString(source("warmup", "result = 1"), "result"));
      if (mode.equals("repeat")) {
        call("first", () -> evaluate(evaluator, "repeat"));
        call("repeat", () -> evaluate(evaluator, "repeat"));
        call("second-module", () -> evaluate(evaluator, "second"));
        event("reader-calls", reader.calls.get());
        try (Evaluator fresh = evaluator(reader, null)) {
          call("fresh", () -> evaluate(fresh, "fresh"));
        }
        event("reader-calls-after-fresh", reader.calls.get());
      } else if (mode.equals("reentry")) {
        reader.reenter = evaluator;
        call("outer", () -> evaluate(evaluator, "reentry"));
        event("reader-calls", reader.calls.get());
      } else if (mode.equals("delayed") || mode.equals("timeout") || mode.equals("close")) {
        reader.gated = true;
        FutureTask<Outcome> evaluation = start("evaluation", () -> evaluate(evaluator, mode));
        reader.awaitEntered();
        event("pending-before-release", !evaluation.isDone());
        if (mode.equals("timeout")) {
          Thread.sleep(2250);
          event("pending-after-timeout-window", !evaluation.isDone());
          reader.release.countDown();
          evaluation.get(10, TimeUnit.SECONDS);
          call("after-timeout", () -> evaluator.evaluateExpressionString(source("after-timeout", "result = 2"), "result"));
        } else if (mode.equals("close")) {
          CountDownLatch closing = new CountDownLatch(1);
          FutureTask<Outcome> close = start("close", () -> { closing.countDown(); evaluator.close(); return "closed"; });
          if (!closing.await(10, TimeUnit.SECONDS)) throw new IllegalStateException("close never started");
          Thread.sleep(100);
          event("close-pending-before-release", !close.isDone());
          reader.release.countDown();
          evaluation.get(10, TimeUnit.SECONDS);
          close.get(10, TimeUnit.SECONDS);
          call("after-close", () -> evaluator.evaluateExpressionString(source("after-close", "result = 2"), "result"));
        } else {
          reader.release.countDown();
          evaluation.get(10, TimeUnit.SECONDS);
          call("repeat", () -> evaluate(evaluator, mode));
        }
        event("reader-calls", reader.calls.get());
        event("reader-interrupted", reader.interrupted.get());
      } else {
        throw new IllegalArgumentException("unknown Host mode");
      }
      call("close-once", () -> { evaluator.close(); return "closed"; });
      call("close-twice", () -> { evaluator.close(); return "closed"; });
      call("after-final-close", () -> evaluator.evaluateExpressionString(source("after-final-close", "result = 2"), "result"));
    }
  }
}

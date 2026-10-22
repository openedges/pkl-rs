import java.io.DataOutputStream;
import java.io.IOException;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.ArrayList;
import java.util.Map;
import java.util.Optional;
import java.util.regex.Pattern;
import org.pkl.core.Evaluator;
import org.pkl.core.EvaluatorBuilder;
import org.pkl.core.Logger;
import org.pkl.core.ModuleSource;
import org.pkl.core.PklBugException;
import org.pkl.core.PklException;
import org.pkl.core.StackFrame;
import org.pkl.core.evaluatorSettings.TraceMode;
import org.pkl.core.resource.ResourceReader;

/** Actual-source trace and read callbacks. Every capture is an unmodified payload. */
public final class M2TraceProbe {
  static Path out;
  static Evaluator evaluator;
  static ModuleSource source;
  static String action, loggerCallback, readerCallback;
  static boolean outputBytes;
  static int logs, reads, loggerDepth, readerDepth;

  static void write(String name, String value) throws IOException {
    Files.write(out.resolve(name), value.getBytes(StandardCharsets.UTF_8));
  }

  static void units(String name, String value) throws IOException {
    try (var stream = new DataOutputStream(Files.newOutputStream(out.resolve(name)))) {
      for (int index = 0; index < value.length(); index++) stream.writeChar(value.charAt(index));
    }
  }

  static void capture(String expression, String prefix) {
    capture(expression, prefix, false);
  }

  static void capture(String expression, String prefix, boolean output) {
    byte[] value = new byte[0];
    String error = "", errorClass = "", status = "success\n";
    try {
      value = output ? evaluator.evaluateOutputBytes(source)
          : evaluator.evaluateExpressionString(source, expression).getBytes(StandardCharsets.UTF_8);
    } catch (PklException exception) {
      status = exception instanceof PklBugException ? "oracle-error\n" : "error\n";
      error = exception.getMessage();
      errorClass = exception.getClass().getName();
    } catch (Throwable exception) {
      status = "oracle-error\n";
      error = output && exception.getMessage() != null ? exception.getMessage() : exception.toString();
      errorClass = exception.getClass().getName();
    }
    try {
      Files.write(out.resolve(prefix + ".stdout.bin"), value);
      write(prefix + ".stderr.bin", error);
      write(prefix + ".status.bin", status);
      write(prefix + ".error-class.bin", errorClass);
    } catch (IOException exception) {
      throw new RuntimeException(exception);
    }
    System.out.println("result=" + prefix + " status=" + status.strip());
  }

  static final class Reader implements ResourceReader {
    public String getUriScheme() { return "life"; }
    public boolean hasHierarchicalUris() { return false; }
    public boolean isGlobbable() { return false; }
    public Optional<Object> read(URI uri) {
      int index = reads++;
      System.out.println("read-enter=" + index + " uri=" + uri);
      readerDepth++;
      try {
        if (readerDepth == 1 && !readerCallback.isEmpty()) {
          capture(readerCallback, "read-callback-" + index);
        }
      } finally {
        readerDepth--;
      }
      System.out.println("read-exit=" + index);
      return Optional.of("resource");
    }
  }

  static final class CaptureLogger implements Logger {
    public void trace(String message, StackFrame frame) {
      int index = logs++;
      String prefix = "log-" + index;
      System.out.println("log-enter=" + index + " depth=" + loggerDepth);
      try {
        write(prefix + ".level.bin", "trace");
        write(prefix + ".message.bin", message);
        units(prefix + ".message.code-units.bin", message);
        write(prefix + ".message.length.bin", String.valueOf(message.length()));
        write(prefix + ".uri.bin", frame.getModuleUri());
        units(prefix + ".uri.code-units.bin", frame.getModuleUri());
        write(prefix + ".member.bin", String.valueOf(frame.getMemberName()));
        write(prefix + ".member.null.bin", String.valueOf(frame.getMemberName() == null));
        units(prefix + ".member.code-units.bin", frame.getMemberName() == null ? "" : frame.getMemberName());
        write(prefix + ".position.bin", frame.getStartLine() + ":" + frame.getStartColumn()
            + ":" + frame.getEndLine() + ":" + frame.getEndColumn());
        for (int line = 0; line < frame.getSourceLines().size(); line++) {
          write(prefix + ".source-" + line + ".bin", frame.getSourceLines().get(line));
          units(prefix + ".source-" + line + ".code-units.bin", frame.getSourceLines().get(line));
        }
      } catch (IOException exception) {
        throw new RuntimeException(exception);
      }
      loggerDepth++;
      try {
        if (loggerDepth == 1 && !loggerCallback.isEmpty()) {
          capture(loggerCallback, "log-callback-" + index);
        }
        if (action.equals("throw")) throw new IllegalStateException("stage37-logger-failure");
        if (action.startsWith("close")) evaluator.close();
        if (action.equals("close-reenter")) capture("stable", "post-close-callback-" + index);
      } finally {
        loggerDepth--;
      }
      System.out.println("log-exit=" + index);
    }
  }

  public static void main(String[] args) throws Exception {
    if (args.length != 3) throw new IllegalArgumentException("CASE_DIRECTORY COMPACT|PRETTY ACTION");
    out = Path.of(args[0]);
    var operation = Files.exists(out.resolve("operation.txt"))
        ? Files.readString(out.resolve("operation.txt")) : "expression";
    if (!operation.equals("expression") && !operation.equals("output-bytes"))
      throw new IllegalArgumentException("unsupported operation");
    outputBytes = operation.equals("output-bytes");
    action = args[2];
    loggerCallback = Files.readString(out.resolve("logger-callback.txt"));
    readerCallback = Files.readString(out.resolve("reader-callback.txt"));
    var builder = EvaluatorBuilder.preconfigured().setColor(false).setModuleCacheDir(null)
        .setPowerAssertionsEnabled(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of())
        .setTraceMode(TraceMode.valueOf(args[1]));
    if (outputBytes) {
      // Output defaults really use the preconfigured prop reader. The original
      // expression-only fixtures keep their exact life-only reader profile.
      var allowed = new ArrayList<>(builder.getAllowedResources());
      allowed.add(Pattern.compile("life:.*"));
      builder.setAllowedResources(allowed).addResourceReader(new Reader());
    } else {
      builder.setAllowedResources(List.of(Pattern.compile("life:.*")))
          .setResourceReaders(List.of(new Reader()));
    }
    if (!action.equals("noop")) builder.setLogger(new CaptureLogger());
    try (var eval = builder.build()) {
      evaluator = eval;
      source = ModuleSource.create(URI.create("file:///stage37-trace.pkl"),
          Files.readString(out.resolve("input.pkl")));
      var expressions = Files.readAllLines(out.resolve("expressions.txt"));
      for (int index = 0; index < expressions.size(); index++) {
        capture(expressions.get(index), "outer-" + index, outputBytes);
      }
    }
    System.out.println("logs=" + logs + " reads=" + reads);
  }
}

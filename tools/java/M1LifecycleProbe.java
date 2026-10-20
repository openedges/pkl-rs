// Development-only bounded lifecycle protocol, not a production evaluator adapter.
import java.io.IOException;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.regex.Pattern;
import org.pkl.core.Evaluator;
import org.pkl.core.EvaluatorBuilder;
import org.pkl.core.ModuleSource;
import org.pkl.core.PklException;
import org.pkl.core.PklBugException;
import org.pkl.core.resource.ResourceReader;

public final class M1LifecycleProbe {
  static final class Reader implements ResourceReader {
    int first, second, calls;
    Evaluator evaluator;
    ModuleSource source;
    public String getUriScheme() { return "life"; }
    public boolean hasHierarchicalUris() { return false; }
    public boolean isGlobbable() { return false; }
    public Optional<Object> read(URI uri) throws IOException {
      switch (uri.toString()) {
        case "life:first":
          if (++first == 1) throw new IOException("transient first failure");
          return Optional.of("A");
        case "life:second":
          if (++second == 1) throw new IOException("transient second failure");
          return Optional.of("B");
        case "life:outer":
          calls++;
          String inner = evaluator.evaluateExpressionString(source, "shared");
          System.out.println("inner value=" + inner + " reader_calls=" + calls);
          return Optional.of(inner);
        case "life:shared":
          calls++;
          return Optional.of("S");
        default: throw new IOException("unexpected resource: " + uri);
      }
    }
  }
  static void diagnostic(Path output, String name, Throwable error) throws IOException {
    Files.writeString(output.resolve(name + ".error.bin"), error.getMessage(), StandardCharsets.UTF_8);
    Files.writeString(output.resolve(name + ".error-class.bin"), error.getClass().getName(), StandardCharsets.UTF_8);
  }
  public static void main(String[] args) throws Exception {
    String mode = args[0];
    Path output = Path.of(args[2]);
    Reader reader = new Reader();
    reader.source = ModuleSource.path(Path.of(args[1]));
    try (Evaluator evaluator = EvaluatorBuilder.preconfigured().setColor(false).setModuleCacheDir(null)
        .setEnvironmentVariables(Map.of()).setExternalProperties(Map.of())
        .setAllowedResources(List.of(Pattern.compile("life:.*")))
        .setResourceReaders(List.of(reader)).build()) {
      reader.evaluator = evaluator;
      if (mode.equals("retry")) {
        for (int attempt = 1; attempt <= 4; attempt++) {
          try {
            String value = evaluator.evaluateExpressionString(reader.source, "result");
            System.out.println("attempt=" + attempt + " outcome=ok value=" + value
                + " first_calls=" + reader.first + " second_calls=" + reader.second);
          } catch (PklException error) {
            diagnostic(output, "attempt-" + attempt, error);
            System.out.println("attempt=" + attempt + " outcome=error first_calls=" + reader.first
                + " second_calls=" + reader.second);
          }
        }
      } else if (mode.equals("reentry")) {
        String seed = evaluator.evaluateExpressionString(reader.source, "seed");
        System.out.println("warmup value=" + seed + " reader_calls=" + reader.calls);
        String value = evaluator.evaluateExpressionString(reader.source, "result");
        System.out.println("outer value=" + value + " reader_calls=" + reader.calls);
        value = evaluator.evaluateExpressionString(reader.source, "shared");
        System.out.println("shared value=" + value + " reader_calls=" + reader.calls);
        value = evaluator.evaluateExpressionString(reader.source, "result");
        System.out.println("repeat value=" + value + " reader_calls=" + reader.calls);
      } else if (mode.equals("nested-new-read")) {
        try {
          evaluator.evaluateExpressionString(reader.source, "result");
          throw new IllegalStateException("unexpected nested-new-read success");
        } catch (PklBugException error) {
          diagnostic(output, "outer", error);
          var trace = new java.io.StringWriter();
          error.printStackTrace(new java.io.PrintWriter(trace));
          Files.writeString(output.resolve("outer.trace.bin"), trace.toString(), StandardCharsets.UTF_8);
          System.out.println("outer outcome=error");
          System.out.println("reader_calls=" + reader.calls);
        }
      } else {
        throw new IllegalArgumentException("unknown mode");
      }
      evaluator.close();
      evaluator.close();
      System.out.println("close outcome=ok");
      try {
        evaluator.evaluateExpressionString(reader.source, "result");
        throw new IllegalStateException("evaluation succeeded after close");
      } catch (org.graalvm.polyglot.PolyglotException | IllegalStateException error) {
        if (error instanceof IllegalStateException && error.getMessage().equals("evaluation succeeded after close")) throw error;
        diagnostic(output, "after-close", error);
        System.out.println("after-close outcome=error");
      }
    }
  }
}

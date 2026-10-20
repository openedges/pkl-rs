// Development-only pinned Java oracle. Production evaluation never calls Java.
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.nio.file.Files;
import java.util.Map;
import org.pkl.core.EvaluatorBuilder;
import org.pkl.core.ModuleSource;
import org.pkl.core.PklException;

public final class M2LanguageProbe {
  private static void outcome(String value) throws Exception {
    String destination = System.getProperty("pkl.m2.outcome");
    if (destination != null) Files.writeString(Path.of(destination), value, StandardCharsets.UTF_8);
  }

  public static void main(String[] args) throws Exception {
    if (args.length != 2) throw new IllegalArgumentException("source expression");
    try (var evaluator = EvaluatorBuilder.preconfigured().setModuleCacheDir(null)
        .setColor(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of()).build()) {
      try {
        var value = evaluator.evaluateExpressionString(ModuleSource.path(Path.of(args[0])), args[1]);
        outcome("success\n");
        System.out.write(value.getBytes(StandardCharsets.UTF_8));
      } catch (PklException error) {
        outcome("pkl-exception\t" + error.getClass().getName() + "\n");
        System.err.write(error.getMessage().getBytes(StandardCharsets.UTF_8));
        System.exit(1);
      }
    }
  }
}

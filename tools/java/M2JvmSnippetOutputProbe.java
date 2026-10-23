// Development-only pinned JVM snippet boundary. Production never calls Java.
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import org.pkl.core.EvaluatorBuilder;
import org.pkl.core.Loggers;
import org.pkl.core.ModuleSource;
import org.pkl.core.PklBugException;
import org.pkl.core.PklException;
import org.pkl.core.StackFrameTransformers;

public final class M2JvmSnippetOutputProbe {
  private static void outcome(String text) throws Exception {
    Files.writeString(Path.of(System.getProperty("pkl.m2.outcome")), text, StandardCharsets.UTF_8);
  }

  public static void main(String[] args) throws Exception {
    if (args.length != 1) throw new IllegalArgumentException("source-path");
    // LanguageSnippetTestsEngine.evaluatorBuilder, without the package/TLS
    // server unused by the explicitly selected local classpath input. Keep the
    // preconfigured ClassLoader factories and the original service descriptors.
    try (var evaluator = EvaluatorBuilder.preconfigured()
        .setColor(false).setLogger(Loggers.stdErr())
        .setStackFrameTransformer(StackFrameTransformers.empty)
        .setEnvironmentVariables(Map.of(
            "NAME1", "value1", "NAME2", "value2", "/foo/bar", "foobar",
            "foo bar", "foo bar", "file:///foo/bar", "file:///foo/bar"))
        .setExternalProperties(Map.of("name1", "value1", "name2", "value2", "/foo/bar", "foobar"))
        .setModuleCacheDir(null).setPowerAssertionsEnabled(true).build()) {
      try {
        var bytes = evaluator.evaluateOutputBytes(ModuleSource.path(Path.of(args[0])));
        outcome("success\n");
        System.out.write(bytes);
      } catch (PklBugException error) {
        outcome("oracle-error\t" + error.getClass().getName() + "\n");
        System.err.write(error.getMessage().getBytes(StandardCharsets.UTF_8));
        System.exit(1);
      } catch (PklException error) {
        outcome("pkl-exception\t" + error.getClass().getName() + "\n");
        System.err.write(error.getMessage().getBytes(StandardCharsets.UTF_8));
        System.exit(1);
      }
    }
  }
}

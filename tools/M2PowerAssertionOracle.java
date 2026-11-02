// Development-only Java oracle; production evaluation does not use Java.
import java.io.DataOutputStream;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import org.pkl.core.EvaluatorBuilder;
import org.pkl.core.ModuleSource;
import org.pkl.core.PklBugException;
import org.pkl.core.PklException;

/** One expression-string capture with an authored URI and explicit power setting. */
public final class M2PowerAssertionOracle {
  private static void writeText(Path dir, String name, String text) throws Exception {
    Files.write(dir.resolve(name + ".bin"), text.getBytes(StandardCharsets.UTF_8));
    try (var out = new DataOutputStream(Files.newOutputStream(dir.resolve(name + ".code-units.bin")))) {
      for (int i = 0; i < text.length(); i++) out.writeChar(text.charAt(i));
    }
  }

  public static void main(String[] args) throws Exception {
    if (args.length != 5 || !(args[3].equals("on") || args[3].equals("off"))) {
      throw new IllegalArgumentException("OUTPUT SOURCE URI on|off EXPRESSION");
    }
    var output = Path.of(args[0]);
    var sourceBytes = Files.readString(Path.of(args[1]), StandardCharsets.UTF_8);
    var source = ModuleSource.create(URI.create(args[2]), sourceBytes);
    var expression = args[4];
    Files.createDirectory(output);
    try (var evaluator = EvaluatorBuilder.preconfigured().setModuleCacheDir(null)
        .setPowerAssertionsEnabled(args[3].equals("on"))
        .setColor(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of()).build()) {
      String status = "success\n", result = "", diagnostic = "", errorClass = "";
      try {
        result = evaluator.evaluateExpressionString(source, expression);
      } catch (PklException error) {
        status = error instanceof PklBugException ? "oracle-error\n" : "error\n";
        diagnostic = error.getMessage();
        if (diagnostic == null) throw new IllegalStateException("PklException without message", error);
        errorClass = error.getClass().getName();
      }
      Files.write(output.resolve("result.status.bin"), status.getBytes(StandardCharsets.UTF_8));
      writeText(output, "result.stdout", result);
      writeText(output, "result.stderr", diagnostic);
      writeText(output, "result.error-class", errorClass);
      writeText(output, "expression", expression);
    }
  }
}

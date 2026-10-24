// Pkl 0.32.0 Any.ifNonNull Java authority: complete raw string API channels.
import java.io.DataOutputStream;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Locale;
import java.util.Map;
import org.pkl.core.Evaluator;
import org.pkl.core.EvaluatorBuilder;
import org.pkl.core.ModuleSource;
import org.pkl.core.PklBugException;
import org.pkl.core.PklException;

public final class AnyIfNonNullOracle {
  static int calls, successes, errors, oracleErrors, apiExceptions;
  static void bytes(Path folder, String name, byte[] raw) throws Exception {
    Files.write(folder.resolve(name + ".bin"), raw);
  }
  static void utf8(Path folder, String name, String value) throws Exception {
    bytes(folder, name, value.getBytes(StandardCharsets.UTF_8));
    try (var stream = new DataOutputStream(Files.newOutputStream(folder.resolve(name + ".code-units.bin")))) {
      for (int i = 0; i < value.length(); i++) stream.writeChar(value.charAt(i));
    }
  }
  static String status(Throwable error) {
    if (error instanceof PklBugException) return "oracle-error\n";
    if (error instanceof PklException) return "error\n";
    return "api-exception\n";
  }
  static String message(Throwable error) {
    return error.getMessage() == null ? error.toString() : error.getMessage();
  }
  public static void main(String[] args) throws Exception {
    if (args.length != 3) throw new IllegalArgumentException("OUTPUT INPUT URI");
    Path output = Path.of(args[0]);
    Path input = Path.of(args[1]);
    var source = ModuleSource.create(URI.create(args[2]), Files.readString(input.resolve("module.pkl"), StandardCharsets.UTF_8));
    var lines = Files.readAllLines(input.resolve("queries.tsv"), StandardCharsets.UTF_8);
    System.out.println("session-open mode=string rows=" + lines.size());
    try (Evaluator evaluator = EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false)
        .setPowerAssertionsEnabled(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of()).build()) {
      for (int i = 0; i < lines.size(); i++) {
        var columns = lines.get(i).split("\t", -1);
        if (columns.length != 2 || !columns[0].matches("[a-z0-9-]+")) throw new IllegalArgumentException("bad row " + i);
        String id = columns[0];
        String expression = columns[1];
        Path folder = output.resolve(String.format(Locale.ROOT, "%03d-%s", i, id));
        Files.createDirectory(folder);
        utf8(folder, "expression", expression);
        String call = "string-" + i + "-" + id;
        System.out.println("call-enter=" + call);
        calls++;
        String stdout = "", stderr = "", errorClass = "", result = "success\n";
        try {
          stdout = evaluator.evaluateExpressionString(source, expression);
          successes++;
        } catch (Throwable thrown) {
          result = status(thrown);
          stderr = message(thrown);
          errorClass = thrown.getClass().getName();
          if (result.equals("error\n")) errors++;
          else if (result.equals("oracle-error\n")) oracleErrors++;
          else apiExceptions++;
        }
        bytes(folder, "result.status", result.getBytes(StandardCharsets.UTF_8));
        utf8(folder, "result.stdout", stdout);
        utf8(folder, "result.stderr", stderr);
        utf8(folder, "result.error-class", errorClass);
        System.out.println("call-exit=" + call + " status=" + result.strip());
      }
    }
    System.out.println("session-closed mode=string rows=" + lines.size() + " public-calls=" + calls
        + " success=" + successes + " errors=" + (errors + oracleErrors + apiExceptions));
  }
}

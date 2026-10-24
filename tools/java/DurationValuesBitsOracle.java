// Separate Stage 59 Pkl 0.32.0 exported Duration raw double-bit observations.
import java.io.DataOutputStream;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Locale;
import java.util.Map;
import org.pkl.core.Duration;
import org.pkl.core.EvaluatorBuilder;
import org.pkl.core.ModuleSource;
import org.pkl.core.PklBugException;
import org.pkl.core.PklException;

public final class DurationValuesBitsOracle {
  static int calls, successes, errors;
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
  public static void main(String[] args) throws Exception {
    if (args.length != 3) throw new IllegalArgumentException("OUTPUT INPUT URI");
    Path output = Path.of(args[0]), input = Path.of(args[1]);
    var source = ModuleSource.create(URI.create(args[2]), Files.readString(input.resolve("module.pkl"), StandardCharsets.UTF_8));
    var lines = Files.readAllLines(input.resolve("queries.tsv"), StandardCharsets.UTF_8);
    System.out.println("session-open mode=export-bits rows=" + lines.size());
    try (var evaluator = EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false)
        .setPowerAssertionsEnabled(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of()).build()) {
      for (int i = 0; i < lines.size(); i++) {
        var parts = lines.get(i).split("\t", -1);
        if (parts.length != 2 || !parts[0].matches("[a-z0-9-]+")) throw new IllegalArgumentException("bad row " + i);
        String id = parts[0], expression = parts[1];
        Path folder = output.resolve(String.format(Locale.ROOT, "%03d-%s", i, id));
        Files.createDirectory(folder);
        utf8(folder, "expression", expression);
        String call = "export-bits-" + i + "-" + id;
        System.out.println("call-enter=" + call);
        calls++;
        String result = "success\n", error = "", errorClass = "", bits = "", unit = "", javaType = "";
        try {
          Object value = evaluator.evaluateExpression(source, expression);
          javaType = value.getClass().getName();
          if (!(value instanceof Duration duration)) throw new IllegalStateException("not Duration: " + javaType);
          bits = String.format(Locale.ROOT, "%016x", Double.doubleToRawLongBits(duration.getValue()));
          unit = duration.getUnit().getSymbol();
          successes++;
        } catch (Throwable thrown) {
          result = status(thrown);
          error = thrown.getMessage() == null ? thrown.toString() : thrown.getMessage();
          errorClass = thrown.getClass().getName();
          errors++;
        }
        bytes(folder, "result.status", result.getBytes(StandardCharsets.UTF_8));
        utf8(folder, "result.error", error);
        utf8(folder, "result.error-class", errorClass);
        utf8(folder, "result.java-type", javaType);
        utf8(folder, "result.raw-bits-hex", bits);
        utf8(folder, "result.unit", unit);
        System.out.println("call-exit=" + call + " status=" + result.strip());
      }
    }
    System.out.println("session-closed mode=export-bits rows=" + lines.size() + " public-calls=" + calls
        + " success=" + successes + " errors=" + errors);
  }
}

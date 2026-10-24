// Independent public Pkl 0.32.0 IntSeq string and typed-export authority.
import java.io.DataOutputStream;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Locale;
import java.util.Map;
import org.pkl.core.EvaluatorBuilder;
import org.pkl.core.ModuleSource;
import org.pkl.core.PklBugException;
import org.pkl.core.PklException;

public final class IntSeqValuesOracle {
  static int calls, successes, errors;

  static void bytes(Path folder, String name, byte[] raw) throws Exception {
    Files.write(folder.resolve(name + ".bin"), raw);
  }

  static void string(Path folder, String name, String value) throws Exception {
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
    Path output = Path.of(args[0]), input = Path.of(args[1]);
    var source = ModuleSource.create(URI.create(args[2]), Files.readString(input.resolve("module.pkl"), StandardCharsets.UTF_8));
    var lines = Files.readAllLines(input.resolve("queries.tsv"), StandardCharsets.UTF_8);
    if (lines.isEmpty()) throw new IllegalArgumentException("empty query set");
    String sessionMode = lines.get(0).split("\t", -1)[0];
    if (!(sessionMode.equals("string") || sessionMode.equals("export"))) throw new IllegalArgumentException("bad mode");
    System.out.println("session-open mode=" + sessionMode + " rows=" + lines.size());
    try (var evaluator = EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false)
        .setPowerAssertionsEnabled(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of()).build()) {
      for (int i = 0; i < lines.size(); i++) {
        var columns = lines.get(i).split("\t", -1);
        if (columns.length != 3 || !columns[0].equals(sessionMode)
            || !columns[1].matches("[a-z0-9-]+")) throw new IllegalArgumentException("bad row " + i);
        String mode = columns[0], id = columns[1], expression = columns[2];
        Path folder = output.resolve(String.format(Locale.ROOT, "%03d-%s", i, id));
        Files.createDirectory(folder);
        string(folder, "mode", mode);
        string(folder, "expression", expression);
        String call = mode + "-" + i + "-" + id;
        System.out.println("call-enter=" + call);
        calls++;
        String stdout = "", stderr = "", errorClass = "", exceptionToString = "";
        String causeClass = "", causeMessage = "", causeToString = "", javaType = "", result = "success\n";
        try {
          if (mode.equals("string")) stdout = evaluator.evaluateExpressionString(source, expression);
          else {
            Object value = evaluator.evaluateExpression(source, expression);
            javaType = value == null ? "null" : value.getClass().getName();
            stdout = String.valueOf(value);
          }
          successes++;
        } catch (Throwable thrown) {
          result = status(thrown);
          stderr = message(thrown);
          errorClass = thrown.getClass().getName();
          exceptionToString = thrown.toString();
          if (thrown.getCause() != null) {
            causeClass = thrown.getCause().getClass().getName();
            causeMessage = message(thrown.getCause());
            causeToString = thrown.getCause().toString();
          }
          errors++;
        }
        bytes(folder, "result.status", result.getBytes(StandardCharsets.UTF_8));
        string(folder, "result.stdout", stdout);
        string(folder, "result.stderr", stderr);
        string(folder, "result.error-class", errorClass);
        string(folder, "result.exception-to-string", exceptionToString);
        string(folder, "result.cause-class", causeClass);
        string(folder, "result.cause-message", causeMessage);
        string(folder, "result.cause-to-string", causeToString);
        string(folder, "result.java-type", javaType);
        System.out.println("call-exit=" + call + " status=" + result.strip());
      }
    }
    System.out.println("session-closed mode=" + sessionMode + " rows=" + lines.size() + " public-calls=" + calls
        + " success=" + successes + " errors=" + errors);
  }
}

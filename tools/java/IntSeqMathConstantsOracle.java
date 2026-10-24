// Pinned Java 0.32.0 public math constant string and numeric export observations.
import java.io.DataOutputStream;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import org.pkl.core.EvaluatorBuilder;
import org.pkl.core.ModuleSource;

public final class IntSeqMathConstantsOracle {
  static void bytes(Path folder, String name, byte[] raw) throws Exception {
    Files.write(folder.resolve(name + ".bin"), raw);
  }

  static void string(Path folder, String name, String value) throws Exception {
    bytes(folder, name, value.getBytes(StandardCharsets.UTF_8));
    try (var stream = new DataOutputStream(Files.newOutputStream(folder.resolve(name + ".code-units.bin")))) {
      for (int i = 0; i < value.length(); i++) stream.writeChar(value.charAt(i));
    }
  }

  static String message(Throwable error) {
    return error.getMessage() == null ? error.toString() : error.getMessage();
  }

  public static void main(String[] args) throws Exception {
    if (args.length != 3) throw new IllegalArgumentException("OUTPUT INPUT URI");
    Path output = Path.of(args[0]), input = Path.of(args[1]);
    var source = ModuleSource.create(URI.create(args[2]), Files.readString(input.resolve("module.pkl"), StandardCharsets.UTF_8));
    List<String> names = Files.readAllLines(input.resolve("names.txt"), StandardCharsets.UTF_8);
    System.out.println("session-open boundary=math-constants rows=" + names.size() + " public-calls=" + (2 * names.size()));
    int calls = 0, successes = 0, errors = 0;
    try (var evaluator = EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false)
        .setPowerAssertionsEnabled(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of()).build()) {
      for (int i = 0; i < names.size(); i++) {
        String name = names.get(i);
        if (!name.matches("[A-Za-z][A-Za-z0-9]*")) throw new IllegalArgumentException("bad constant " + i);
        String expression = name;
        Path folder = output.resolve(String.format(Locale.ROOT, "%03d-%s", i, name));
        Files.createDirectory(folder);
        string(folder, "expression", expression);
        String call = "string-" + i + "-" + name;
        System.out.println("call-enter=" + call);
        calls++;
        try {
          string(folder, "string.stdout", evaluator.evaluateExpressionString(source, expression));
          string(folder, "string.stderr", "");
          bytes(folder, "string.status", "success\n".getBytes(StandardCharsets.UTF_8));
          successes++;
          System.out.println("call-exit=" + call + " status=success");
        } catch (Throwable error) {
          string(folder, "string.stdout", "");
          string(folder, "string.stderr", message(error));
          bytes(folder, "string.status", "error\n".getBytes(StandardCharsets.UTF_8));
          errors++;
          System.out.println("call-exit=" + call + " status=error");
        }
        call = "typed-" + i + "-" + name;
        System.out.println("call-enter=" + call);
        calls++;
        try {
          Object value = evaluator.evaluateExpression(source, expression);
          long bits;
          if (value instanceof Long integer) bits = integer;
          else if (value instanceof Double floating) bits = Double.doubleToRawLongBits(floating);
          else throw new IllegalStateException("unexpected Java numeric class: " + value.getClass().getName());
          try (var stream = new DataOutputStream(Files.newOutputStream(folder.resolve("typed.raw-be8.bin")))) {
            stream.writeLong(bits);
          }
          string(folder, "typed.java-type", value.getClass().getName());
          string(folder, "typed.value-string", String.valueOf(value));
          string(folder, "typed.stderr", "");
          bytes(folder, "typed.status", "success\n".getBytes(StandardCharsets.UTF_8));
          successes++;
          System.out.println("call-exit=" + call + " status=success");
        } catch (Throwable error) {
          bytes(folder, "typed.raw-be8", new byte[0]);
          string(folder, "typed.java-type", "");
          string(folder, "typed.value-string", "");
          string(folder, "typed.stderr", message(error));
          bytes(folder, "typed.status", "error\n".getBytes(StandardCharsets.UTF_8));
          errors++;
          System.out.println("call-exit=" + call + " status=error");
        }
      }
    }
    System.out.println("session-closed boundary=math-constants rows=" + names.size() + " public-calls=" + calls
        + " success=" + successes + " errors=" + errors);
  }
}

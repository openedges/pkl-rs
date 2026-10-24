// Java-only public exported Class identity/lifetime observations, never candidate expectations.
import java.io.DataOutputStream;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import org.pkl.core.*;

public final class M2ClassExportProbe {
  static Path out;
  static void write(String name, String value) throws Exception {
    Files.write(out.resolve(name), value.getBytes(StandardCharsets.UTF_8));
  }
  static void string(String name, String value) throws Exception {
    write(name + ".bin", value);
    try (var stream = new DataOutputStream(Files.newOutputStream(out.resolve(name + ".code-units.bin")))) {
      for (int i = 0; i < value.length(); i++) stream.writeChar(value.charAt(i));
    }
  }
  static Evaluator build() {
    return EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false)
        .setPowerAssertionsEnabled(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of()).build();
  }
  static Object call(Evaluator evaluator, ModuleSource source, String expression, String prefix, boolean stringResult) throws Exception {
    Object result = null;
    String value = "", error = "", errorClass = "", status = "success\n", valueType = "";
    System.out.println("call-enter=" + prefix);
    try {
      result = stringResult ? evaluator.evaluateExpressionString(source, expression) : evaluator.evaluateExpression(source, expression);
      if (stringResult) value = (String) result;
      valueType = result == null ? "null" : result.getClass().getName();
    } catch (PklException exception) {
      status = exception instanceof PklBugException ? "oracle-error\n" : "error\n";
      error = exception.getMessage(); errorClass = exception.getClass().getName();
    } catch (Throwable exception) {
      status = "api-exception\n"; error = exception.toString(); errorClass = exception.getClass().getName();
    }
    string(prefix + ".stdout", value); string(prefix + ".stderr", error);
    write(prefix + ".status.bin", status); write(prefix + ".error-class.bin", errorClass);
    write(prefix + ".java-value-type.bin", valueType);
    System.out.println("call-exit=" + prefix + " status=" + status.strip());
    return result;
  }
  static void compare(String name, Object first, Object second) throws Exception {
    String reference = first == null || second == null ? "unavailable" : String.valueOf(first == second);
    String equals = first == null || second == null ? "unavailable" : String.valueOf(first.equals(second));
    string(name + ".reference", reference); string(name + ".equals", equals);
    System.out.println("identity=" + name + " reference=" + reference + " equals=" + equals);
  }
  public static void main(String[] args) throws Exception {
    if (args.length != 3) throw new IllegalArgumentException("DIRECTORY URI OTHER_URI");
    out = Path.of(args[0]);
    var first = ModuleSource.create(URI.create(args[1]), Files.readString(out.resolve("first.pkl")));
    var changed = ModuleSource.create(URI.create(args[1]), Files.readString(out.resolve("changed.pkl")));
    var other = ModuleSource.create(URI.create(args[2]), Files.readString(out.resolve("first.pkl")));
    try (var evaluator = build(); var independent = build()) {
      Object a = call(evaluator, first, "Child", "export-0", false);
      Object b = call(evaluator, first, "Child", "export-1", false);
      Object c = call(evaluator, changed, "Child", "export-2", false);
      Object d = call(evaluator, other, "Child", "export-3", false);
      Object e = call(independent, first, "Child", "export-4", false);
      compare("same-uri", a, b); compare("same-uri-changed-source", a, c);
      compare("different-uri", a, d); compare("different-evaluator", a, e);
      call(evaluator, changed, "marker", "marker-0", true);
      evaluator.close();
      call(evaluator, first, "Child", "post-close-0", false);
    }
    System.out.println("closed public-calls=7 identity-comparisons=4");
  }
}

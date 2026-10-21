// Oracle-only module cache identity observations, not full Rust module loading.
import java.net.URI;
import java.nio.file.*;
import java.util.Map;
import org.pkl.core.*;

public final class M2ClassIdentityProbe {
  private static Evaluator evaluator() {
    return EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false)
        .setPowerAssertionsEnabled(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of()).build();
  }
  public static void main(String[] args) throws Exception {
    Path out = Path.of(args[0]);
    var first = ModuleSource.create(URI.create("file:///m2-platform.pkl"), Files.readString(out.resolve("first.pkl")));
    var changed = ModuleSource.create(URI.create("file:///m2-platform.pkl"), Files.readString(out.resolve("changed.pkl")));
    var other = ModuleSource.create(URI.create("file:///m2-other.pkl"), Files.readString(out.resolve("first.pkl")));
    try (var e = evaluator(); var independent = evaluator()) {
      Object a = e.evaluateExpression(first, "Child");
      System.out.println("same-uri-source=" + (a == e.evaluateExpression(first, "Child")));
      System.out.println("same-uri-changed-source=" + (a == e.evaluateExpression(changed, "Child")));
      System.out.println("same-uri-changed-marker=" + e.evaluateExpressionString(changed, "marker"));
      System.out.println("different-uri-same-source=" + (a == e.evaluateExpression(other, "Child")));
      System.out.println("different-evaluator=" + (a == independent.evaluateExpression(first, "Child")));
    }
  }
}

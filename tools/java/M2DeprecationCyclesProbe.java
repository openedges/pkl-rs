// Development-only public Java evaluator cycles; shares the exact logger capture utility.
import java.net.URI;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import org.pkl.core.*;

public final class M2DeprecationCyclesProbe {
  public static void main(String[] args) throws Exception {
    if (args.length != 3) throw new IllegalArgumentException("DIRECTORY URI CYCLES");
    var out = Path.of(args[0]);
    M2DeprecationProbe.out = out;
    M2DeprecationProbe.action = "capture";
    M2DeprecationProbe.callback = "";
    M2DeprecationProbe.source = ModuleSource.create(URI.create(args[1]), Files.readString(out.resolve("input.pkl")));
    var expressions = Files.readAllLines(out.resolve("expressions.txt"));
    for (int cycle = 0; cycle < Integer.parseInt(args[2]); cycle++) {
      System.out.println("cycle-enter=" + cycle);
      try (var evaluator = EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false)
          .setPowerAssertionsEnabled(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of())
          .setLogger(new M2DeprecationProbe.CaptureLogger()).build()) {
        M2DeprecationProbe.evaluator = evaluator;
        for (int call = 0; call < expressions.size(); call++)
          M2DeprecationProbe.capture(expressions.get(call), "cycle-" + cycle + "-outer-" + call);
      }
      System.out.println("cycle-exit=" + cycle);
    }
    System.out.println("closed logs=" + M2DeprecationProbe.logs + " warnings=" + M2DeprecationProbe.warnings);
  }
}

// Development-only persistent oracle; one Evaluator remains open across every call.
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import org.pkl.core.*;

public final class M2ClassLifecycleProbe {
  public static void main(String[] args) throws Exception {
    Path sourcePath = Path.of(args[0]), expressions = Path.of(args[1]), out = Path.of(args[2]);
    var source = ModuleSource.create(URI.create("file:///m2-platform.pkl"), Files.readString(sourcePath));
    try (var evaluator = EvaluatorBuilder.preconfigured().setModuleCacheDir(null)
        .setPowerAssertionsEnabled(false).setColor(false).setEnvironmentVariables(Map.of())
        .setExternalProperties(Map.of()).build()) {
      var calls = Files.readAllLines(expressions);
      for (int i = 0; i < calls.size(); i++) {
        byte[] stdout = new byte[0], stderr = new byte[0];
        String status, errorClass = "";
        try {
          stdout = evaluator.evaluateExpressionString(source, calls.get(i)).getBytes(StandardCharsets.UTF_8);
          status = "success\n";
        } catch (PklException error) {
          status = error instanceof PklBugException ? "oracle-error\n" : "error\n";
          stderr = error.getMessage().getBytes(StandardCharsets.UTF_8);
          errorClass = error.getClass().getName();
        } catch (Throwable error) {
          status = "oracle-error\n";
          errorClass = error.getClass().getName();
          var bytes = new java.io.ByteArrayOutputStream();
          error.printStackTrace(new java.io.PrintStream(bytes, true, StandardCharsets.UTF_8));
          stderr = bytes.toByteArray();
        }
        Files.write(out.resolve(i + ".stdout.bin"), stdout);
        Files.write(out.resolve(i + ".stderr.bin"), stderr);
        Files.writeString(out.resolve(i + ".status.bin"), status, StandardCharsets.UTF_8);
        Files.writeString(out.resolve(i + ".error-class.bin"), errorClass, StandardCharsets.UTF_8);
        System.out.println("call=" + i + " completed");
      }
    }
    System.out.println("closed");
  }
}

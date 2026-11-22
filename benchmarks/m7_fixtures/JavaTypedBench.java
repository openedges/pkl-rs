// SPDX-License-Identifier: Apache-2.0
// Development-only Java typed-evaluation timing harness for apples-to-apples comparison
// against `@pkl-rs/napi` and `@pkl-rs/wasm` `evaluateExpressionValue(source, "module")`.
// Materializes the full `PModule` (`org.pkl.core.Evaluator.evaluate`) including 64-bit Longs,
// `byte[]`, UTF-16 Strings, and nested `PObject` collections, verifying raw output.text
// preflight and typed value integrity outside the timed region.
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import org.pkl.core.Evaluator;
import org.pkl.core.EvaluatorBuilder;
import org.pkl.core.ModuleSource;
import org.pkl.core.PModule;
import org.pkl.core.PObject;

public final class JavaTypedBench {
  private static Evaluator evaluator() {
    return EvaluatorBuilder.preconfigured()
        .setModuleCacheDir(null)
        .setColor(false)
        .setEnvironmentVariables(Map.of())
        .setExternalProperties(Map.of())
        .build();
  }

  @SuppressWarnings("unchecked")
  private static void verifyTypedModule(PModule mod) {
    Object countObj = mod.getProperty("count");
    if (!(countObj instanceof Long) || ((Long) countObj).longValue() != 200L) {
      throw new AssertionError("unexpected count: " + countObj);
    }
    Object maxBigObj = mod.getProperty("maxBig");
    if (!(maxBigObj instanceof Long) || ((Long) maxBigObj).longValue() != 9007199254741193L) {
      throw new AssertionError("unexpected maxBig: " + maxBigObj);
    }
    Object payloadObj = mod.getProperty("payload");
    if (!(payloadObj instanceof byte[])) {
      throw new AssertionError("unexpected payload type: " + payloadObj);
    }
    byte[] payload = (byte[]) payloadObj;
    if (payload.length != 7 || (payload[0] & 0xFF) != 0 || (payload[6] & 0xFF) != 255) {
      throw new AssertionError("unexpected payload bytes");
    }
    Object itemsObj = mod.getProperty("items");
    if (!(itemsObj instanceof List<?>)) {
      throw new AssertionError("unexpected items type: " + itemsObj);
    }
    List<Object> items = (List<Object>) itemsObj;
    if (items.size() != 200) {
      throw new AssertionError("unexpected items length: " + items.size());
    }
    PObject first = (PObject) items.get(0);
    if (((Long) first.getProperty("id")).longValue() != 9007199254740994L
        || ((Long) first.getProperty("negId")).longValue() != -9007199254740994L) {
      throw new AssertionError("64-bit integer precision check failed");
    }
    if (!"row-1-한글-😀".equals(first.getProperty("label"))) {
      throw new AssertionError("UTF-16 string check failed: " + first.getProperty("label"));
    }
    byte[] raw = (byte[]) first.getProperty("raw");
    if (raw.length != 4 || (raw[3] & 0xFF) != 255) {
      throw new AssertionError("nested byte[] check failed");
    }
  }

  private static double window(
      Evaluator evaluator, ModuleSource source, long minimumNanos, String phase, int index) {
    long elapsed = 0;
    long iterations = 0;
    while (elapsed < minimumNanos) {
      long before = System.nanoTime();
      PModule mod = evaluator.evaluate(source);
      elapsed += System.nanoTime() - before;
      verifyTypedModule(mod);
      iterations++;
    }
    double perOp = (double) elapsed / iterations;
    System.out.println(
        "{\"phase\":\""
            + phase
            + "\",\"index\":"
            + index
            + ",\"elapsed_ns\":"
            + elapsed
            + ",\"iterations\":"
            + iterations
            + ",\"ns_per_op\":"
            + perOp
            + "}");
    return perOp;
  }

  private static boolean stable(List<Double> values, int windows, double tolerance) {
    if (values.size() < windows) return false;
    double[] recent =
        values.subList(values.size() - windows, values.size()).stream()
            .mapToDouble(Double::doubleValue)
            .sorted()
            .toArray();
    double median = (recent[windows / 2 - 1] + recent[windows / 2]) / 2.0;
    return median > 0 && (recent[windows - 1] - recent[0]) / median <= tolerance;
  }

  public static void main(String[] args) throws Exception {
    if (args.length != 9) {
      throw new IllegalArgumentException(
          "expected 9 args: <module> <expected_text> <min_warmup_s> <max_warmup_s> <samples> <sample_s> <conv_windows> <tolerance> <first_out>");
    }
    ModuleSource source = ModuleSource.path(Path.of(args[0]));
    Path expectedTextPath = Path.of(args[1]);
    long minimumWarmup = (long) (Double.parseDouble(args[2]) * 1e9);
    long maximumWarmup = (long) (Double.parseDouble(args[3]) * 1e9);
    int samples = Integer.parseInt(args[4]);
    long sampleTime = (long) (Double.parseDouble(args[5]) * 1e9);
    int convergenceWindows = Integer.parseInt(args[6]);
    double tolerance = Double.parseDouble(args[7]);
    Path firstOut = Path.of(args[8]);

    long beforeInit = System.nanoTime();
    try (Evaluator ev = evaluator()) {
      System.out.println(
          "{\"phase\":\"initialize\",\"elapsed_ns\":" + (System.nanoTime() - beforeInit) + "}");

      // Preflight raw output.text parity before timing
      byte[] textBytes = ev.evaluateOutputText(source).getBytes(StandardCharsets.UTF_8);
      byte[] expectedText = Files.readAllBytes(expectedTextPath);
      if (!Arrays.equals(textBytes, expectedText)) {
        throw new AssertionError("JavaTypedBench preflight output.text mismatch");
      }
      Files.write(firstOut, textBytes);

      long beforeFirst = System.nanoTime();
      PModule first = ev.evaluate(source);
      long firstNs = System.nanoTime() - beforeFirst;
      verifyTypedModule(first);
      System.out.println(
          "{\"phase\":\"first-request\",\"elapsed_ns\":"
              + firstNs
              + ",\"error\":false,\"bytes\":"
              + textBytes.length
              + "}");

      var warmup = new ArrayList<Double>();
      long started = System.nanoTime();
      long elapsed;
      boolean converged;
      do {
        warmup.add(window(ev, source, sampleTime, "warmup", warmup.size()));
        elapsed = System.nanoTime() - started;
        converged = elapsed >= minimumWarmup && stable(warmup, convergenceWindows, tolerance);
      } while (!converged && elapsed < maximumWarmup);
      System.out.println(
          "{\"phase\":\"warmup-result\",\"elapsed_ns\":"
              + elapsed
              + ",\"converged\":"
              + converged
              + ",\"windows\":"
              + warmup.size()
              + "}");

      for (int i = 0; i < samples; i++) {
        window(ev, source, sampleTime, "measurement", i);
      }
    }
  }
}

// Development-only timing harness. Production evaluation never calls this code.
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
import org.pkl.core.PklException;

public final class Benchmark {
  private static final class Result {
    final byte[] bytes;
    final boolean error;
    Result(byte[] bytes, boolean error) { this.bytes = bytes; this.error = error; }
  }

  private static Evaluator evaluator() {
    return EvaluatorBuilder.preconfigured()
        .setModuleCacheDir(null).setColor(false)
        .setEnvironmentVariables(Map.of()).setExternalProperties(Map.of()).build();
  }

  private static Result call(Evaluator evaluator, ModuleSource source) {
    try {
      return new Result(evaluator.evaluateExpressionString(source, "run(0)")
          .getBytes(StandardCharsets.UTF_8), false);
    } catch (PklException error) {
      return new Result(error.getMessage().getBytes(StandardCharsets.UTF_8), true);
    }
  }

  private static Result invoke(String mode, Evaluator persistent, ModuleSource source) {
    if (mode.equals("fresh-evaluator")) {
      try (var fresh = evaluator()) { return call(fresh, source); }
    }
    return call(persistent, source);
  }

  private static void check(Result result, byte[] expected, boolean expectedError) {
    if (result.error != expectedError || !Arrays.equals(result.bytes, expected)) {
      throw new AssertionError("output bytes/outcome differ from the verified Java preflight");
    }
  }

  private static double window(String mode, Evaluator evaluator, ModuleSource source,
      byte[] expected, boolean expectedError, long minimumNanos, String phase, int index) {
    long elapsed = 0;
    long iterations = 0;
    // Correctness checks occur outside the timed operation. Each window requires
    // at least the configured amount of measured operation time, not check time.
    while (elapsed < minimumNanos) {
      long before = System.nanoTime();
      Result result = invoke(mode, evaluator, source);
      elapsed += System.nanoTime() - before;
      check(result, expected, expectedError);
      iterations++;
    }
    double perOperation = (double) elapsed / iterations;
    System.out.println("{\"phase\":\"" + phase + "\",\"index\":" + index
        + ",\"elapsed_ns\":" + elapsed + ",\"iterations\":" + iterations
        + ",\"ns_per_op\":" + perOperation + "}");
    return perOperation;
  }

  private static boolean stable(List<Double> values, int windows, double tolerance) {
    if (values.size() < windows) return false;
    double[] recent = values.subList(values.size() - windows, values.size())
        .stream().mapToDouble(Double::doubleValue).sorted().toArray();
    double median = (recent[windows / 2 - 1] + recent[windows / 2]) / 2;
    return median > 0 && (recent[windows - 1] - recent[0]) / median <= tolerance;
  }

  public static void main(String[] args) throws Exception {
    if (args.length != 12) throw new IllegalArgumentException("expected 12 harness arguments");
    String mode = args[0];
    if (!List.of("verify", "persistent", "fresh-evaluator").contains(mode)) {
      throw new IllegalArgumentException("unknown benchmark mode: " + mode);
    }
    ModuleSource source = ModuleSource.path(Path.of(args[1]));
    Path expectedPath = Path.of(args[2]);
    boolean expectedError = Boolean.parseBoolean(args[3]);
    long minimumWarmup = (long) (Double.parseDouble(args[4]) * 1e9);
    long maximumWarmup = (long) (Double.parseDouble(args[5]) * 1e9);
    int samples = Integer.parseInt(args[6]);
    long sampleTime = (long) (Double.parseDouble(args[7]) * 1e9);
    int convergenceWindows = Integer.parseInt(args[8]);
    double tolerance = Double.parseDouble(args[9]);
    Path firstOutput = Path.of(args[10]);
    boolean writeExpected = Boolean.parseBoolean(args[11]);
    if (!mode.equals("verify") && (minimumWarmup < 10_000_000_000L
        || maximumWarmup < minimumWarmup || samples < 5 || sampleTime < 100_000_000L
        || convergenceWindows < 4 || convergenceWindows % 2 != 0
        || tolerance <= 0 || tolerance > 0.5)) {
      throw new IllegalArgumentException("insufficient warmup/sampling contract");
    }
    long before = System.nanoTime();
    Evaluator owned = null;
    if (mode.equals("fresh-evaluator")) {
      // Record context construction/close separately, without retaining an unused
      // evaluator alongside the fresh evaluator in every timed operation.
      try (var initial = evaluator()) { }
    } else {
      owned = evaluator();
    }
    try (var persistent = owned) {
      System.out.println("{\"phase\":\"initialize\",\"elapsed_ns\":" + (System.nanoTime() - before) + "}");
      before = System.nanoTime();
      Result first = invoke(mode, persistent, source);
      long firstNanos = System.nanoTime() - before;
      Files.write(firstOutput, first.bytes);
      if (first.error != expectedError) throw new AssertionError("unexpected preflight outcome");
      if (writeExpected) Files.write(expectedPath, first.bytes);
      byte[] expected = Files.readAllBytes(expectedPath);
      check(first, expected, expectedError);
      System.out.println("{\"phase\":\"first-request\",\"elapsed_ns\":" + firstNanos
          + ",\"error\":" + first.error + ",\"bytes\":" + first.bytes.length + "}");
      if (mode.equals("verify")) return;
      var warmup = new ArrayList<Double>();
      long started = System.nanoTime();
      long elapsed;
      boolean converged;
      do {
        warmup.add(window(mode, persistent, source, expected, expectedError, sampleTime,
            "warmup", warmup.size()));
        elapsed = System.nanoTime() - started;
        converged = elapsed >= minimumWarmup && stable(warmup, convergenceWindows, tolerance);
      } while (!converged && elapsed < maximumWarmup);
      System.out.println("{\"phase\":\"warmup-result\",\"elapsed_ns\":" + elapsed
          + ",\"converged\":" + converged + ",\"windows\":" + warmup.size() + "}");
      for (int index = 0; index < samples; index++) {
        window(mode, persistent, source, expected, expectedError, sampleTime, "measurement", index);
      }
    }
  }
}

// Development-only JVM oracle and bounded warm-workload probe.
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Map;
import org.pkl.core.Evaluator;
import org.pkl.core.EvaluatorBuilder;
import org.pkl.core.ModuleSource;
import org.pkl.core.PklException;

public final class M1RuntimeProbe {
  private static long lastWindowNanos;
  private static Evaluator evaluator() {
    return EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false)
        .setEnvironmentVariables(Map.of()).setExternalProperties(Map.of()).build();
  }

  private static void check(String value, int seed) {
    if (!Arrays.equals(value.getBytes(StandardCharsets.UTF_8),
        Integer.toString(seed + 8256).getBytes(StandardCharsets.UTF_8))) {
      throw new AssertionError("arithmetic result bytes differ");
    }
  }

  private static double window(Evaluator evaluator, ModuleSource source, String[] expressions,
      String phase, int index, long minimumNanos) {
    long elapsed = 0, iterations = 0;
    while (elapsed < minimumNanos) {
      int seed = (int) (iterations % 256);
      long before = System.nanoTime();
      String value = evaluator.evaluateExpressionString(source, expressions[seed]);
      elapsed += System.nanoTime() - before;
      check(value, seed);
      iterations++;
    }
    double perOperation = (double) elapsed / iterations;
    lastWindowNanos = elapsed;
    System.out.println("{\"phase\":\"" + phase + "\",\"index\":" + index
        + ",\"elapsed_ns\":" + elapsed + ",\"iterations\":" + iterations
        + ",\"ns_per_op\":" + perOperation + "}");
    return perOperation;
  }

  private static boolean stable(ArrayList<Double> values) {
    if (values.size() < 8) return false;
    double[] recent = values.subList(values.size() - 8, values.size())
        .stream().mapToDouble(Double::doubleValue).sorted().toArray();
    double median = (recent[3] + recent[4]) / 2;
    return (recent[7] - recent[0]) / median <= 0.15;
  }

  public static void main(String[] args) throws Exception {
    if (args.length < 3) throw new IllegalArgumentException("mode source member/seconds [samples]");
    ModuleSource source = ModuleSource.path(Path.of(args[1]));
    try (Evaluator evaluator = evaluator()) {
      if (args[0].equals("compare")) {
        try {
          System.out.print(evaluator.evaluateExpressionString(source, args[2]));
        } catch (PklException error) {
          System.err.print(error.getMessage());
          System.exit(1);
        }
        return;
      }
      if (args[0].equals("compilation") && args.length == 4) {
        long minimum = Long.parseLong(args[2]) * 1_000_000_000L;
        boolean floating = args[3].equals("float");
        long started = System.nanoTime();
        long calls = 0;
        do {
          int seed = (int) (calls % 256);
          String value = evaluator.evaluateExpressionString(source,
              "run(" + seed + (floating ? ".0)" : ")"));
          String expected = Integer.toString(seed + 8256) + (floating ? ".0" : "");
          if (!Arrays.equals(value.getBytes(StandardCharsets.UTF_8), expected.getBytes(StandardCharsets.UTF_8))) {
            throw new AssertionError("compilation probe result mismatch");
          }
          calls++;
        } while (System.nanoTime() - started < minimum);
        System.out.println("{\"phase\":\"compilation-probe\",\"verified_calls\":" + calls + "}");
        return;
      }
      if (!args[0].equals("benchmark") || args.length != 4) {
        throw new IllegalArgumentException("invalid benchmark arguments");
      }
      long minimumWarmup = Long.parseLong(args[2]) * 1_000_000_000L;
      int samples = Integer.parseInt(args[3]);
      if (minimumWarmup < 10_000_000_000L || samples < 5) throw new IllegalArgumentException("insufficient sampling");
      String[] expressions = new String[256];
      for (int seed = 0; seed < 256; seed++) {
        expressions[seed] = "run(" + seed + ")";
        check(evaluator.evaluateExpressionString(source, expressions[seed]), seed);
      }
      System.out.println("{\"phase\":\"preflight\",\"seeds\":256}");
      var values = new ArrayList<Double>();
      long started = System.nanoTime();
      long measuredWarmup = 0;
      boolean converged;
      do {
        values.add(window(evaluator, source, expressions, "warmup", values.size(), 100_000_000L));
        measuredWarmup += lastWindowNanos;
        converged = measuredWarmup >= minimumWarmup && stable(values);
      } while (!converged && System.nanoTime() - started < minimumWarmup * 3);
      System.out.println("{\"phase\":\"warmup-result\",\"converged\":" + converged + "}");
      for (int index = 0; index < samples; index++) {
        window(evaluator, source, expressions, "measurement", index, 250_000_000L);
      }
    }
  }
}

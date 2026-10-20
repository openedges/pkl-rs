// Development-only Truffle compilation diagnostic. This is not a timing harness.
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.Map;
import org.pkl.core.Evaluator;
import org.pkl.core.EvaluatorBuilder;
import org.pkl.core.ModuleSource;

public final class M1GraalProbe {
  private static Path expectedPath(Path directory, int seed) {
    return directory.resolve(String.format(java.util.Locale.ROOT, "seed-%03d.bin", seed));
  }

  public static void main(String[] args) throws Exception {
    if (args.length < 4 || args.length > 5) {
      throw new IllegalArgumentException("preflight|exercise source arithmetic|fib expected-dir [seconds]");
    }
    boolean preflight = args[0].equals("preflight");
    if ((!preflight && !args[0].equals("exercise"))
        || args.length != (preflight ? 4 : 5)) {
      throw new IllegalArgumentException("invalid diagnostic mode or argument count");
    }
    int seeds = switch (args[2]) {
      case "arithmetic" -> 256;
      // The unchanged M0 workload computes fib(24 + seed), so use bounded inputs.
      case "fib" -> 2;
      default -> throw new IllegalArgumentException("unknown workload");
    };
    Path expectedDirectory = Path.of(args[3]);
    long seconds = preflight ? 0 : Long.parseLong(args[4]);
    if (!preflight && (seconds < 1 || seconds > 600)) {
      throw new IllegalArgumentException("diagnostic duration must be 1..600 seconds");
    }
    if (preflight) Files.createDirectory(expectedDirectory);
    byte[][] expected = new byte[seeds][];
    String[] expressions = new String[seeds];
    for (int seed = 0; seed < seeds; seed++) {
      expressions[seed] = "run(" + seed + ")";
      if (!preflight) expected[seed] = Files.readAllBytes(expectedPath(expectedDirectory, seed));
    }
    ModuleSource source = ModuleSource.path(Path.of(args[1]));
    try (Evaluator evaluator = EvaluatorBuilder.preconfigured()
        .setModuleCacheDir(null).setColor(false)
        .setEnvironmentVariables(Map.of()).setExternalProperties(Map.of()).build()) {
      if (preflight) {
        for (int seed = 0; seed < seeds; seed++) {
          Files.write(expectedPath(expectedDirectory, seed), evaluator
              .evaluateExpressionString(source, expressions[seed]).getBytes(StandardCharsets.UTF_8));
        }
        System.out.println("{\"phase\":\"preflight\",\"seeds\":" + seeds + "}");
        return;
      }
      long started = System.nanoTime();
      long calls = 0;
      do {
        int seed = (int) (calls % seeds);
        byte[] actual = evaluator.evaluateExpressionString(source, expressions[seed])
            .getBytes(StandardCharsets.UTF_8);
        if (!Arrays.equals(actual, expected[seed])) {
          throw new AssertionError("result bytes differ at seed " + seed + ", call " + calls);
        }
        calls++;
      } while (System.nanoTime() - started < seconds * 1_000_000_000L);
      System.out.println("{\"phase\":\"compilation-probe\",\"verified_calls\":" + calls
          + ",\"seeds\":" + seeds + "}");
    }
  }
}

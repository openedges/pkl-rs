// Development-only evidence that the selected JVM exposes an optimizing Truffle runtime.
import com.oracle.truffle.api.Truffle;

final class OracleRuntimeProbe {
  public static void main(String[] args) {
    System.out.println(Truffle.getRuntime().getName());
  }
}

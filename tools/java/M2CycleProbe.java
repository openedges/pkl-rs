// Diagnostic-only experiment: explicitly changes class initialization before
// evaluation. This is not the normal M2 oracle boundary or a patched Pkl JAR.
public final class M2CycleProbe {
  public static void main(String[] args) throws Exception {
    Class.forName("org.pkl.core.runtime.VmStackOverflowException");
    M2LanguageProbe.main(args);
  }
}

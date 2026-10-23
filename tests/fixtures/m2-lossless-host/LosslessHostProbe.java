// Development-only Java oracle; production evaluation never calls this helper.
import java.io.*;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;
import java.util.regex.Pattern;
import org.pkl.core.*;
import org.pkl.core.module.*;
import org.pkl.core.resource.ResourceReader;

public final class LosslessHostProbe {
  static Path work, out;
  static String mode, cause;
  static int resolves, sources, reads;
  static final String PAYLOAD = new String(new char[] {
    '\ud83d', '|', '\udc00', '|', '\ud83d', '\ude00', '|', '?', '|', '\ufffd', '|', '\u0000'
  });
  static void bytes(String name, byte[] value) throws IOException { Files.write(out.resolve(name), value); }
  static void text(String name, String value) throws IOException { bytes(name, value.getBytes(StandardCharsets.UTF_8)); }
  static void units(String name, String value) throws IOException {
    try (var data = new DataOutputStream(Files.newOutputStream(out.resolve(name)))) {
      for (int i = 0; i < value.length(); i++) data.writeChar(value.charAt(i));
    }
  }
  static void capture(Evaluator evaluator, String expression, int index) throws IOException {
    String prefix = "call-" + index;
    text(prefix + ".expression.bin", expression);
    try {
      String value = evaluator.evaluateExpressionString(ModuleSource.path(work.resolve("main.pkl")), expression);
      text(prefix + ".status.bin", "success\n"); text(prefix + ".stdout.bin", value);
      units(prefix + ".stdout.code-units.bin", value); bytes(prefix + ".stderr.bin", new byte[0]);
      bytes(prefix + ".stderr.code-units.bin", new byte[0]); bytes(prefix + ".error-class.bin", new byte[0]);
    } catch (RuntimeException failure) {
      text(prefix + ".status.bin", "error\n"); bytes(prefix + ".stdout.bin", new byte[0]);
      bytes(prefix + ".stdout.code-units.bin", new byte[0]);
      text(prefix + ".stderr.bin", failure.getMessage()); units(prefix + ".stderr.code-units.bin", failure.getMessage());
      text(prefix + ".error-class.bin", failure.getClass().getName());
      var stack = new StringWriter(); failure.printStackTrace(new PrintWriter(stack)); text(prefix + ".java-stack.bin", stack.toString());
    }
  }
  static final class Key implements ModuleKey {
    final URI uri;
    Key(URI uri) { this.uri = uri; }
    public URI getUri() { return uri; }
    public boolean hasHierarchicalUris() { return true; }
    public boolean isGlobbable() { return false; }
    public ResolvedModuleKey resolve(org.pkl.core.SecurityManager security) throws IOException, SecurityManagerException {
      security.checkResolveModule(uri);
      if (++resolves == 1 && mode.equals("module-resolve")) throw new IOException(cause);
      return new ResolvedModuleKey() {
        public ModuleKey getOriginal() { return Key.this; }
        public URI getUri() { return uri; }
        public String loadSource() throws IOException {
          if (++sources == 1 && mode.equals("module-source")) throw new IOException(cause);
          return "value = 9\n";
        }
      };
    }
  }
  public static void main(String[] args) throws Exception {
    work = Path.of(args[0]); out = Path.of(args[1]); mode = args[2]; Files.createDirectories(out);
    cause = switch (args[3]) { case "null" -> null; case "empty" -> ""; default -> PAYLOAD; };
    text("cause-null.bin", Boolean.toString(cause == null));
    if (cause != null) { text("cause.bin", cause); units("cause.code-units.bin", cause); }
    var builder = EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false)
      .setEnvironmentVariables(Map.of()).setExternalProperties(Map.of()).setPowerAssertionsEnabled(true);
    if (mode.equals("unknown-format")) builder.setOutputFormat(PAYLOAD);
    var factories = new ArrayList<ModuleKeyFactory>();
    factories.add(uri -> Optional.ofNullable("lossless".equals(uri.getScheme()) ? new Key(uri) : null));
    factories.addAll(builder.getModuleKeyFactories()); builder.setModuleKeyFactories(factories);
    var modules = new ArrayList<Pattern>(SecurityManagers.defaultAllowedModules); modules.add(Pattern.compile("lossless:")); builder.setAllowedModules(modules);
    var resources = new ArrayList<Pattern>(SecurityManagers.defaultAllowedResources); resources.add(Pattern.compile("lossless:")); builder.setAllowedResources(resources);
    builder.addResourceReader(new ResourceReader() {
      public String getUriScheme() { return "lossless"; }
      public boolean hasHierarchicalUris() { return true; }
      public boolean isGlobbable() { return false; }
      public Optional<Object> read(URI uri) throws IOException {
        if (++reads == 1) throw new IOException(cause);
        return Optional.of("9");
      }
    });
    try (var evaluator = builder.build()) {
      int index = 0;
      for (String expression : Files.readAllLines(work.resolve("expressions.txt"))) capture(evaluator, expression, index++);
    }
    text("counters.bin", "resolve=" + resolves + "\nsource=" + sources + "\nread=" + reads + "\n");
  }
}

// Development-only pinned Java observation helper; never production evaluation.
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.net.URI;
import java.io.*;
import java.util.*;
import java.util.regex.Pattern;
import org.pkl.core.*;
import org.pkl.core.module.*;

public final class LoaderProbe {
  static Path work, out;
  static String mode;
  static Evaluator evaluator;
  static int event, callback, resolveCount, loadCount, call;
  static void bytes(String name, byte[] value) throws IOException { Files.write(out.resolve(name), value); }
  static void string(String name, String value) throws IOException { bytes(name, value.getBytes(StandardCharsets.UTF_8)); }
  static void event(String message) throws IOException {
    string("event-" + (event++) + ".bin", message);
    System.out.println(message);
  }
  static void evaluate(String prefix, String expression) throws Exception {
    string(prefix + ".expression.bin", expression);
    try {
      var result = evaluator.evaluateExpressionString(ModuleSource.path(work.resolve("main.pkl")), expression);
      string(prefix + ".status.bin", "success\n");
      string(prefix + ".stdout.bin", result);
      bytes(prefix + ".stderr.bin", new byte[0]);
      bytes(prefix + ".error-class.bin", new byte[0]);
      event(prefix + " success");
    } catch (RuntimeException e) {
      string(prefix + ".status.bin", "error\n"); bytes(prefix + ".stdout.bin", new byte[0]);
      string(prefix + ".stderr.bin", e.getMessage());
      string(prefix + ".error-class.bin", e.getClass().getName());
      var stack = new StringWriter(); e.printStackTrace(new PrintWriter(stack)); string(prefix + ".java-stack.bin", stack.toString());
      event(prefix + " error " + e.getClass().getName());
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
      int attempt = ++resolveCount;
      event("resolve " + uri + " attempt=" + attempt);
      if (mode.equals("resolve-failure") && attempt == 1) throw new FileNotFoundException("resolve-first");
      URI canonical = mode.equals("alias") ? URI.create("life:/canonical") : uri;
      security.checkResolveModule(canonical);
      return new ResolvedModuleKey() {
        public ModuleKey getOriginal() { return Key.this; }
        public URI getUri() { return canonical; }
        public String loadSource() throws IOException {
          int attempt = ++loadCount;
          event("source-start " + canonical + " attempt=" + attempt);
          if ((mode.equals("source-failure") && attempt == 1) || (mode.equals("source-reentry-inner-failure") && attempt == 2)) throw new IOException("source-first");
          if (mode.startsWith("source-reentry") && callback++ == 0) {
            try { evaluate("inner-0", Files.readString(work.resolve("callback.txt"))); }
            catch (Exception e) { throw new IOException("callback-infrastructure", e); }
          }
          if (mode.startsWith("source-close")) { evaluator.close(); event("source-close");
            if (mode.equals("source-close-reentry")) { try { evaluate("inner-0", Files.readString(work.resolve("callback.txt"))); } catch(Exception e) { throw new IOException(e); } }
          }
          if (mode.equals("source-reentry-outer-failure") && attempt == 1) throw new IOException("outer-source-after-inner");
          if (mode.equals("source-timeout")) { try { Thread.sleep(4000); } catch (InterruptedException e) { throw new IOException(e); } }
          String value = Files.readString(work.resolve("source.pkl"));
          event("source-end " + canonical + " attempt=" + attempt);
          return value;
        }
      };
    }
  }
  public static void main(String[] args) throws Exception {
    work = Path.of(args[0]); out = Path.of(args[1]); mode = args[2]; Files.createDirectories(out);
    var builder = EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false)
      .setEnvironmentVariables(Map.of()).setExternalProperties(Map.of());
    var factories = new ArrayList<ModuleKeyFactory>();
    factories.add(uri -> { if (!"life".equals(uri.getScheme())) return Optional.empty(); event("factory " + uri); return Optional.of(new Key(uri)); });
    factories.addAll(builder.getModuleKeyFactories()); builder.setModuleKeyFactories(factories);
    var allowed = new ArrayList<Pattern>(SecurityManagers.defaultAllowedModules); allowed.add(Pattern.compile("life:")); builder.setAllowedModules(allowed);
    if (mode.equals("source-timeout")) builder.setTimeout(java.time.Duration.ofSeconds(3));
    builder.setLogger(new Logger() { public void trace(String message, StackFrame frame) { try {
      event("trace " + message + " @" + frame.getModuleUri());
      if (mode.equals("init-reentry") && callback++ == 0) evaluate("inner-0", Files.readString(work.resolve("callback.txt")));
    } catch(Exception e) {throw new RuntimeException(e);} } });
    evaluator = builder.build();
    try {
      for (var line : Files.readAllLines(work.resolve("plan.tsv"))) {
        var parts = line.split("\t", -1);
        if (parts[0].equals("eval")) evaluate("outer-" + (call++), new String(Base64.getDecoder().decode(parts[1]), StandardCharsets.UTF_8));
        else if (parts[0].equals("write")) { Files.write(work.resolve(parts[1]), Base64.getDecoder().decode(parts[2])); event("write " + parts[1]); }
        else throw new IllegalArgumentException("Unknown action " + parts[0]);
      }
    } finally { evaluator.close(); }
    string("counters.bin", "resolve=" + resolveCount + "\nsource=" + loadCount + "\ncallback=" + callback + "\n");
  }
}

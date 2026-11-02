// Development-only cross-module Pkl 0.32 Java oracle.
import java.io.DataOutputStream;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Optional;
import org.pkl.core.EvaluatorBuilder;
import org.pkl.core.ModuleSource;
import org.pkl.core.PklBugException;
import org.pkl.core.PklException;
import org.pkl.core.module.ModuleKey;
import org.pkl.core.module.ModuleKeyFactory;
import org.pkl.core.module.ResolvedModuleKey;
import org.pkl.core.module.ResolvedModuleKeys;

public final class M2CrossSourceOracle {
  static final class Factory implements ModuleKeyFactory {
    final Map<URI, String> modules;
    Factory(Map<URI, String> modules) { this.modules = modules; }
    public Optional<ModuleKey> create(URI uri) {
      if (!modules.containsKey(uri)) return Optional.empty();
      return Optional.of(new ModuleKey() {
        public URI getUri() { return uri; }
        public boolean hasHierarchicalUris() { return true; }
        public boolean isGlobbable() { return false; }
        public ResolvedModuleKey resolve(org.pkl.core.SecurityManager manager)
            throws org.pkl.core.SecurityManagerException {
          manager.checkResolveModule(uri);
          return ResolvedModuleKeys.virtual(this, uri, modules.get(uri), true);
        }
      });
    }
  }
  static void text(Path out, String name, String value) throws Exception {
    Files.write(out.resolve(name + ".bin"), value.getBytes(StandardCharsets.UTF_8));
    try (var stream = new DataOutputStream(Files.newOutputStream(out.resolve(name + ".code-units.bin")))) {
      for (int i = 0; i < value.length(); i++) stream.writeChar(value.charAt(i));
    }
  }
  public static void main(String[] args) throws Exception {
    if (args.length != 4 || !(args[3].equals("on") || args[3].equals("off")))
      throw new IllegalArgumentException("OUTPUT INPUT MAIN_URI on|off");
    Path out = Path.of(args[0]); Path input = Path.of(args[1]); URI uri = URI.create(args[2]);
    var modules = new LinkedHashMap<URI, String>();
    for (String name : new String[] {"defs.pkl", "forward.pkl"}) {
      if (Files.exists(input.resolve(name)))
        modules.put(uri.resolve(name), Files.readString(input.resolve(name), StandardCharsets.UTF_8));
    }
    var builder = EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false)
        .setPowerAssertionsEnabled(args[3].equals("on"))
        .setEnvironmentVariables(Map.of()).setExternalProperties(Map.of());
    var factories = new ArrayList<>(builder.getModuleKeyFactories());
    factories.addFirst(new Factory(modules)); builder.setModuleKeyFactories(factories);
    var source = ModuleSource.create(uri, Files.readString(input.resolve("main.pkl"), StandardCharsets.UTF_8));
    Files.createDirectory(out);
    try (var evaluator = builder.build()) {
      String status = "success\n", stdout = "", stderr = "", errorClass = "";
      try { stdout = evaluator.evaluateExpressionString(source, "output.text"); }
      catch (PklException error) {
        status = error instanceof PklBugException ? "oracle-error\n" : "error\n";
        stderr = error.getMessage();
        if (stderr == null) throw new IllegalStateException("null PklException message", error);
        errorClass = error.getClass().getName();
      }
      Files.write(out.resolve("result.status.bin"), status.getBytes(StandardCharsets.UTF_8));
      text(out, "result.stdout", stdout); text(out, "result.stderr", stderr);
      text(out, "result.error-class", errorClass);
      text(out, "expression", "output.text");
    }
  }
}

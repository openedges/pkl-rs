// Development-only observations through public Java evaluation/module/logger APIs.
import java.io.DataOutputStream;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Optional;
import org.pkl.core.*;
import org.pkl.core.module.*;

public final class M2ClassValuesProbe {
  static Path out;
  static Evaluator evaluator;
  static ModuleSource source;
  static String action, callback;
  static int logs, warnings, depth, callbackAfter;
  static boolean callbackUsed, throwUsed, closeUsed;
  static int providers;

  static void provider(String kind, URI uri, boolean matched) {
    int index = providers++;
    try {
      string("provider/event-" + index + ".uri", uri.toString());
      write("provider/event-" + index + ".kind.bin", kind);
      write("provider/event-" + index + ".matched.bin", String.valueOf(matched));
      Files.writeString(out.resolve("provider/protocol.bin"), "provider=" + index + " kind=" + kind
          + " matched=" + matched + "\n", StandardCharsets.UTF_8,
          java.nio.file.StandardOpenOption.CREATE, java.nio.file.StandardOpenOption.APPEND);
    } catch (Exception exception) { throw new RuntimeException(exception); }
  }

  static void write(String name, String value) throws Exception {
    Files.write(out.resolve(name), value.getBytes(StandardCharsets.UTF_8));
  }

  static void string(String name, String value) throws Exception {
    write(name + ".bin", value);
    try (var stream = new DataOutputStream(Files.newOutputStream(out.resolve(name + ".code-units.bin")))) {
      for (int i = 0; i < value.length(); i++) stream.writeChar(value.charAt(i));
    }
  }

  static void capture(String expression, String prefix) {
    String value = "", error = "", errorClass = "", status = "success\n";
    System.out.println("call-enter=" + prefix);
    try {
      value = evaluator.evaluateExpressionString(source, expression);
    } catch (PklException exception) {
      status = exception instanceof PklBugException ? "oracle-error\n" : "error\n";
      error = exception.getMessage();
      errorClass = exception.getClass().getName();
    } catch (Throwable exception) {
      status = "api-exception\n";
      error = exception.toString();
      errorClass = exception.getClass().getName();
    }
    try {
      string(prefix + ".stdout", value);
      string(prefix + ".stderr", error);
      write(prefix + ".status.bin", status);
      write(prefix + ".error-class.bin", errorClass);
    } catch (Exception exception) {
      throw new RuntimeException(exception);
    }
    System.out.println("call-exit=" + prefix + " status=" + status.strip());
  }

  static final class CaptureLogger implements Logger {
    public void trace(String message, StackFrame frame) { log("trace", message, frame); }
    public void warn(String message, StackFrame frame) { log("warn", message, frame); }
    void log(String level, String message, StackFrame frame) {
      int index = logs++;
      if (level.equals("warn")) warnings++;
      String prefix = "log-" + index;
      System.out.println("log-enter=" + index + " level=" + level + " depth=" + depth);
      try {
        write(prefix + ".level.bin", level);
        string(prefix + ".message", message);
        write(prefix + ".message.length.bin", String.valueOf(message.length()));
        string(prefix + ".uri", frame.getModuleUri());
        string(prefix + ".member", frame.getMemberName() == null ? "" : frame.getMemberName());
        write(prefix + ".member.null.bin", String.valueOf(frame.getMemberName() == null));
        write(prefix + ".position.bin", frame.getStartLine() + ":" + frame.getStartColumn()
            + ":" + frame.getEndLine() + ":" + frame.getEndColumn());
        write(prefix + ".source-count.bin", String.valueOf(frame.getSourceLines().size()));
        for (int line = 0; line < frame.getSourceLines().size(); line++)
          string(prefix + ".source-" + line, frame.getSourceLines().get(line));
      } catch (Exception exception) { throw new RuntimeException(exception); }
      depth++;
      try {
        if (!callback.isEmpty() && !callbackUsed && index >= callbackAfter) {
          callbackUsed = true;
          capture(callback, "log-callback-" + index);
        }
        // Real nested callbacks can consume the action before this entry resumes.
        if (index >= callbackAfter && action.equals("throw-once") && !throwUsed) {
          throwUsed = true;
          System.out.println("log-throw=" + index);
          throw new IllegalStateException("stage56-logger-failure");
        }
        if (index >= callbackAfter && action.equals("close") && !closeUsed) {
          closeUsed = true;
          evaluator.close();
          System.out.println("logger-close=" + index);
        }
      } finally { depth--; }
      System.out.println("log-exit=" + index);
    }
  }

  static final class SupportFactory implements ModuleKeyFactory {
    final Map<URI, String> modules;
    SupportFactory(Map<URI, String> modules) { this.modules = modules; }
    public Optional<ModuleKey> create(URI uri) {
      provider("create", uri, modules.containsKey(uri));
      if (!modules.containsKey(uri)) return Optional.empty();
      return Optional.of(new ModuleKey() {
        public URI getUri() { return uri; }
        public boolean hasHierarchicalUris() { return true; }
        public boolean isGlobbable() { return false; }
        public ResolvedModuleKey resolve(org.pkl.core.SecurityManager securityManager)
            throws org.pkl.core.SecurityManagerException {
          provider("resolve", uri, true);
          securityManager.checkResolveModule(uri);
          return ResolvedModuleKeys.virtual(this, uri, modules.get(uri), true);
        }
      });
    }
  }

  static Evaluator build(String loggerMode, Map<URI, String> modules) {
    var builder = EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false)
        .setPowerAssertionsEnabled(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of());
    var factories = new ArrayList<>(builder.getModuleKeyFactories());
    factories.addFirst(new SupportFactory(modules));
    builder.setModuleKeyFactories(factories);
    if (loggerMode.equals("capture")) builder.setLogger(new CaptureLogger());
    return builder.build();
  }

  public static void main(String[] args) throws Exception {
    if (args.length != 6) throw new IllegalArgumentException("DIRECTORY URI LOGGER_MODE LOGGER_ACTION CALLBACK CALLBACK_AFTER");
    out = Path.of(args[0]); action = args[3]; callback = args[4]; callbackAfter = Integer.parseInt(args[5]);
    Files.createDirectory(out.resolve("provider"));
    write("provider/protocol.bin", "");
    write("provider/key-configuration.bin", "hierarchical=true\nglobbable=false\ncached=true\nlocal=false\nresolved-uri=original-uri\nsecurity=checkResolveModule\n");
    var modules = new LinkedHashMap<URI, String>();
    for (String line : Files.readAllLines(out.resolve("modules.tsv"))) {
      String[] parts = line.split("\t", -1);
      if (parts.length != 2) throw new IllegalArgumentException("invalid authored support module mapping");
      modules.put(URI.create(parts[0]), Files.readString(out.resolve(parts[1])));
    }
    try (var eval = build(args[2], modules)) {
      evaluator = eval;
      source = ModuleSource.create(URI.create(args[1]), Files.readString(out.resolve("input.pkl")));
      var expressions = Files.readAllLines(out.resolve("expressions.txt"));
      for (int index = 0; index < expressions.size(); index++) capture(expressions.get(index), "outer-" + index);
    }
    System.out.println("closed logs=" + logs + " warnings=" + warnings);
  }
}

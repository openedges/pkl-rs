// Development-only source-driven public Java Logger observations. No Rust expectations.
import java.io.DataOutputStream;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;
import org.pkl.core.*;

public final class M2DeprecationProbe {
  static Path out;
  static Evaluator evaluator;
  static ModuleSource source;
  static String action, callback;
  static int logs, warnings, depth, callbackAfter;
  static boolean callbackUsed;

  static void write(String name, String value) throws Exception {
    Files.write(out.resolve(name), value.getBytes(StandardCharsets.UTF_8));
  }

  static void units(String name, String value) throws Exception {
    try (var stream = new DataOutputStream(Files.newOutputStream(out.resolve(name)))) {
      for (int i = 0; i < value.length(); i++) stream.writeChar(value.charAt(i));
    }
  }

  static void string(String name, String value) throws Exception {
    write(name + ".bin", value);
    units(name + ".code-units.bin", value);
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
      // Public callback/lifecycle exceptions are recorded separately from guest failures.
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
      int warning = level.equals("warn") ? warnings++ : -1;
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
      } catch (Exception exception) {
        throw new RuntimeException(exception);
      }
      depth++;
      try {
        if (level.equals("warn")) {
          if (depth == 1 && !callback.isEmpty() && !callbackUsed && warning >= callbackAfter) {
            callbackUsed = true;
            capture(callback, "log-callback-" + index);
          }
          if (action.equals("throw") || (action.equals("throw-once") && warning == 0))
            throw new IllegalStateException("stage54-logger-failure");
          if (action.startsWith("close")) evaluator.close();
          if (action.equals("close-reenter")) capture("stable", "post-close-callback-" + index);
        }
      } finally {
        depth--;
      }
      System.out.println("log-exit=" + index);
    }
  }

  public static void main(String[] args) throws Exception {
    if (args.length < 4 || args.length > 5)
      throw new IllegalArgumentException("DIRECTORY URI ACTION CALLBACK [CALLBACK_AFTER]");
    out = Path.of(args[0]); action = args[2]; callback = args[3];
    callbackAfter = args.length == 5 ? Integer.parseInt(args[4]) : 0;
    var builder = EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false)
        .setPowerAssertionsEnabled(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of());
    if (!action.equals("noop")) builder.setLogger(new CaptureLogger());
    try (var eval = builder.build()) {
      evaluator = eval;
      source = ModuleSource.create(URI.create(args[1]), Files.readString(out.resolve("input.pkl")));
      var expressions = Files.readAllLines(out.resolve("expressions.txt"));
      for (int i = 0; i < expressions.size(); i++) capture(expressions.get(i), "outer-" + i);
    }
    System.out.println("closed logs=" + logs + " warnings=" + warnings);
  }
}

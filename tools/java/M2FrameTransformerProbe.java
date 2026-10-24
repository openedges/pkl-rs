// Development-only source-driven public Java frame transformer observations.
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.net.URI;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import org.pkl.core.*;

public final class M2FrameTransformerProbe {
  static Path out;
  static Evaluator evaluator;
  static ModuleSource source;
  static String operation, transformerMode, transformerAction, transformerCallback;
  static String loggerMode, loggerAction, loggerCallback;
  static int transformerAfter, loggerAfter, transforms, logs, warnings, depth;
  static boolean transformerCallbackUsed, loggerCallbackUsed, transformerThrown, loggerThrown;
  static StackFrame replacement;

  static void write(String name, String value) throws Exception {
    Files.write(out.resolve(name), value.getBytes(StandardCharsets.UTF_8));
  }

  static void string(String name, String value) throws Exception {
    write(name + ".bin", value);
    try (var stream = new DataOutputStream(Files.newOutputStream(out.resolve(name + ".code-units.bin")))) {
      for (int i = 0; i < value.length(); i++) stream.writeChar(value.charAt(i));
    }
  }

  static String readUnits(String name) throws Exception {
    var path = out.resolve("replacement").resolve(name + ".u16be");
    var size = Files.size(path);
    if (size % 2 != 0) throw new IllegalArgumentException("odd replacement UTF16 byte count");
    var result = new StringBuilder();
    try (var stream = new DataInputStream(Files.newInputStream(path))) {
      for (long index = 0; index < size / 2; index++) result.append(stream.readChar());
    }
    return result.toString();
  }

  static StackFrame readReplacement() throws Exception {
    var directory = out.resolve("replacement");
    var position = Files.readString(directory.resolve("position.txt")).split(":", -1);
    if (position.length != 4) throw new IllegalArgumentException("expected four frame positions");
    var lines = new ArrayList<String>();
    int count = Integer.parseInt(Files.readString(directory.resolve("source-count.txt")));
    for (int i = 0; i < count; i++) lines.add(readUnits("source-" + i));
    var memberNull = Files.readString(directory.resolve("member-null.txt"));
    if (!memberNull.equals("true") && !memberNull.equals("false"))
      throw new IllegalArgumentException("expected nullable member marker");
    return new StackFrame(readUnits("uri"), memberNull.equals("true") ? null : readUnits("member"),
        List.copyOf(lines), Integer.parseInt(position[0]), Integer.parseInt(position[1]),
        Integer.parseInt(position[2]), Integer.parseInt(position[3]));
  }

  static void frame(String prefix, StackFrame frame) throws Exception {
    string(prefix + ".uri", frame.getModuleUri());
    string(prefix + ".member", frame.getMemberName() == null ? "" : frame.getMemberName());
    write(prefix + ".member.null.bin", String.valueOf(frame.getMemberName() == null));
    write(prefix + ".position.bin", frame.getStartLine() + ":" + frame.getStartColumn()
        + ":" + frame.getEndLine() + ":" + frame.getEndColumn());
    write(prefix + ".source-count.bin", String.valueOf(frame.getSourceLines().size()));
    for (int line = 0; line < frame.getSourceLines().size(); line++)
      string(prefix + ".source-" + line, frame.getSourceLines().get(line));
  }

  static void testResult(String prefix, TestResults result) throws Exception {
    string(prefix + ".test.module-name", result.moduleName());
    string(prefix + ".test.display-uri", result.displayUri());
    string(prefix + ".test.logs", result.logs());
    write(prefix + ".test.counts.bin", result.totalTests() + ":" + result.totalFailures()
        + ":" + result.totalAsserts() + ":" + result.totalAssertsFailed());
    write(prefix + ".test.failed.bin", String.valueOf(result.failed()));
    int errors = 0;
    if (result.error() != null) {
      string(prefix + ".test.error-" + errors + ".message", String.valueOf(result.error().message()));
      string(prefix + ".test.error-" + errors + ".exception", result.error().exception().getMessage());
      write(prefix + ".test.error-" + errors++ + ".error-class.bin", result.error().exception().getClass().getName());
    }
    for (var section : List.of(result.facts(), result.examples())) {
      for (var group : section.results()) {
        for (var error : group.errors()) {
          string(prefix + ".test.error-" + errors + ".message", String.valueOf(error.message()));
          string(prefix + ".test.error-" + errors + ".exception", error.exception().getMessage());
          write(prefix + ".test.error-" + errors++ + ".error-class.bin", error.exception().getClass().getName());
        }
      }
    }
    write(prefix + ".test.error-count.bin", String.valueOf(errors));
  }

  static void capture(String expression, String prefix, boolean test) {
    String value = "", error = "", errorClass = "", status = "success\n";
    System.out.println("call-enter=" + prefix);
    try {
      if (test) testResult(prefix, evaluator.evaluateTest(source, false));
      else value = evaluator.evaluateExpressionString(source, expression);
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

  static StackFrame transform(StackFrame original) {
    int index = transforms++;
    System.out.println("transform-enter=" + index + " depth=" + depth);
    depth++;
    try {
      frame("transform-" + index + ".input", original);
      if (!transformerCallbackUsed && !transformerCallback.isEmpty() && index >= transformerAfter) {
        transformerCallbackUsed = true;
        capture(transformerCallback, "transform-callback-" + index, false);
      }
      if (transformerAction.equals("throw-once") && !transformerThrown && index >= transformerAfter) {
        transformerThrown = true;
        System.out.println("transform-throw=" + index);
        throw new IllegalStateException("stage55-transformer-failure");
      }
      var transformed = switch (transformerMode) {
        case "identity" -> StackFrameTransformers.empty.apply(original);
        case "full" -> replacement;
        case "numbered" -> replacement.withModuleUri(replacement.getModuleUri() + "#" + index);
        default -> throw new IllegalArgumentException("unsupported observed transformer");
      };
      frame("transform-" + index + ".output", transformed);
      System.out.println("transform-exit=" + index);
      return transformed;
    } catch (RuntimeException exception) {
      throw exception;
    } catch (Exception exception) {
      throw new RuntimeException(exception);
    } finally {
      depth--;
    }
  }

  static final class CaptureLogger implements Logger {
    public void trace(String message, StackFrame frame) { log("trace", message, frame); }
    public void warn(String message, StackFrame frame) { log("warn", message, frame); }
    void log(String level, String message, StackFrame frame) {
      int index = logs++;
      if (level.equals("warn")) warnings++;
      System.out.println("log-enter=" + index + " level=" + level + " depth=" + depth);
      depth++;
      try {
        write("log-" + index + ".level.bin", level);
        string("log-" + index + ".message", message);
        write("log-" + index + ".message.length.bin", String.valueOf(message.length()));
        frame("log-" + index, frame);
        if (!loggerCallbackUsed && !loggerCallback.isEmpty() && index >= loggerAfter) {
          loggerCallbackUsed = true;
          capture(loggerCallback, "log-callback-" + index, false);
        }
        if (loggerAction.equals("throw-once") && !loggerThrown && index >= loggerAfter) {
          loggerThrown = true;
          System.out.println("log-throw=" + index);
          throw new IllegalStateException("stage55-logger-failure");
        }
        System.out.println("log-exit=" + index);
      } catch (RuntimeException exception) {
        throw exception;
      } catch (Exception exception) {
        throw new RuntimeException(exception);
      } finally {
        depth--;
      }
    }
  }

  public static void main(String[] args) throws Exception {
    if (args.length != 11) throw new IllegalArgumentException("DIRECTORY URI OPERATION TMODE TACTION TCALLBACK TAFTER LMODE LACTION LCALLBACK LAFTER");
    out = Path.of(args[0]); operation = args[2]; transformerMode = args[3]; transformerAction = args[4];
    transformerCallback = args[5]; transformerAfter = Integer.parseInt(args[6]); loggerMode = args[7];
    loggerAction = args[8]; loggerCallback = args[9]; loggerAfter = Integer.parseInt(args[10]);
    if (!List.of("expression", "test").contains(operation) || !List.of("default", "builtin-identity", "identity", "full", "numbered").contains(transformerMode)
        || !List.of("none", "throw-once").contains(transformerAction) || !List.of("none", "throw-once").contains(loggerAction)
        || !List.of("capture", "noop").contains(loggerMode)) throw new IllegalArgumentException("unsupported profile");
    if (List.of("full", "numbered").contains(transformerMode)) replacement = readReplacement();
    var builder = EvaluatorBuilder.preconfigured().setModuleCacheDir(null).setColor(false)
        .setPowerAssertionsEnabled(false).setEnvironmentVariables(Map.of()).setExternalProperties(Map.of())
        .setLogger(loggerMode.equals("noop") ? Loggers.noop() : new CaptureLogger());
    if (transformerMode.equals("builtin-identity")) builder.setStackFrameTransformer(StackFrameTransformers.empty);
    else if (!transformerMode.equals("default")) builder.setStackFrameTransformer(M2FrameTransformerProbe::transform);
    try (var eval = builder.build()) {
      evaluator = eval;
      source = ModuleSource.create(URI.create(args[1]), Files.readString(out.resolve("input.pkl")));
      if (operation.equals("expression")) {
        var expressions = Files.readAllLines(out.resolve("expressions.txt"));
        for (int i = 0; i < expressions.size(); i++) capture(expressions.get(i), "outer-" + i, false);
      } else {
        int calls = Integer.parseInt(Files.readString(out.resolve("calls.txt")));
        for (int i = 0; i < calls; i++) capture(null, "outer-" + i, true);
      }
    }
    System.out.println("closed transforms=" + transforms + " logs=" + logs + " warnings=" + warnings);
  }
}

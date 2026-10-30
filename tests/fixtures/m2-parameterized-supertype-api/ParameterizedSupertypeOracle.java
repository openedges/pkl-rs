import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import org.pkl.core.EvaluatorBuilder;
import org.pkl.core.ModuleSource;
import org.pkl.core.PklBugException;
import org.pkl.core.PklException;
import org.pkl.core.StackFrame;
import org.pkl.core.StackFrameTransformers;
import org.pkl.core.module.ModuleKeys;

/** Expected-free finite standard-library parameterized superclass observations. */
public final class ParameterizedSupertypeOracle {
  private static byte[] units(String value) {
    byte[] result = new byte[value.length() * 2];
    for (int i = 0; i < value.length(); i++) {
      result[i * 2] = (byte) (value.charAt(i) >>> 8);
      result[i * 2 + 1] = (byte) value.charAt(i);
    }
    return result;
  }

  private static void text(Path dir, String name, String value) throws Exception {
    Files.write(dir.resolve(name + ".bin"), value.getBytes(StandardCharsets.UTF_8));
    Files.write(dir.resolve(name + ".code-units.bin"), units(value));
  }

  private static void ascii(Path dir, String name, String value) throws Exception {
    Files.writeString(dir.resolve(name), value, StandardCharsets.UTF_8);
  }

  private static void nullable(Path dir, String name, String value) throws Exception {
    ascii(dir, name + ".present.bin", value == null ? "false\n" : "true\n");
    if (value != null) text(dir, name, value);
  }

  private static void frame(Path dir, StackFrame value) throws Exception {
    Files.createDirectory(dir);
    text(dir, "module-uri", value.getModuleUri());
    nullable(dir, "member-name", value.getMemberName());
    ascii(dir, "positions.tsv", value.getStartLine() + "\t" + value.getStartColumn()
        + "\t" + value.getEndLine() + "\t" + value.getEndColumn() + "\n");
    ascii(dir, "source-line-count.bin", value.getSourceLines().size() + "\n");
    for (int i = 0; i < value.getSourceLines().size(); i++) {
      text(dir, String.format("source-line-%04d", i), value.getSourceLines().get(i));
    }
  }

  private static final class Capture {
    Path row;
    Path setup;
    int setupFactories;
    int factories;
    int frames;
    int logs;

    void factory(URI uri, boolean selectedChild) {
      try {
        Path dir = row == null ? setup : row;
        int index = row == null ? setupFactories++ : factories++;
        if (index >= 256) throw new IllegalStateException("factory observation bound");
        text(dir, String.format("factory-%04d-uri", index), uri.toString());
        ascii(dir, String.format("factory-%04d-selected-child.bin", index), selectedChild + "\n");
      } catch (Exception error) {
        throw new IllegalStateException("observation write failed", error);
      }
    }

    StackFrame transform(StackFrame input, boolean identity) {
      try {
        int index = frames++;
        if (index >= 256) throw new IllegalStateException("frame observation bound");
        frame((row == null ? setup : row).resolve(String.format("frame-%04d-input", index)), input);
        StackFrame output = identity ? input : StackFrameTransformers.defaultTransformer.apply(input);
        frame((row == null ? setup : row).resolve(String.format("frame-%04d-output", index)), output);
        return output;
      } catch (Exception error) {
        throw new IllegalStateException("observation write failed", error);
      }
    }

    void log(String level, String message, StackFrame input) {
      try {
        if (logs >= 64) throw new IllegalStateException("logger observation bound");
        Path dir = row.resolve(String.format("logger-%04d", logs++));
        Files.createDirectory(dir);
        text(dir, "level", level);
        text(dir, "message", message);
        frame(dir.resolve("frame"), input);
      } catch (Exception error) {
        throw new IllegalStateException("observation write failed", error);
      }
    }
  }

  public static void main(String[] args) throws Exception {
    if (args.length != 2) throw new IllegalArgumentException("inputs and outputs required");
    Path inputs = Path.of(args[0]);
    Path outputs = Path.of(args[1]);
    Files.createDirectory(outputs);
    StringBuilder runtime = new StringBuilder();
    for (String name : new String[] {"java.version", "java.runtime.version", "java.vendor",
        "java.vm.name", "java.vm.version", "java.home", "file.encoding", "user.language",
        "user.country", "user.timezone"}) {
      runtime.append(name).append('\t').append(System.getProperty(name)).append('\n');
    }
    runtime.append("maxMemory\t").append(Runtime.getRuntime().maxMemory()).append('\n');
    ascii(outputs, "runtime.tsv", runtime.toString());
    List<String> programs = Files.readAllLines(inputs.resolve("programs.tsv"), StandardCharsets.UTF_8);
    List<String> profiles = Files.readAllLines(inputs.resolve("profiles.tsv"), StandardCharsets.UTF_8);
    if (programs.size() != 8 || !profiles.equals(List.of("default", "identity"))) {
      throw new IllegalArgumentException("finite protocol changed");
    }
    int observations = 0;
    for (String program : programs) {
      if (!program.matches("[a-z][a-z-]+")) throw new IllegalArgumentException("program ID");
      Path sourceDir = inputs.resolve(program);
      String source = Files.readString(sourceDir.resolve("module.pkl"), StandardCharsets.UTF_8);
      URI uri = URI.create(Files.readString(sourceDir.resolve("uri.txt"), StandardCharsets.UTF_8));
      List<String> queries = Files.readAllLines(sourceDir.resolve("queries.tsv"), StandardCharsets.UTF_8);
      if (queries.size() != 1) {
        throw new IllegalArgumentException("query count");
      }
      for (String profile : profiles) {
        Path session = outputs.resolve(program).resolve(profile);
        Files.createDirectories(session);
        Capture capture = new Capture();
        capture.setup = session;
        boolean identity = profile.equals("identity");
        var builder = EvaluatorBuilder.preconfigured()
            .setModuleCacheDir(null).setColor(false).setPowerAssertionsEnabled(false)
            .setEnvironmentVariables(Map.of()).setExternalProperties(Map.of())
            .setStackFrameTransformer(input -> capture.transform(input, identity))
            .setLogger(new org.pkl.core.Logger() {
              @Override public void trace(String message, StackFrame input) {
                capture.log("trace", message, input);
              }
              @Override public void warn(String message, StackFrame input) {
                capture.log("warn", message, input);
              }
            });
        try (var evaluator = builder.build()) {
          for (int round = 0; round < 2; round++) {
            for (String query : queries) {
              String[] columns = query.split("\t", -1);
              if (columns.length != 3 || !columns[0].matches("q[0-9]{4}")) {
                throw new IllegalArgumentException("query protocol");
              }
              capture.row = session.resolve("round-" + round).resolve(columns[0]);
              Files.createDirectories(capture.row);
              capture.frames = capture.factories = capture.logs = 0;
              text(capture.row, "expression", columns[2]);
              text(capture.row, "operation", columns[1]);
              try {
                ModuleSource module = ModuleSource.create(uri, source);
                String result = switch (columns[1]) {
                  case "expression" -> evaluator.evaluateExpressionString(module, columns[2]);
                  case "output-text" -> evaluator.evaluateOutputText(module);
                  default -> throw new IllegalArgumentException("operation");
                };
                ascii(capture.row, "status.bin", "success\n");
                text(capture.row, "string", result);
              } catch (Throwable error) {
                String status = error instanceof PklBugException ? "oracle-error"
                    : error instanceof PklException ? "error" : "api-exception";
                ascii(capture.row, "status.bin", status + "\n");
                text(capture.row, "exception-class", error.getClass().getName());
                nullable(capture.row, "error", error.getMessage());
                text(capture.row, "exception-to-string", error.toString());
                Throwable cause = error.getCause();
                ascii(capture.row, "cause.present.bin", cause == null ? "false\n" : "true\n");
                if (cause != null) {
                  text(capture.row, "cause-class", cause.getClass().getName());
                  nullable(capture.row, "cause-message", cause.getMessage());
                  text(capture.row, "cause-to-string", cause.toString());
                }
              }
              ascii(capture.row, "counts.tsv", capture.frames + "\t" + capture.logs + "\n");
              System.out.println(program + "\t" + profile + "\t" + round + "\t" + columns[0] + "\tcomplete");
              System.out.flush();
              observations++;
              capture.row = null;
            }
          }
        }
      }
    }
    if (observations != 32) throw new IllegalStateException("incomplete matrix");
  }
}

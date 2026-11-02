import java.io.PrintWriter;
import java.io.StringWriter;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.net.URLEncoder;
import java.nio.ByteBuffer;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.pkl.core.Release;
import org.pkl.core.runtime.VmBugException;
import org.pkl.core.runtime.VmEvalException;
import org.pkl.core.runtime.VmException;
import org.pkl.core.runtime.VmExceptionRenderer;
import org.pkl.core.stdlib.test.TestNodes;

/** Captures the pinned Java runtime's exception rendering without synthesized expectations. */
public final class M2JavaExceptionOracle {
  private record NodeSpec(int id, String className, String message, String localized,
                          String override, Integer cause) {}
  private record FrameSpec(int owner, String loader, String module, String version,
                           String className, String method, String file, int line,
                           boolean omitLoader, boolean omitVersion) {}
  private record Edge(int from, int to) {}
  private record Case(Map<Integer, NodeSpec> nodes, List<FrameSpec> frames, List<Edge> edges) {}

  private static final class ModelException extends RuntimeException {
    private final NodeSpec spec;
    ModelException(NodeSpec spec) { super(spec.message()); this.spec = spec; }
    @Override public String getLocalizedMessage() { return spec.localized(); }
    @Override public String toString() {
      if (spec.override() != null) return spec.override();
      return super.toString();
    }
  }

  private static String readUtf8(Path path) throws Exception {
    return StandardCharsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT)
        .onUnmappableCharacter(CodingErrorAction.REPORT)
        .decode(ByteBuffer.wrap(Files.readAllBytes(path))).toString();
  }

  private static List<String> lines(Path path) throws Exception {
    String content = readUtf8(path);
    if (content.isEmpty() || !content.endsWith("\n") || content.indexOf('\r') >= 0)
      throw new IllegalArgumentException("expected nonempty LF-terminated file: " + path);
    return List.of(content.substring(0, content.length() - 1).split("\n", -1));
  }

  private static int id(String text) {
    if (!text.matches("0|[1-9][0-9]*")) throw new IllegalArgumentException("invalid ID: " + text);
    return Integer.parseInt(text);
  }

  private static String string(String text, boolean optional) {
    if (optional && text.equals("-")) return null;
    if (!text.matches("u(?:[0-9a-f]{4})*")) throw new IllegalArgumentException("invalid UTF-16 field: " + text);
    StringBuilder result = new StringBuilder();
    for (int i = 1; i < text.length(); i += 4)
      result.append((char) Integer.parseInt(text.substring(i, i + 4), 16));
    return result.toString();
  }

  private static boolean flag(String text) {
    if (text.equals("0")) return false;
    if (text.equals("1")) return true;
    throw new IllegalArgumentException("invalid flag: " + text);
  }

  private static Case parse(Path path) throws Exception {
    Map<Integer, NodeSpec> nodes = new LinkedHashMap<>();
    List<FrameSpec> frames = new ArrayList<>();
    List<Edge> edges = new ArrayList<>();
    for (String line : lines(path)) {
      String[] f = line.split("\t", -1);
      switch (f[0]) {
        case "throwable" -> {
          if (f.length != 7) throw new IllegalArgumentException("invalid throwable row");
          int id = id(f[1]);
          NodeSpec node = new NodeSpec(id, string(f[2], false), string(f[3], true),
              string(f[4], true), string(f[5], true), f[6].equals("-") ? null : id(f[6]));
          if (nodes.putIfAbsent(id, node) != null) throw new IllegalArgumentException("duplicate node " + id);
        }
        case "frame" -> {
          if (f.length != 11) throw new IllegalArgumentException("invalid frame row");
          frames.add(new FrameSpec(id(f[1]), string(f[2], true), string(f[3], true),
              string(f[4], true), string(f[5], false), string(f[6], false), string(f[7], true),
              Integer.parseInt(f[8]), flag(f[9]), flag(f[10])));
        }
        case "suppressed" -> {
          if (f.length != 3) throw new IllegalArgumentException("invalid suppressed row");
          edges.add(new Edge(id(f[1]), id(f[2])));
        }
        default -> throw new IllegalArgumentException("invalid row type: " + f[0]);
      }
    }
    NodeSpec root = nodes.get(0);
    if (root == null || root.message() == null || root.message().indexOf('%') >= 0)
      throw new IllegalArgumentException("root needs a nonnull message without format specifiers");
    if (!root.className().equals(VmBugException.class.getName())
        || !root.message().equals(root.localized()) || root.override() != null)
      throw new IllegalArgumentException("root model must describe the actual VmBugException");
    for (NodeSpec node : nodes.values()) {
      if (node.cause() != null && (!nodes.containsKey(node.cause()) || node.cause() == node.id()))
        throw new IllegalArgumentException("invalid cause for " + node.id());
    }
    for (FrameSpec frame : frames) if (!nodes.containsKey(frame.owner()))
      throw new IllegalArgumentException("unknown frame owner " + frame.owner());
    for (Edge edge : edges)
      if (!nodes.containsKey(edge.from()) || !nodes.containsKey(edge.to()) || edge.from() == edge.to())
        throw new IllegalArgumentException("invalid suppressed edge");
    return new Case(nodes, frames, edges);
  }

  private static void write(Path dir, String stem, String value) throws Exception {
    Files.write(dir.resolve(stem + ".bin"), value.getBytes(StandardCharsets.UTF_8));
    byte[] raw = new byte[value.length() * 2];
    for (int i = 0; i < value.length(); i++) {
      char c = value.charAt(i);
      raw[2 * i] = (byte) (c >>> 8);
      raw[2 * i + 1] = (byte) c;
    }
    Files.write(dir.resolve(stem + ".code-units.bin"), raw);
  }

  private static void capture(Case input, Path dir, Field format, Method caught) throws Exception {
    Map<Integer, Throwable> nodes = new LinkedHashMap<>();
    for (NodeSpec spec : input.nodes().values()) if (spec.id() != 0) {
      if (spec.className().equals(IllegalArgumentException.class.getName())) {
        if (!java.util.Objects.equals(spec.message(), spec.localized()) || spec.override() != null)
          throw new IllegalArgumentException("standard throwable has custom behavior");
        nodes.put(spec.id(), new IllegalArgumentException(spec.message()));
      } else if (spec.className().equals(ModelException.class.getName())) {
        nodes.put(spec.id(), new ModelException(spec));
      } else {
        throw new IllegalArgumentException("unimplemented oracle throwable class: " + spec.className());
      }
    }
    NodeSpec root = input.nodes().get(0);
    Throwable rootCause = root.cause() == null ? null : nodes.get(root.cause());
    VmBugException bug = new VmBugException(root.message(), rootCause, false, new Object[0],
        null, List.of(), null, null, null, null, Map.of(), List.of());
    nodes.put(0, bug);
    for (NodeSpec spec : input.nodes().values()) if (spec.id() != 0)
      nodes.get(spec.id()).initCause(spec.cause() == null ? null : nodes.get(spec.cause()));
    for (Edge edge : input.edges()) nodes.get(edge.from()).addSuppressed(nodes.get(edge.to()));
    Map<Integer, List<StackTraceElement>> stacks = new LinkedHashMap<>();
    for (int id : nodes.keySet()) stacks.put(id, new ArrayList<>());
    for (FrameSpec frame : input.frames()) {
      StackTraceElement element = new StackTraceElement(frame.loader(), frame.module(), frame.version(),
          frame.className(), frame.method(), frame.file(), frame.line());
      format.setByte(element, (byte) ((frame.omitLoader() ? 1 : 0) | (frame.omitVersion() ? 2 : 0)));
      stacks.get(frame.owner()).add(element);
    }
    for (Map.Entry<Integer, Throwable> entry : nodes.entrySet())
      entry.getValue().setStackTrace(stacks.get(entry.getKey()).toArray(StackTraceElement[]::new));
    VmEvalException ordinary = new VmEvalException(root.message(), rootCause, false, new Object[0],
        null, List.of(), null, null, null, null, Map.of(), List.of());
    ordinary.setStackTrace(new StackTraceElement[0]);
    VmExceptionRenderer renderer = new VmExceptionRenderer(null, false, false);
    Files.createDirectory(dir);
    write(dir, "ordinary", renderer.render(ordinary));
    write(dir, "bug", renderer.render(bug));
    VmExceptionRenderer colored = new VmExceptionRenderer(null, true, false);
    write(dir, "colored-ordinary", colored.render(ordinary));
    write(dir, "colored-bug", colored.render(bug));
    write(dir, "caught", (String) caught.invoke(null, bug));
    write(dir, "release", Release.current().versionInfo());
    write(dir, "line-separator", System.lineSeparator());
    for (Map.Entry<Integer, Throwable> entry : nodes.entrySet()) {
      int id = entry.getKey();
      Throwable node = entry.getValue();
      StringWriter buffer = new StringWriter();
      node.printStackTrace(new PrintWriter(buffer));
      write(dir, "trace-" + id, buffer.toString());
      write(dir, "description-" + id, node.toString());
      write(dir, "url-" + id, URLEncoder.encode(node.toString(), StandardCharsets.UTF_8).replace("+", "%20"));
      StackTraceElement[] stack = node.getStackTrace();
      for (int i = 0; i < stack.length; i++) write(dir, "frame-" + id + "-" + i, stack[i].toString());
    }
  }

  public static void main(String[] args) throws Exception {
    if (args.length != 2) throw new IllegalArgumentException("expected INPUT_CASE_DIRECTORY OUTPUT_DIRECTORY");
    Path input = Path.of(args[0]);
    Path output = Path.of(args[1]);
    Map<String, Case> cases = new LinkedHashMap<>();
    for (String name : lines(input.resolve("cases.txt"))) {
      if (!name.matches("[A-Za-z0-9][A-Za-z0-9_-]*") || cases.containsKey(name))
        throw new IllegalArgumentException("invalid or duplicate case ID: " + name);
      cases.put(name, parse(input.resolve(name + ".tsv")));
    }
    Field format = StackTraceElement.class.getDeclaredField("format");
    format.setAccessible(true);
    Method caught = TestNodes.class.getDeclaredMethod("render", VmException.class);
    caught.setAccessible(true);
    Files.createDirectory(output);
    for (Map.Entry<String, Case> entry : cases.entrySet())
      capture(entry.getValue(), output.resolve(entry.getKey()), format, caught);
  }
}

// Development-only pinned Java 0.32.0 PklBinaryDecoder oracle probe.
// Production evaluation never calls Java.
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.regex.Pattern;
import org.pkl.core.DataSize;
import org.pkl.core.Duration;
import org.pkl.core.PClassInfo;
import org.pkl.core.PModule;
import org.pkl.core.PNull;
import org.pkl.core.PObject;
import org.pkl.core.Pair;
import org.pkl.core.PklBinaryDecoder;

public final class M3PklBinaryDecodeProbe {
  private static void appendJsonString(StringBuilder sb, String value) {
    sb.append('"');
    for (int i = 0; i < value.length(); i++) {
      char c = value.charAt(i);
      switch (c) {
        case '"' -> sb.append("\\\"");
        case '\\' -> sb.append("\\\\");
        case '\b' -> sb.append("\\b");
        case '\f' -> sb.append("\\f");
        case '\n' -> sb.append("\\n");
        case '\r' -> sb.append("\\r");
        case '\t' -> sb.append("\\t");
        default -> {
          if (c < 0x20 || Character.isSurrogate(c)) {
            sb.append(String.format("\\u%04x", (int) c));
          } else {
            sb.append(c);
          }
        }
      }
    }
    sb.append('"');
  }

  private static String hexBytes(byte[] bytes) {
    StringBuilder sb = new StringBuilder(bytes.length * 2);
    for (byte b : bytes) {
      sb.append(String.format("%02x", b & 0xff));
    }
    return sb.toString();
  }

  private static String utf16Hex(String value) {
    StringBuilder sb = new StringBuilder(value.length() * 4);
    for (int i = 0; i < value.length(); i++) {
      sb.append(String.format("%04x", (int) value.charAt(i)));
    }
    return sb.toString();
  }

  private static void appendClassInfo(StringBuilder sb, PClassInfo<?> info) {
    sb.append("{\"moduleName\":");
    appendJsonString(sb, info.getModuleName());
    sb.append(",\"simpleName\":");
    appendJsonString(sb, info.getSimpleName());
    sb.append(",\"moduleUri\":");
    appendJsonString(sb, info.getModuleUri().toString());
    sb.append('}');
  }

  private static void appendValue(StringBuilder sb, Object value) {
    if (value == null || value instanceof PNull) {
      sb.append("{\"type\":\"null\"}");
    } else if (value instanceof Boolean b) {
      sb.append("{\"type\":\"bool\",\"value\":").append(b ? "true" : "false").append('}');
    } else if (value instanceof Long l) {
      sb.append("{\"type\":\"int\",\"value\":").append(l).append('}');
    } else if (value instanceof Double d) {
      sb.append("{\"type\":\"float\",\"bits\":\"")
          .append(String.format("%016x", Double.doubleToRawLongBits(d)))
          .append("\"}");
    } else if (value instanceof String s) {
      sb.append("{\"type\":\"string\",\"utf16_hex\":\"")
          .append(utf16Hex(s))
          .append("\",\"text\":");
      appendJsonString(sb, s);
      sb.append('}');
    } else if (value instanceof byte[] bytes) {
      sb.append("{\"type\":\"bytes\",\"hex\":\"").append(hexBytes(bytes)).append("\"}");
    } else if (value instanceof Duration dur) {
      sb.append("{\"type\":\"duration\",\"bits\":\"")
          .append(String.format("%016x", Double.doubleToRawLongBits(dur.getValue())))
          .append("\",\"unit\":");
      appendJsonString(sb, dur.getUnit().getSymbol());
      sb.append('}');
    } else if (value instanceof DataSize ds) {
      sb.append("{\"type\":\"datasize\",\"bits\":\"")
          .append(String.format("%016x", Double.doubleToRawLongBits(ds.getValue())))
          .append("\",\"unit\":");
      appendJsonString(sb, ds.getUnit().getSymbol());
      sb.append('}');
    } else if (value instanceof Pattern pat) {
      sb.append("{\"type\":\"regex\",\"pattern\":");
      appendJsonString(sb, pat.pattern());
      sb.append('}');
    } else if (value instanceof Pair<?, ?> pair) {
      sb.append("{\"type\":\"pair\",\"first\":");
      appendValue(sb, pair.getFirst());
      sb.append(",\"second\":");
      appendValue(sb, pair.getSecond());
      sb.append('}');
    } else if (value instanceof List<?> list) {
      sb.append("{\"type\":\"list\",\"elements\":[");
      boolean first = true;
      for (Object elem : list) {
        if (!first) sb.append(',');
        first = false;
        appendValue(sb, elem);
      }
      sb.append("]}");
    } else if (value instanceof Set<?> set) {
      sb.append("{\"type\":\"set\",\"elements\":[");
      boolean first = true;
      for (Object elem : set) {
        if (!first) sb.append(',');
        first = false;
        appendValue(sb, elem);
      }
      sb.append("]}");
    } else if (value instanceof Map<?, ?> map) {
      sb.append("{\"type\":\"map\",\"entries\":[");
      boolean first = true;
      for (Map.Entry<?, ?> entry : map.entrySet()) {
        if (!first) sb.append(',');
        first = false;
        sb.append('[');
        appendValue(sb, entry.getKey());
        sb.append(',');
        appendValue(sb, entry.getValue());
        sb.append(']');
      }
      sb.append("]}");
    } else if (value instanceof PModule mod) {
      sb.append("{\"type\":\"module\",\"uri\":");
      appendJsonString(sb, mod.getModuleUri().toString());
      sb.append(",\"name\":");
      appendJsonString(sb, mod.getModuleName());
      sb.append(",\"class\":");
      appendClassInfo(sb, mod.getClassInfo());
      sb.append(",\"properties\":[");
      boolean first = true;
      for (Map.Entry<String, Object> entry : mod.getProperties().entrySet()) {
        if (!first) sb.append(',');
        first = false;
        sb.append('[');
        appendJsonString(sb, entry.getKey());
        sb.append(',');
        appendValue(sb, entry.getValue());
        sb.append(']');
      }
      sb.append("]}");
    } else if (value instanceof PObject obj) {
      sb.append("{\"type\":\"object\",\"class\":");
      appendClassInfo(sb, obj.getClassInfo());
      sb.append(",\"properties\":[");
      boolean first = true;
      for (Map.Entry<String, Object> entry : obj.getProperties().entrySet()) {
        if (!first) sb.append(',');
        first = false;
        sb.append('[');
        appendJsonString(sb, entry.getKey());
        sb.append(',');
        appendValue(sb, entry.getValue());
        sb.append(']');
      }
      sb.append("]}");
    } else {
      throw new IllegalStateException("Unexpected decoded value type: " + value.getClass().getName());
    }
  }

  public static void main(String[] args) throws Exception {
    if (args.length != 1) {
      System.err.println("usage: M3PklBinaryDecodeProbe <msgpack-file>");
      System.exit(2);
    }
    byte[] bytes = Files.readAllBytes(Path.of(args[0]));
    try {
      Object decoded = PklBinaryDecoder.decode(bytes);
      StringBuilder sb = new StringBuilder();
      appendValue(sb, decoded);
      sb.append('\n');
      System.out.write(sb.toString().getBytes(StandardCharsets.UTF_8));
    } catch (RuntimeException error) {
      String outer = error.getMessage() != null ? error.getMessage() : "";
      Throwable cause = error.getCause();
      String inner = (cause != null && cause.getMessage() != null) ? cause.getMessage() : "";
      String payload = "error: " + outer + "\ncause: " + inner + "\n";
      System.err.write(payload.getBytes(StandardCharsets.UTF_8));
      System.exit(1);
    }
  }
}

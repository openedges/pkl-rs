// Development-only structural oracle; the production parser never invokes Java.
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import org.pkl.parser.Parser;
import org.pkl.parser.ParserError;

public final class M1FrontendProbe {
  public static void main(String[] args) throws Exception {
    String source = Files.readString(Path.of(args[1]), StandardCharsets.UTF_8);
    if (args[0].equals("positions")) {
      int utf16 = 0, byteIndex = 0, codepoint = 0, line = 1, column = 1;
      System.out.println("0\t0\t0\t1\t1");
      while (utf16 < source.length()) {
        int ch = source.codePointAt(utf16);
        int width = Character.charCount(ch);
        byteIndex += new String(Character.toChars(ch)).getBytes(StandardCharsets.UTF_8).length;
        utf16 += width;
        codepoint++;
        if (ch == '\n' || (ch == '\r' && (utf16 == source.length() || source.charAt(utf16) != '\n'))) {
          line++; column = 1;
        } else { column += width; }
        System.out.println(byteIndex + "\t" + utf16 + "\t" + codepoint + "\t" + line + "\t" + column);
      }
    } else if (args[0].equals("parse")) {
      try {
        var module = new Parser().parseModule(source);
        System.out.println("ok");
        for (var property : module.getProperties()) {
          var span = property.span();
          var expr = property.getExpr().span();
          System.out.println(property.getName().getValue() + "\t" + span.charIndex() + "\t" + span.length()
              + "\t" + expr.charIndex() + "\t" + expr.length());
        }
      } catch (ParserError error) {
        System.out.println("error\t" + error.span().charIndex() + "\t" + error.span().length());
        System.out.print(error.getMessage());
      }
    } else if (args[0].equals("value")) {
      try (var evaluator = org.pkl.core.EvaluatorBuilder.preconfigured()
          .setModuleCacheDir(null).setColor(false)
          .setEnvironmentVariables(java.util.Map.of())
          .setExternalProperties(java.util.Map.of()).build()) {
        System.out.print(evaluator.evaluateExpressionString(
            org.pkl.core.ModuleSource.path(Path.of(args[1])), "result"));
      }
    } else {
      throw new IllegalArgumentException("expected positions or parse");
    }
  }
}

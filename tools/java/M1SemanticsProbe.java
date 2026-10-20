// M1 primitive oracle. Uses the exact JDK verified by tools/oracle.py.
// Pkl 0.32.0 delegates these primitives to Double, StrictMath, and Pattern;
// this is not a replacement for full Pkl expression/diagnostic comparisons.
import java.io.*;
import java.nio.charset.StandardCharsets;
import java.util.HexFormat;
import java.util.regex.*;

public class M1SemanticsProbe {
  static final HexFormat HEX = HexFormat.of();
  static String unhex(String value) { return new String(HEX.parseHex(value), StandardCharsets.UTF_8); }
  static String hex(String value) { return HEX.formatHex(value.getBytes(StandardCharsets.UTF_8)); }
  static String numeric(String[] f) {
    if (f.length == 3) {
      long a = Long.parseLong(f[1]), b = Long.parseLong(f[2]);
      if ((f[0].equals("div") || f[0].equals("rem")) && b == 0) return "error:divisionByZero\n";
      try {
        long value = switch (f[0]) {
          case "add" -> StrictMath.addExact(a,b);
          case "sub" -> StrictMath.subtractExact(a,b);
          case "mul" -> StrictMath.multiplyExact(a,b);
          case "div" -> { if(a == Long.MIN_VALUE && b == -1) throw new ArithmeticException(); yield a/b; }
          case "rem" -> a%b;
          default -> throw new IllegalArgumentException(f[0]);
        };
        return value + "\n";
      } catch (ArithmeticException e) { return "error:integerOverflow\n"; }
    }
    double x = Double.longBitsToDouble(Long.parseUnsignedLong(f[1],16));
    if(f[0].equals("format")) return Double.toString(x) + "\n";
    double value = switch(f[0]) {
      case "sqrt" -> StrictMath.sqrt(x); case "sin" -> StrictMath.sin(x);
      case "cos" -> StrictMath.cos(x); case "tan" -> StrictMath.tan(x);
      case "exp" -> StrictMath.exp(x); case "log" -> StrictMath.log(x);
      case "cbrt" -> StrictMath.cbrt(x); case "rint" -> StrictMath.rint(x);
      case "ceil" -> StrictMath.ceil(x); case "floor" -> StrictMath.floor(x);
      default -> throw new IllegalArgumentException(f[0]);
    };
    return String.format(java.util.Locale.ROOT,"%016x\n", Double.doubleToLongBits(value));
  }
  static String regex(String[] f) {
    try {
      Pattern p = Pattern.compile(unhex(f[1]),Pattern.UNICODE_CASE);
      Matcher m = p.matcher(unhex(f[2]));
      if(f[0].equals("matches")) return m.matches() + "\n";
      if(f[0].equals("replace")) {
        try { return "text:" + hex(m.replaceAll(unhex(f[3]))) + "\n"; }
        catch(IllegalArgumentException | IndexOutOfBoundsException e) { return "error:" + hex(e.getMessage()) + "\n"; }
      }
      StringBuilder result = new StringBuilder();
      while(m.find()) {
        result.append("match:").append(m.start()).append(',').append(m.end()).append(',').append(hex(m.group())).append('\n');
        for(int i=1;i<=m.groupCount();i++) {
          if(m.group(i)==null) result.append("group:null\n");
          else result.append("group:").append(m.start(i)).append(',').append(m.end(i)).append(',').append(hex(m.group(i))).append('\n');
        }
      }
      return result.append("end\n").toString();
    } catch(PatternSyntaxException e) { return "error:" + hex(e.getMessage()) + "\n"; }
  }
  public static void main(String[] args) throws Exception {
    try(var reader = new BufferedReader(new InputStreamReader(System.in,StandardCharsets.UTF_8))) {
      String line;
      while((line=reader.readLine())!=null) {
        String[] parts = line.split("\t",2);
        String[] fields = parts[1].split("\t",-1);
        String output = parts[0].equals("numeric") ? numeric(fields) : regex(fields);
        System.out.println(hex(output));
      }
    }
  }
}

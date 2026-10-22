// Development-only full String.toUpperCase(Locale.ROOT) inventory from the pinned JDK.
import java.io.BufferedOutputStream;
import java.io.DataOutputStream;
import java.util.Locale;

public final class M2UppercaseProbe {
  public static void main(String[] args) throws Exception {
    try (var output = new DataOutputStream(new BufferedOutputStream(System.out))) {
      output.write(new byte[] {'P', 'K', 'L', 'U', 'P', 'R', '1', 0});
      output.writeInt(0x110000);
      // Character.toChars also accepts isolated surrogate code points. Keeping
      // those rows verifies their units before any UTF-8 writer conversion.
      for (int cp = 0; cp < 0x110000; cp++) {
        String upper = new String(Character.toChars(cp)).toUpperCase(Locale.ROOT);
        output.writeShort(upper.length());
        for (int i = 0; i < upper.length(); i++) output.writeChar(upper.charAt(i));
      }
    }
  }
}

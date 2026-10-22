// Development-only inventory of the pinned JDK predicates used for Unicode escapes.
public final class M2UnicodeEscapeProbe {
  public static void main(String[] args) {
    // Lexer.nextChar and Integer.parseInt consume UTF-16 units, not code points.
    int begin = -1;
    for (int unit = 0; unit <= 0x10000; unit++) {
      boolean present = unit < 0x10000 && Character.isLetterOrDigit((char) unit);
      if (present && begin == -1) begin = unit;
      if (!present && begin != -1) {
        System.out.printf(java.util.Locale.ROOT, "letter_or_digit\t%x\t%x\n", begin, unit - 1);
        begin = -1;
      }
    }
    for (int unit = 0; unit < 0x10000; unit++) {
      int value = Character.digit((char) unit, 16);
      if (value < 0) continue;
      begin = unit;
      while (unit + 1 < 0x10000
          && Character.digit((char) (unit + 1), 16) == value + unit + 1 - begin) unit++;
      System.out.printf(java.util.Locale.ROOT, "hex_digit\t%x\t%x\t%x\n", begin, unit, value);
    }
  }
}

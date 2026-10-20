// Development-only full Unicode predicate inventory from the pinned JDK.
public final class M2IdentifierProbe {
  private static boolean accepts(String kind, int cp) {
    // Lexer.nextChar consumes UTF-16 units; isRegularIdentifier uses codePoints().
    if (!kind.startsWith("regular_") && cp > 0xffff) return false;
    return kind.endsWith("start")
        ? cp == '_' || cp == '$' || Character.isUnicodeIdentifierStart(cp)
        : cp == '$' || Character.isUnicodeIdentifierPart(cp);
  }

  public static void main(String[] args) {
    for (String kind : new String[] {"start", "part", "regular_start", "regular_part"}) {
      int begin = -1;
      for (int cp = 0; cp <= 0x110000; cp++) {
        boolean present = cp < 0x110000 && accepts(kind, cp);
        if (present && begin == -1) begin = cp;
        if (!present && begin != -1) {
          System.out.printf(java.util.Locale.ROOT, "%s\t%x\t%x\n", kind, begin, cp - 1);
          begin = -1;
        }
      }
    }
  }
}

# Pkl 0.32.0 Dynamic property-name rendering

This independent Java fixture records 10 public string observations for `Map.toDynamic()` and `Dynamic.toString()`. It covers unpaired high and low surrogate property names, U+FFFD, NUL, an identifier-like name, and a map containing distinct surrogate and replacement names. The UTF-16 code unit channels preserve distinctions that the Java UTF-8 output replaces with `?`.

The authored module and ordered expressions are in `module.pkl`, `cases.json`, and `queries.tsv`. `oracle-evidence.tar.gz` contains two fresh ordered JVM sessions, every raw UTF-8 and UTF-16 channel, complete process transcripts, actual JDK/JAR identities, and copied pinned upstream sources including `VmValueRenderer` and `Lexer`. `archive-identity.json` and `manifest.json` bind the fixture. Run `python3 tools/collection_dynamic_render_fixtures.py` for offline verification.

All 10 Java calls succeed in each session. These observations establish rendered string values and raw code units; they do not establish Java wrapper identity, typed export compatibility, Rust candidate compatibility, or a full Pkl compatibility result.

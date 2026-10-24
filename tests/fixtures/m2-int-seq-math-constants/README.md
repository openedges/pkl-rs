# Pkl 0.32.0 math constant prerequisite

This is a separate Java-only supplement to the Stage 61 IntSeq string fixture. It observes all 17 external `pkl:math` constants declared in the pinned `MathNodes.java`. Each fresh JVM session makes one public string call and one public typed export call per constant, in the authored order. The typed observation records the Java class and the exact eight bytes of the exported `Long` value or IEEE-754 `Double` bits in big-endian order. The raw process protocol and both full repetitions are retained.

These observations support the IntSeq module's pinned math import. Math functions and Rust candidate compatibility are outside this supplement.

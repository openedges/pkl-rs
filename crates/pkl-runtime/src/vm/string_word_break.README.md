The adjacent `string_word_break.bin` contains the exact `WordBreakIteratorData` resource from the pinned Eclipse Temurin 21.0.12.1+1 runtime. `string_word_break.provenance.json` records the JIMAGE identity, uncompressed resource offset, size, SHA256 and table dimensions.

`string_lower.rs` reads the table for the `Locale.ROOT` final-Sigma condition used by Pkl 0.32.0. It preserves the pinned `RuleBasedBreakIterator` forward and backward state transitions, ignored character categories, UTF-16 offsets, boundary cache and `CharacterIterator.DONE` behavior. This is separate from Java Character's simple lower/title mappings and Pkl's two different whitespace classifiers.

The bytes are extracted as data, without executing Java. The shared evaluator consumes the table on native, Wasm and N-API targets; production evaluation does not invoke Java. Exact extraction does not itself establish compatibility: the String oracle and lifecycle tests must be run against the candidate implementation.

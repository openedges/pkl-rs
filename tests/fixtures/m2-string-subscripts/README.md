These fixtures retain fourteen actual Pkl 0.32.0 `evaluateExpressionString` observations from two fresh JVMs. `cases.json` preserves the source URI, expression order, and raw payload paths.

Compare successful values using `utf16BEFile`; its units are the actual Java string. `utf8File` preserves Java UTF8 serialization separately, including `?` for unpaired surrogates. Error payloads contain the complete default API diagnostic and exact source span. JVM error class names are provenance, not Rust producer requirements. These records do not establish CLI, file, or whole-language compatibility.

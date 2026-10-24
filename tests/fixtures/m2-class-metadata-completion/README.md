# Additive base Class metadata authority

This fixture contains actual public Java Pkl 0.32.0 observations for five additional `pkl:base` Class declarations: `YamlRenderer`, `PListRenderer`, `PropertiesRenderer`, `RegexMatch`, and `PcfRenderDirective`. The existing `m2-class-values` fixture and its 44 sessions remain separate and unchanged.

The main cohort has 10 sessions, each executed in two fresh JVM processes using the existing `M2ClassValuesProbe`. Its 244 public expression queries produced 204 successes and 40 ordinary Java errors; all 10 sessions have identical raw observations across the two repetitions. Java oracle errors and API exceptions were absent in this cohort. These are Java observations, not candidate compatibility passes.

The controls cover bare and direct `pkl:base` qualified Class identity, `simpleName`, `getClass`, Class type, Class versus instance identity, descriptor versus prototype properties, actual nominal renderer ancestry, ordinary lazy defaults and overrides, Unicode values, and concrete property type/constraint failures. `PcfRenderDirective` is distinct from `RenderDirective`. Its nullable fields default to null and its required value is undefined. `RegexMatch.groups` defaults to an empty List; its required value/start/end are undefined, and its ordinary `toString` method forces value. The source-pinned immediate parents are `ValueRenderer` for the three renderers and implicit `Typed` for the two other classes. Complete reflection is a separate requirement.

No external renderer method or regex constructor/matcher is invoked. Metadata and ordinary prototype coverage do not establish renderer algorithms or regex compatibility. Runtime conditions retain the existing oracle heap settings, locale, encodings, and empty evaluator environment/external properties.

`oracle-evidence.tar.gz` preserves all raw UTF-8 and Java UTF-16 channels, full ordered common protocol, actual Java exception class provenance, provider observations, exact source/query inputs, oracle/runtime/argv identities, source snapshots, compile output, process results, and source stability checks. `main/` is the final acquisition. `preserved-history-01/` retains the complete initial acquisition, whose qualified queries used a module lexical import alias unavailable in query scope. Its 20 additional ordinary errors are preserved as history, not extra independent coverage. Final queries use direct `import("pkl:base")` expressions.

The provider channels contain actual public Java module-key factory observations and fixed key properties. Common candidate query/logger/protocol comparison does not compare or discharge Java provider SPI parity. A public exported-Class identity supplement was not acquired for this additive cohort and remains required. The original fixture's export observations remain valid for their own inputs; they are not fabricated or relabeled as observations of these five classes. Whole M2, API/lifecycle, platform, and performance requirements remain open.

Verify the independently pinned authority offline:

```sh
python3 tools/class_values_fixtures.py --fixture tests/fixtures/m2-class-metadata-completion
python3 tools/class_metadata_fixtures.py --fixture tests/fixtures/m2-class-metadata-completion
```

Acquire a new Java-only run into a fresh directory:

```sh
python3 tools/class_metadata_oracle.py --inputs tests/fixtures/m2-class-metadata-completion --output target/m2/NEW-class-metadata-oracle
python3 tools/class_metadata_oracle.py --verify-report target/m2/NEW-class-metadata-oracle
```

Use the existing `class_values_compare.py --fixture` transport for candidate observations. `--verify-main` checks only the common query/logger/protocol matrix; `--verify-full` must retain the provider/export obligations as nonpasses until those APIs are genuinely mapped and compared. No candidate execution is claimed by this authority publication.

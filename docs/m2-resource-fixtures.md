# Typed Resource Java fixtures

The compact fixture contains 129 ordered primary-root API calls from two actual
Pkl 0.32.0 Java sources: cached text/Resource/Missing and retry behavior, and 24
Resource byte vectors with text/base64/class/rendering and constructor controls.
Two fresh JVMs each execute two fresh evaluators. Every selected value, complete
diagnostic, UTF-16 code-unit payload, Java UTF-8 payload and reader count agrees
across those four Java observations. The selected comparison therefore executes
516 API calls. Expected data comes exclusively from the sealed Java observer.

`expected.tsv` stores raw status bytes and both channel encodings as hexadecimal;
its final columns preserve reader counts before and after each API call. Sources
retain their original URI separately from their local fixture paths. Resource
payload files retain the original bytes, including malformed UTF-8. No URI or
language output is normalized by the fixture verifier.

Run `python3 tools/resource_oracle_fixtures.py` to verify the fixture. It pins the
independent Java report, inventory, source/case declarations and closure, checks
all four Java observations, joins expected fields to the original payload
inventory, and rejects altered sources, payloads, call order or coverage counts.

The complete 740-call Java report remains provenance. The initial native consumer
leaves 224 calls unexecuted: 20 calls through another ModuleSource in the same
evaluator and 204 URI/policy controls. Fresh SourceProbe instances do not establish
that independent-root API boundary. Standard policy callbacks, reader.close and
an explicit Java System.gc request are separate from native per-poll collection.

The initial immutable native candidate records 508 passes, four Unsupported
outcomes for default Resource.bytes and four complete diagnostic mismatches for
Uri alias provenance. Fixture integrity does not turn those results into passes.
This is selected development evidence, not complete Resource, URI, platform,
product API, language or M2 compatibility.

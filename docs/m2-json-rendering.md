# JSON rendering and render directives

The shared runtime implements `JsonRenderer.renderValue` and `renderDocument`
through the same frame-owned renderer continuation as `PcfRenderer`. Native,
Wasm, and N-API adapters do not contain independent JSON evaluators. Renderer
options, lazy members, converters, property annotation callbacks, and nested
renderer calls can suspend through the existing Host interface.

JSON strings retain UTF-16 code units, including unpaired surrogates. Escaping
operates on those units; conversion to Java-compatible UTF-8 bytes happens at
the output boundary. Object and collection iteration preserves language order.
Property annotations run before value converters, and null omission runs after
conversion. Converter paths retain the original property name when an
annotation changes its output name. JSON document output ends with a newline;
`renderValue` does not add a document newline.

`RenderDirective` supplies verbatim text to JSON and PCF rendering. Reading its
`text` uses ordinary lazy forcing and type checks; these renderers do not force
its `bytes` property. Pending directive receivers, paths, original and converted
values, and nested renderer state participate in the evaluator's GC roots.
Cancellation, closure, deadlines, and late Host completions use the shared
session lifecycle.

Supported `pkl:json.Property` and `Value` declarations are compiled from the
pinned `json.pkl` source in the ordinary module arena. The external JSON parser
remains unsupported. This checkpoint uses the default allow policy for bundled
modules. Evaluator-owned bundled module resolution, security admission, and
cache behavior remain a required follow-up. In the pinned Java implementation,
the singleton bypass for
`pkl:base`, `pkl:ref`, and `pkl:test` applies only to ModuleKey resolution;
import-edge trust and security checks still run before module-cache lookup.
Both boundaries must be preserved when adding admission.

`List.join` and `Set.join` use a shared continuation with ordered element
stringification. Their Java `Object.toString` contract differs from Pkl's
virtual typed `toString`: ordinary objects force deeply, nested strict
collections render cached values, and directly joined Bytes retain all bytes.
The continuation can suspend during object or Reference stringification.

Document conversion hints preserve original and converted values. Static
error-template hints retain priority over dynamically assigned conversion
hints, as in the Java diagnostic renderer. Shared value carriers retain a
semantic reference token separately from numeric, UTF-16, and byte payload
equality. Bytes wrappers can share backing storage without sharing identity.
Numeric local slots retain their physical primitive or Object representation,
while descriptor state belongs to the executable call target and persists
across invocations. Generic reads box primitive slots using the pinned JDK
Long cache and fresh Double references. External renderer argument slots use
the same binding path. The source-backed private base bootstrap initializes
PCF output through ordinary evaluation before a user module runs.

Generator iterations have independent physical locals and a descriptor shared
by that generator's executable node. Generated members restore their parameter
and generator bindings when forced, using the source descriptor's current kind
and preserving old physical primitive slots. These copies and suspended outer
locals are GC roots. Numeric constant properties retain their declaration's
boxed value with or without observation instrumentation. `String.dropLast`
counts Unicode code points while preserving unmatched UTF-16 units, receiver
identity for a zero count, and the canonical empty result for a proper empty
substring.

The actual four-session Java converter-history regression compares all 44 API
results, complete UTF-16 diagnostics and Java UTF-8 bytes, with collection at
every poll and zero retained slots after closure. Broader identity controls
remain separate: generator frame copies, constant collection allocation,
complete producer-specific reference behavior, and JVM-global standard-module
lifetimes are not established by that test. Cached-exception hint parity and
the full boxing-identity surface remain required work.

The immutable Stage47 C02 candidate matched both untouched original
`api/moduleOutput2.pkl` and `api/pcfRenderer9.pkl` CLI programs twice, comparing
full stdout, stderr, exit status, generated files, and the original workspace.
C01's missing-join failures are retained. Development tests also exercise
suspension, collection, and terminal cleanup. Those results do not establish
complete renderer/API compatibility, all platform execution, the full M2 gate,
or the performance targets. New Java renderer controls and final platform and
Rust checks are separate verification obligations.

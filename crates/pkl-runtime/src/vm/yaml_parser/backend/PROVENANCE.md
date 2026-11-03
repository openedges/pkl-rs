# Source provenance

This dependency-free Rust backend adapts reachable parsing paths from SnakeYAML
Engine 3.0.1 and scalar tag resolver languages from Pkl 0.32.0. Its presence does
not establish complete compatibility.

The upstream [SnakeYAML Engine 3.0.1 source artifact](https://repo.maven.apache.org/maven2/org/snakeyaml/snakeyaml-engine/3.0.1/snakeyaml-engine-3.0.1-sources.jar)
has SHA-256
`a9ff0dd1deb1781fb478bb17dd5b9758859a0419531a543608d6ca391449562c`.
The original local acquisition is identified by the repository-root path
`target/m2/stage142-yaml-backend-design-01/source-manifest.json`; this is an
evidence identity, not a path relative to this directory. Relevant source
boundaries include ScannerImpl, StreamReader, ParserImpl, Composer, MergeUtils,
Mark, exception classes, Tag, UriEncoder, CharConstants and PercentEscaper.
Comments-disabled Pkl settings make comment events and retention unreachable;
comments are still scanned.

Copyright (c) 2018, SnakeYAML. The complete Apache License, Version 2.0 text is
retained in [LICENSE-APACHE-2.0.txt](LICENSE-APACHE-2.0.txt). Original source
notices remain in the retained source artifact. The Rust adaptations use
explicit state ownership and have no Java or Truffle runtime dependency.

The Pkl resolver sources are `YamlResolver.java`, `Yaml11Resolver.java`,
`Yaml12Resolver.java` and `YamlCompatParserResolver.java` under
`external/pkl/pkl-core/src/main/java/org/pkl/core/util/yaml/snake/`, relative to
the repository root, at revision
`e51fb2f91774265666f94980ec630db794a4db2d`. Copyright © 2024–2025 Apple Inc. and
the Pkl project authors, Apache-2.0. ParserNodes numeric and object constructors
are outside the pure graph backend.

The implementation also accounts for the pinned Pkl fat-JAR relocation in
`external/pkl/build-logic/src/main/kotlin/pklFatJar.gradle.kts` and actual direct
composer observations. Shaded ordinary exception identities and two reader
failure captures establish the selected JDK range diagnostic and
bounds-before-EOF ordering.

Source-derived unit controls remain implementation regression checks. Selected
compiled graph and native observations are recorded in [STATUS.md](STATUS.md);
the final Stage144 checkpoint report is pending publication. Complete
compatibility requires further pinned Java raw graph/error and full Pkl output
comparisons. Original draft documents and acquisition failures remain unchanged.

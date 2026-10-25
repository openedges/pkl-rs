# Pinned Character name data

The adjacent checked UTF-16 backend tables are deterministically decoded from
`java.base/java/lang/uniName.dat` in the pinned Temurin JDK 21.0.12.1+1 runtime
image. The runtime resource and the distribution's JMOD payload were compared
byte for byte. Payload SHA-256:
`2e18d5cebacf69138806cf5a91e69b0d28b006077b5601761449d83e3ff1ff83`.

The 34,887 complete names are indexed without sampled-name exceptions. The
normalization table is derived from the pinned Character simple mappings and
CharacterData00/Latin1 full uppercase expansions. ASCII names use Java trim and
ROOT uppercase semantics. Assigned code points without explicit names use the
canonical block-name plus uppercase hexadecimal fallback from Character.java.

Preparation source, runtime-image offsets, all source hashes, exhaustive decoded
table round-trip and resource provenance are retained in the Stage76 syntax
draft's `prepare_names.py` and `name-provider-report.json`. This preparation does
not establish a compiled Rust compatibility result. Applicable JDK license and
Unicode notices are preserved in `jdk-character-names-legal/`.

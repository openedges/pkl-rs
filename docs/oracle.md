# Pinned Pkl 0.32.0 Oracles (JVM, GraalVM CE, and Official Native Image)

Verification uses three explicitly pinned, checksummed Apple Pkl **0.32.0**
reference profiles locked in [oracle.lock.json](../oracle.lock.json). All three
are strictly for verification and baseline measurement — never as a production
fallback in Rust evaluation paths. A successful bootstrap or repeated oracle
result never by itself establishes Rust compatibility or performance parity.

1. **`reference` (default JVM oracle)**: Official Maven Central
   `org.pkl-lang:pkl-tools:0.32.0` shaded JAR running on exact pinned **Eclipse
   Adoptium Temurin OpenJDK `21.0.13.1+2-LTS`** (`java.version=21.0.13.1`).
   Authoritative byte oracle for M0–M6 differential compatibility, M7 fuzzing,
   and all 19 embedded API + JS/Wasm performance workloads.
2. **`native-image` (primary CLI/server/`pkldoc` performance oracle, pinned 2026-11-23)**:
   Official GitHub Release `0.32.0` prebuilt `native-image` CLI executables
   (`pkl-macos-aarch64` [`99,267,376 B`, SHA-256 `fb891856...a848c13928`] and
   `pkldoc-macos-aarch64` [`99,266,664 B`, SHA-256 `f363e095...46923f`] for
   `Darwin-arm64`). Primary `<= 1.00x` execution time and `os.wait4` peak RSS
   strict gate for all 15 CLI, server, and `pkldoc` workloads (`cold-cli` and
   `steady-server`).
3. **`performance` (archived M0 GraalVM CE optimizing JVM profile)**: Official
   **GraalVM Community Edition `25.0.1`** archive (`Darwin-arm64`), used for
   the archived M0 optimizing-JIT baseline in `reports/m0/benchmarks.json`.

[oracle.lock.json](../oracle.lock.json) pins download URLs, byte sizes,
SHA-256 digests, embedded release version (`0.32.0`) and source commit
(`e51fb2f`, matching [references.lock](../references.lock)), source metadata
hashes, JDK identity, heap settings, and execution policy. No reference
checkout under `external/` is modified.

## Bootstrap and verification

Requirements are Python 3.9+ (Python 3.10+ for M2/M7 scripts using
`zip(..., strict=True)`), Git, and Eclipse Temurin OpenJDK `21.0.13.1+2-LTS`
(provisioned separately from
[Adoptium's official releases](https://adoptium.net/temurin/releases/)). Select
the pinned JDK with `PKL_ORACLE_JAVA` when it is not `java` on `PATH`. For the
`native-image` profile, `fetch --profile native-image` installs into
`.cache/oracle/native-image-0.32.0/`, or pre-provisioned executables may be
pointed to via `PKL_ORACLE_NATIVE_PKL` and `PKL_ORACLE_NATIVE_PKLDOC` (still
strictly checked for exact byte size, SHA-256, executable bit, and `--version`):

```sh
export PKL_ORACLE_JAVA=/absolute/path/to/pinned-jdk/bin/java

# 1) Default JVM reference oracle (Temurin 21 + pkl-tools-0.32.0.jar)
python3 tools/oracle.py fetch
python3 tools/oracle.py verify
python3 tools/oracle.py smoke

# 2) Official Apple Pkl 0.32.0 native-image CLI oracle (pkl + pkldoc)
python3 tools/oracle.py fetch --profile native-image
python3 tools/oracle.py verify --profile native-image

# 3) Optimizing GraalVM CE 25.0.1 profile (macOS arm64 M0 reference)
python3 tools/oracle.py fetch --profile performance
python3 tools/oracle.py verify --profile performance

# Tool unit & tamper-detection tests
PYTHONPATH=tools python3 -m unittest tools/tests/test_oracle.py -v
```

`fetch` downloads the fixed HTTPS URL to a temporary file, verifies SHA-256 and
embedded metadata, and atomically installs `.cache/oracle/pkl-tools-0.32.0.jar`.
It never replaces an existing corrupt artifact. Remove a corrupt cache file
explicitly before fetching again. `verify` and `smoke` never fetch artifacts;
a missing or corrupted artifact, wrong JDK vendor/build, dirty reference
checkout, mismatched source revision/gitlink, or changed source metadata fails.
The SHA-256 is a reviewed project pin calculated from the official publication,
not an independent signature verification. The artifact is not rebuilt locally.

The full `fetch` command is also the clean-cache bootstrap check. A JDK with the
same major version but another patch, vendor, or runtime build is rejected. The
exact executable, runtime `lib/modules`, and JDK `release` hashes are recorded in
provenance for the current OS/architecture; this does not assert that JDK binaries
on different platforms have identical bytes. Changing JDK policy requires an
explicit lock update and new evidence.

The default `reference` profile uses externally provisioned Temurin 21 for the
byte oracle. The `performance` profile downloads the separately pinned official
GraalVM Community 25.0.1 archive into `.cache/oracle` and extracts it there
(as of 2026-11-20, `graalvm/graalvm-ce-builds` has not yet published a JDK
`25.0.5.1` Community Edition build addressing `CVE-2026-114938`–`114942`; the
`performance` profile is macOS arm64-only and not downloaded by default CI, and
will be updated when a patched CE 25 release is published).
It verifies the archive SHA-256 and installed Java launcher, runtime modules,
release metadata, JVM library, Java library, and optimizing compiler library
against locked hashes. Currently only macOS arm64 has a pinned performance
distribution; other platforms fail explicitly until their distribution and
runtime hashes are reviewed and added. System JDK installations are untouched.

## Execution contract and evidence

Each run records `target/m0/oracle/provenance.json`. It includes the lock hash,
artifact URL/hash, source metadata, absolute Java and JAR paths, Java identity and
binary hashes, OS/architecture, complete JVM command, allowlisted environment,
and raw Java/Pkl version output encoded as hexadecimal. No inherited environment
or secrets are logged.
The performance profile writes `performance-provenance.json`, including the
official JDK archive URL/hash and an actual Java source-launch probe of
`Truffle.getRuntime().getName()`. It requires the exact optimizing runtime name
`GraalVM CE`, with no stderr. A runtime probe establishes compiler availability;
actual warmup and compiled workload evidence belong to the benchmark runner.

Execution uses UTC, English/US Java locale, UTF-8, a noninteractive terminal,
fixed terminal width, a 64 MiB initial heap and 512 MiB maximum heap, and isolated
home/temp directories under `.cache/oracle`. The subprocess environment is an
allowlist; inherited `JAVA_TOOL_OPTIONS`, `JDK_JAVA_OPTIONS`, `_JAVA_OPTIONS`,
`CLASSPATH`, credentials, proxy settings, and user settings are not passed through.
Callers must supply an explicit working directory and record their inputs,
resources, CLI arguments, and allowed network fixtures. This bootstrap does not
sandbox arbitrary upstream modules or block all network access.

`smoke` evaluates a successful module and a module with a missing member twice.
It checks the known success bytes and exit status, checks that the error exits
with code 1 on stderr, then compares each replay's complete raw stdout, stderr,
and exit code. The error result is captured from Java, not synthesized or used
as evidence of Rust compatibility. Files are:

- `work/success.pkl` and `work/error.pkl`: exact smoke inputs.
- `success.stdout.bin`, `success.stderr.bin`, `error.stdout.bin`,
  `error.stderr.bin`: untouched bytes from the first execution.
- `smoke.json`: input/output hashes, commands, working paths, exit statuses,
  replay checks, and oracle provenance.

With `--profile performance`, smoke evidence is placed in the `performance/`
subdirectory so it cannot overwrite reference-profile captures.

Source locations contain the actual working path. Move or mount the entire test
workspace consistently for differential comparisons; never rewrite diagnostics
to hide path differences. Version text also contains the actual OS/JDK identity.

Python callers can use the importable interface:

```python
metadata = ensure_oracle(repository_root, fetch=False)
result = subprocess.run(
    [*metadata["command"], "eval", "--no-project", "--color=never", "case.pkl"],
    cwd=case_directory,
    env=metadata["environment"],
    capture_output=True,
)
```

The returned metadata is JSON-compatible. `command` ends with
`org.pkl.cli.Main` and is ready for CLI arguments; `java` and `artifact` are
absolute paths for JVM API probes. Keep subprocess output as bytes.
Use `ensure_oracle(repository_root, fetch=False, profile="performance")` for
measurements against the optimizing GraalVM runtime, and
`ensure_native_image_oracle(repository_root, fetch=False)` to retrieve verified
paths and hashes for the pinned Official Apple Pkl `0.32.0` `native-image`
`pkl` and `pkldoc` CLI executables (`exact_native_image_lock_match: true`).

## Upstream build, Truffle, and licensing

The pinned `pkl-tools/pkl-tools.gradle.kts` defines the shaded publication.
Upstream source builds require JDK 25 or newer and target JVM bytecode 17 by
default; this is distinct from this project's selected runtime JDK 21.
`gradle/libs.versions.toml` pins Graal/Truffle dependencies to `25.0.1`.
The default reference runtime is Temurin HotSpot using the artifact's bundled Truffle
runtime, with no external Graal compiler, no additional polyglot engine flags,
and no native-image executable. Upstream suppresses its interpreter-only warning
in `VmUtils`. This interpreter-based reference must not be presented as
measurement against an optimizing GraalVM. Performance measurements use the
separate GraalVM profile, which permits native access and legacy Unsafe memory
access explicitly with `--enable-native-access=ALL-UNNAMED` and
`--sun-misc-unsafe-memory-access=allow`; these match upstream dependency needs
on JDK 25 and avoid warnings changing captured stderr. Neither flag disables
Truffle optimization. Both profiles use the locked 64 MiB initial/512 MiB maximum
heap; this is a fixed experiment condition, not the upstream CLI default or a
claim about total resident memory. Performance reports must identify the
configuration and distinguish cold and warmed measurements. Native-image
measurements remain a separate comparison.

The official artifact is Apache-2.0 with third-party components. Consult the
pinned source [LICENSE.txt](../external/pkl/LICENSE.txt),
[NOTICE.txt](../external/pkl/NOTICE.txt), and
[THIRD-PARTY-NOTICES.txt](../external/pkl/THIRD-PARTY-NOTICES.txt), whose hashes are
in the lock. Preserve these notices when redistributing. JAR license entries can
belong to shaded dependencies and do not replace the complete upstream notices.
Temurin is GPL-2.0 with the Classpath exception; its installed `legal/` directory
contains the runtime's applicable notices. The downloaded GraalVM Community
distribution and its components also carry notices in their extracted `legal/`
directory, including GPL-2.0 with the Classpath exception.

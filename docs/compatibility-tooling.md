# Compatibility evidence tooling

`tools/compat.py` records results from the pinned Java Pkl 0.32.0 oracle and
compares a candidate runner against those results. It compares complete stdout,
stderr, exit codes, and generated file paths/bytes without trimming, text decoding,
reserialization, or content normalization. Recording Java outputs, or comparing
Java with itself, verifies the harness and does not establish Rust compatibility.

## Commands

Bootstrap the reference oracle as described in [oracle.md](oracle.md), then use
an empty evidence directory and a dedicated generated workspace:

```sh
python3 tools/compat.py record \
  --manifest compatibility/smoke.json \
  --workspace target/m0/smoke-workspace \
  --output target/m0/smoke-baseline
python3 tools/compat.py check \
  --manifest compatibility/smoke.json \
  --output target/m0/smoke-baseline
python3 tools/compat.py record \
  --manifest compatibility/corpus.json \
  --workspace target/m0/full-workspace \
  --output target/m0/full-corpus --jobs 4 --timeout 30
python3 -m unittest discover -s tools/tests -p 'test_compat.py'
```

`record`, `compare`, and `check` return nonzero for incomplete or failing coverage.
A corpus containing blocked requirements remains incomplete even when every
ready case executes successfully. Do not remove blocked cases to obtain a green
full-corpus report. A smaller manifest is a separately named, explicitly limited
scope such as `compatibility/smoke.json`.

A candidate runner is a JSON object with a nonempty string array:

```json
{"command": ["/absolute/path/to/candidate-pkl"]}
```

The harness appends each manifest case's CLI arguments. A runner can instead be
a process adapter implementing that CLI boundary. Candidate comparison requires
the same absolute workspace path as Java so diagnostic source locations remain
comparable:

```sh
python3 tools/compat.py compare \
  --manifest compatibility/smoke.json \
  --expected target/m0/smoke-baseline \
  --runner target/m0/candidate-runner.json \
  --workspace target/m0/smoke-workspace \
  --output target/m0/smoke-candidate
python3 tools/compat.py check \
  --manifest compatibility/smoke.json \
  --output target/m0/smoke-candidate
```

Using the pinned Java command as the candidate is a harness replay test. Label
that result Java-versus-Java; it is not evidence for a Rust evaluator.

## Safety and input provenance

Managed workspaces and evidence directories must be below this repository's
`target/`. Workspace/evidence overlap, protected manifest/runner/baseline overlap,
and symlink components in managed paths are rejected before mutations. A
workspace must be empty or contain this tool's exact ownership marker for the
same repository and path. An unmarked nonempty directory is never deleted. Choose
a new path for workspaces made by older tooling. An owned workspace is recreated
from scratch, so stale fixtures, generated files, and caches do not survive.
Reference submodules are copied from tracked inputs and never used as writable
execution directories.

Before recording or comparing, the harness checks source repository revisions,
tracked path inventories, resource inventory hashes, every declared input and
resource checksum, symlink target text, and resource kinds/sizes. It repeats
checks against staged bytes. Project smoke fixtures carry explicit source hashes.
A full staged snapshot, including empty directories and symlink identities, is
stored as `inputs.json`; comparisons require it to match the Java snapshot.

A Java baseline must have complete recorded coverage, intact raw capture files,
and the same oracle lock, source, JAR, JDK identity/binary hashes, and JVM command
as the current verified oracle. `check` rereads all accepted case captures and
validates their bytes, generated file sets/hashes, report metadata, and input
snapshot. Missing or corrupted evidence cannot pass by retaining a successful
status in `report.json`.

## Execution isolation and output scope

Each case receives a freshly created home, temp, and default package cache under
its deterministic workspace state path. Java `user.home` and `java.io.tmpdir`
properties are replaced accordingly. Case environment overrides cannot inject
Java options/classpaths or override those isolation directories. The oracle's
allowlisted environment remains the starting point; inherited credentials and
Java option variables are not forwarded.

Known read-only `eval` commands execute concurrently up to `--jobs`. Their entire
batch is invalidated if a post-execution snapshot detects source/workspace
mutation. Other commands, including Pkl tests and package creation, execute
serially. Their secondary workspace changes are captured and the original staged
inputs restored before the next case. A failed restoration prevents later
writable cases from running with altered inputs. The harness does not add
optional JUnit reports or otherwise introduce generated products absent from the
requested CLI command.

Each capture stores raw stdout/stderr plus two file scopes:

- `generated/<path>` contains files under the case's `{output_dir}`.
- `workspace/<path>` contains added or modified secondary files outside private
  home/temp/cache and explicit generated directories.

`workspace_changes` records complete before/after path inventories and content
hashes, including deletions, directory changes, and symlink changes. Raw secondary
file bytes are retained; source locations and names are not rewritten. Output
symlinks under `{output_dir}` are rejected explicitly rather than followed.

Subprocesses run in a new POSIX session. On timeout, the complete process group is
killed, partial stdout/stderr and generated files are retained, and the case has
status `timeout`. Successful process completion also cleans up surviving group
members. Windows process-tree cleanup is not implemented; that runner reports an
environment error. This is process isolation for the supported test contracts,
not an operating-system sandbox for hostile executables. Writes outside the
managed workspace and case home/temp paths are not comprehensively observable.
External services and fetched package payloads still require the manifest's
separate pinned dependency/network contract; a successful process is not proof
that an unpinned live response became reproducible.

## Reports and status interpretation

Each case directory contains `stdout.bin`, `stderr.bin`, `result.json`, and its
captured file tree. The final `report.json` includes the manifest/input hashes,
oracle provenance, candidate command/executable and file hashes, actual per-case
commands/working paths, deterministic environment controls, coverage counts, and
case results. `progress.jsonl` is an incremental diagnostic stream; only the final
report and strict `check` establish execution accounting.

| Status | Meaning |
| --- | --- |
| `recorded` | Java ran with an allowed exit code and raw evidence was saved. |
| `passed` | The candidate matched all required captured bytes and metadata. |
| `failed` | Candidate exit expectations or a complete observable comparison differed. |
| `unexpected_oracle_exit` | Java exited outside the manifest's allowed outcomes. |
| `blocked` | The inventory declares an execution requirement unavailable. |
| `timeout` | The invocation exceeded its deadline; partial raw evidence was saved. |
| `environment_error` | A runner, input, resource, expected result, output contract, or isolation check failed. |

Only `recorded` is accepted for a recording report and only `passed` for a
candidate report. Empty manifests, duplicate or missing results, blocked cases,
errors, skips, corrupted captures, and unavailable runners cannot count as passes.
Expected nonzero Pkl errors remain valid cases when their complete outputs and
exit codes satisfy the declared contract. Historical reports without the current
input snapshot and capture metadata must be regenerated.

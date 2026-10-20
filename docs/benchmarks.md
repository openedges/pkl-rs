# Initial Java Benchmark Contract

[benchmarks/workloads.json](../benchmarks/workloads.json) defines the initial Java
workloads, sampling policy, and remaining required workload groups. This is an M0
baseline acquisition tool. There is no Rust evaluator measurement, time/RSS ratio,
performance target verdict, or full workload completion claim.

```sh
python3 tools/oracle.py fetch --profile performance
python3 tools/benchmark.py --check-contract
python3 -m unittest discover -s tools/tests -p test_benchmark.py -v
python3 tools/benchmark.py
python3 tools/benchmark.py --check-report target/m0/benchmarks
```

The default output is `target/m0/benchmarks`; use `--output` for another fresh directory.
Existing evidence is never overwritten. The measurement host must have the pinned
optimizing JDK distribution in [oracle.lock.json](../oracle.lock.json), currently
GraalVM Community 25.0.1 for macOS arm64. The script obtains the verified `performance`
oracle profile and records the exact artifact, JDK archive/runtime hashes, JVM options,
compiler executable hash, harness hashes, benchmark input bytes/hashes, OS, CPU, RAM,
load averages, and Python identity. Compilation precedes measurements and is recorded
separately. Inputs or harness changes during a run reject the final report.

JVM settings retain the oracle's `-Xms64m -Xmx512m`, encoding, locale, timezone, user,
temporary directory, environment allowlist, and native-access settings. No inherited
Java option environment variables are accepted. Run without concurrent tests, builds,
Pantry processes, or other heavy applications. Record the host's power/thermal state
and load context when publishing results; the tool records start/end load but cannot
eliminate system contention or certify thermal stability.

## Workloads and Boundaries

Six deterministic fixtures exercise scalar evaluation, stdlib collection map/fold,
recursive Fibonacci computation, approximately 219 KB of Unicode output, deliberate
failure diagnostics, and a small diamond import graph. Each module defines `run(seed)`
and sets `output.text = run(0)` through an output amendment. The API invokes `run(0)`
through `Evaluator.evaluateExpressionString`; it does not repeatedly read a memoized
`output.text` property. Each invocation parses the expression and executes the function,
including export/string conversion and UTF-8 byte conversion. Cached module definitions,
imports, and function call targets remain available in persistent mode. This is an API
boundary measurement, not a pure VM instruction benchmark.

| Mode or phase | Timing and memory boundary |
| --- | --- |
| `cold-cli` | Five fresh Java processes per workload, from process creation through output capture and process exit. Includes startup, parsing, evaluation, rendering/output, and shutdown. The OS filesystem/page cache is uncontrolled and usually warm after preflight; this does not measure cold disk caches. |
| API initialization | Construct the first evaluator in a fresh JVM; in fresh-evaluator mode close this setup evaluator immediately. JVM launch/class loading before this call remains in the separately recorded whole-process duration. |
| First API request | First invocation including module loading. In fresh-evaluator mode it also builds/closes that operation's evaluator. |
| `persistent` | Repeated `evaluateExpressionString(module, "run(0)")` with the same evaluator; computation reruns while module/function definitions are reused. |
| `fresh-evaluator` | Build evaluator, load/evaluate the module and imports, invoke the expression, export bytes, and close the evaluator on every operation. The JVM and global runtime remain warm; evaluator/module state is fresh. |
| API process peak RSS | One peak per independent JVM fork, covering initialization, first request, correctness validation, warmup, measurement, and close. It is not isolated steady-state heap usage or an individual request's RSS. |

CLI uses `--no-cache --no-project --settings pkl:settings --color=never`; API evaluators
disable the package cache and use explicit empty properties/environment maps. Fixtures
do not configure per-request timeouts; the outer process timeout bounds the run. A rejected
initial attempt configured a 30-second API timeout and exhausted the 512 MiB heap while
queuing millions of canceled timeout tasks in the scalar loop. The raw failed attempt
is retained separately; it is not an eligible performance result. Upstream
[`EvaluatorImpl.java:374`](https://github.com/apple/pkl/blob/e51fb2f91774265666f94980ec630db794a4db2d/pkl-core/src/main/java/org/pkl/core/EvaluatorImpl.java#L374)
schedules a task per evaluation, while
[`TimeoutTask.cancel():542`](https://github.com/apple/pkl/blob/e51fb2f91774265666f94980ec630db794a4db2d/pkl-core/src/main/java/org/pkl/core/EvaluatorImpl.java#L542)
marks the task canceled and leaves it scheduled until its delay expires. This changes
the CPU baseline boundary; the Java reference implementation remains unmodified. Timeout and
cancellation behavior remains in the required lifecycle/memory workload inventory.
Fixtures
require local filesystem imports and stdlib only. No package downloads or network I/O
occur in the measured workloads. The diamond graph is deliberately small; large graphs
remain a pending requirement.

Peak RSS comes from `os.wait4` for the exact child PID. macOS reports bytes; Linux reports
KiB, converted to bytes. The script never uses cumulative `RUSAGE_CHILDREN`, JVM heap,
or allocation counters as process RSS. Missing/unsupported RSS is an error. Timed-out
process groups are killed and cannot become eligible samples. Windows measurement is
currently unimplemented.

## Correctness and Sampling

Before timing, Java CLI stdout, stderr, and exit code are captured and replayed exactly.
The API records its raw exported bytes or full `PklException` message, then verifies an
independent replay. Successful CLI/API bytes must also match. Failure fixtures require
CLI exit 1 with diagnostic bytes and an API `PklException`; they are not treated as
unexpected successful evaluation. CLI and API error channels differ by their public
boundary, so each uses its own exact baseline. Expected bytes come from the pinned Java
oracle and are retained without trimming or reserialization.

Every timed API operation checks the complete returned bytes and success/error outcome
against preflight, outside the operation's timer. Every cold sample checks stdout,
stderr, and exit status before inclusion. Incorrect output, missing evidence, missing
RSS, timeouts, unknown modes, insufficient warmup, false convergence, missing samples,
or missing workload results reject validation. Correctness-check allocations remain
inside the whole-process RSS boundary.

Each API mode runs in two independent JVM forks per workload. Each fork warms up for
at least ten seconds. Windows accumulate at least 250 ms of measured operation time;
each window records its operation count, elapsed time, and mean nanoseconds per operation.
Warmup stops after the minimum duration when the most recent eight windows satisfy:

```text
(maximum window mean - minimum window mean) / median window mean <= 0.20
```

Warmup lasts at most 45 seconds before recording `inconclusive`. It still records the
measurement windows so unstable behavior remains inspectable, but cannot be described
as converged steady-state performance. Wall-clock warmup duration includes validation
and instrumentation; each raw window independently records the accumulated measured
operation time. Nine measurement windows follow warmup in each fork. Initialization,
first request, warmup windows, and measurement windows remain separate in the raw JSONL.

Reports expose each fork's empirical distribution, median, p90/p95, standard deviation,
range, and the distribution of fork medians and fork peaks. These small initial samples
do not establish statistical significance or an upper performance bound. No aggregate
average can hide a required workload's regression. Future Rust comparisons require
identical correct outputs, compatible process/host boundaries, independent repeated runs,
and explicit uncertainty assessment for each workload.

The optimizing runtime probe verifies availability only. A separate instrumented
recursive-compute fork enables `polyglot.engine.TraceCompilation` and preserves its raw
streams. The report records completed `fib` compilation trace lines if observed. Trace
instrumentation is excluded from timing samples. That trace establishes evidence for
the instrumented workload only; it does not prove every measured call target compiled
or that all workloads reached peak performance. Missing trace evidence remains explicit.

## Evidence and Unimplemented Scope

`report.json` includes the full initial-workload results and pending requirement groups.
Per-process `result.json`, raw stdout/stderr, raw expected output, warmup windows,
measurement windows, compilation output, compiler/runtime provenance, and trace streams
remain alongside input/helper source snapshots and compiled helper hashes. `partial.json` retains completed workloads if a later operation
fails. `--check-report` validates raw hashes, sample counts, required initial workload
coverage, RSS presence, operation arithmetic, and reported convergence against the raw
evidence. Process capture paths must identify distinct samples/forks, recorded commands
must match their workload/mode, first-request bytes and independent preflight replays
must remain present, and summaries are recomputed. Trace claims are checked against
the actual trace files. It does not turn an inconclusive or pending requirement into a pass.

The required inventory still includes long-running memory recovery, cold package/HTTP
I/O, larger imports, every renderer and generated file, REPL and server sessions,
Wasm/browser hosting, N-API conversion/lifecycle, JVM adapters, developer tools, official
native-image reference runs, and all Rust candidates. These remain `unimplemented` in
the workload contract. See [TESTING.md](../TESTING.md) for the complete release and
performance criteria.

The initial macOS arm64 acquisition is summarized in
[reports/m0/benchmarks.json](../reports/m0/benchmarks.json). All six workloads produced
30 validated cold CLI samples and 24 independent API forks, with convergence in every
fork. The separate recursive trace observed `FrameWithoutBoxing` escape-related Tier 1
compilation bailouts for the fixture's `fib` and `run` functions; only the stdlib
`toString` target completed compilation. Recursive guest compilation remains unobserved.
These measurements characterize the pinned runtime's observed behavior and have limited
applicability to fully optimized recursive execution. Stable timing windows and the
availability of Graal do not establish peak optimized workload performance. The raw
trace, raw successful acquisition, and rejected timeout attempt have separate verified
archives linked from the summary.

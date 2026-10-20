# Java Host lifecycle evidence for M1

The pinned Java 0.32.0 evaluator was exercised with an actual custom
`ResourceReader`, controlled callback gates, and concurrent close. This grounds
lifecycle decisions in the Java implementation. It is Java-only observation
evidence; Rust `HostSession` tokens, cooperative suspension, and error enums are
separate architectural contracts. These runs establish neither byte equivalence
nor public JVM/API compatibility for Rust.

```sh
python3 tools/m1_host.py --output target/m1/host-java-new-run
python3 tools/m1_host.py --check-report target/m1/host-java-new-run
python3 -m unittest discover -s tools/tests -p test_m1_host.py -v
python3 tools/evidence.py verify reports/m1/host-java-evidence.tar.gz
```

Each new run retains the Java helper/harness source snapshots, exact oracle/JDK
provenance, commands, module input bytes, stdout/stderr, complete exception message
bytes, stack traces, and an integrity inventory. The helper records its own small
event protocol; the complete original exception messages are saved separately.
Reports check that each required operation and real callback occurred. They do
not normalize errors or compare a Rust implementation with Java. Exception stack
traces and executor messages can contain process-specific details.

## Observed results

| Experiment | Java 0.32.0 observation | Implication for the Rust prototype |
| --- | --- | --- |
| Repeated evaluation | Same-module evaluation and a second module reading the same resource invoke the reader once; a fresh evaluator invokes it again | Scope evaluated-module/resource state to an evaluator; keep compiled-code cache lifetime separate |
| Delayed read | Evaluation remains pending until the callback gate opens; the returned string is reused on a repeat call | Preserve evaluation state while adapters wait for Host data |
| Same-thread reentry | The resource callback successfully evaluates a different module on the same evaluator; inner and outer results are both `41` | Rust's rejection of an active-session start is a known prototype limitation, not Java behavior parity |
| Timeout during read | Timeout interrupts the blocked callback and reports exactly `Evaluation timed out after 2 second(s).` | Timeout permanently ends the evaluator, but matching the public failure type/message remains future work |
| Close during read | Close interrupts the callback, evaluation reports an I/O `PklException`, and close completes without opening the manual gate | A platform adapter must cancel its outstanding I/O; merely discarding its completion is a weaker contract |
| After ordinary close | Evaluation throws `org.graalvm.polyglot.PolyglotException` with `Context execution was cancelled.` | Record actual oracle behavior rather than assuming the documented exception class |
| After timeout | Evaluation throws `java.util.concurrent.RejectedExecutionException` because its timeout scheduler is shut down | The current Rust `Closed` error differs; exact Java adapter compatibility remains unresolved |
| Repeated close | The second close succeeds | Make closure idempotent |

The timeout post-close exception messages contain executor/task identity hashes.
Their complete bytes are retained unchanged. These Java-only experiments check
the observed exception class and nonempty message, not byte equality of separate
runs. They cannot support a claim that a stable Rust error string matches Java.
The timeout message itself is compared as complete bytes.

Both timeout and close interrupted `CountDownLatch.await()` in the reader. The
helper converts that interruption to an `IOException`, so the active close case
reports the corresponding Pkl I/O diagnostic. This is evidence for this exact
callback implementation, not a universal guarantee that arbitrary user I/O
responds to cancellation. Ten-second watchdogs guard the test harness; exceeding
them is an acquisition failure rather than a passing cancellation test. The
2-second timeout, 2.25-second observation window, and 100-millisecond close window
are experiment conditions, not performance or scheduling guarantees.

## Upstream source contract

`Evaluator.close()` documents that close waits for active cancellation, prevents
further use, and leaves previously exported values valid. Its Javadoc names
`IllegalStateException` for evaluation after closure. The actual pinned artifact
produced the exception classes above. The report deliberately retains that
difference; it does not rewrite observable behavior to match the Javadoc.

`EvaluatorImpl.TimeoutTask.run()` calls `close()`, and `doEvaluate()` explicitly
states that a timeout means the evaluator is closed. `close()` closes the
polyglot context and shuts down the timeout executor. `doEvaluate()` schedules
its timeout before entering the context, which explains the observed rejected
schedule after timeout. This explanation is an inference supported by those
source paths and the captured exception stack.

The selected public evaluator architecture must permit same-thread callback
reentry. It needs a stack of evaluation contexts, each with its own continuation,
locals, return state, and Host root set, sharing the evaluator's module/resource
caches and terminal lifetime. The present `HostSession` supports one active
continuation and returns `Busy` for a second start. That is an explicit prototype
limitation. Implementing the context stack and its cancellation/rooting rules is
M2/M4 work; the successful Java reentry probe prevents treating that restriction
as a compatible public API contract.

`ResourceReader.read(URI)` is synchronous. Java's public SPI has no Rust-style
poll/resume token protocol. The Rust design's stale-completion rejection,
monotonic host clock, and explicit instruction budgets are shared-engine adapter
choices requiring their own target tests. Same-thread reentry, cancellation of
in-flight platform I/O, exported-value validity after close, arbitrary callback
exceptions, and precise JVM adapter diagnostics remain compatibility work.

Source references:

- `external/pkl/pkl-core/src/main/java/org/pkl/core/Evaluator.java`
- `external/pkl/pkl-core/src/main/java/org/pkl/core/EvaluatorImpl.java`
- `external/pkl/pkl-core/src/main/java/org/pkl/core/EvaluatorBuilder.java`
- `external/pkl/pkl-core/src/main/java/org/pkl/core/resource/ResourceReader.java`

The first ad-hoc trial used an unsupported synthetic module URI and failed before
reader entry. Its raw failures remain archived; that trial has incomplete source
snapshot provenance and is not evidence for lifecycle success. The first full
runner invocation exposed incorrect validation assumptions about timeout wording
and the post-timeout exception class. That failed report and every raw stream
remain alongside the corrected, successful five-observation run. Neither failed
attempt is counted as a successful observation suite.

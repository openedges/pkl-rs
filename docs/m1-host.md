# M1 Host contract

`pkl-runtime::host::HostSession` owns one VM, its suspended frames, lazy state,
and heap. `poll` returns a completed value/error, an outstanding resource read,
or a cooperative instruction-budget yield. The adapter fulfills a read with
`resume`; it never restarts evaluation to deliver the resource. The core has no
filesystem, network, timer, Node, browser, or Java dependency.

Every read token contains session, context, and request identities. Repeated
polling preserves the token. Duplicate, stale, noncanonical, cross-session, and
out-of-order ancestor completions are rejected. An active evaluation rejects an
ordinary second `start`; a callback uses `reenter` with its current read token.

Reentry saves the parent's frames, operands, inputs, and pending read in the same
VM. Contexts share the heap and successful member cache. A nested `Ready` returns
the child result and restores the waiting parent; its original token can then
resume. A child error also restores the parent without poisoning its lazy state.
Invalid nested starts leave the parent intact. Collection traces all contexts;
callers must supply externally retained result handles to `collect`. The bounded
prototype permits at most 64 contexts and explicitly rejects deeper reentry.
This experiment does not implement general module/resource caches or final APIs.

Public cancellation, timeout, and explicit close permanently close the session,
release its VM roots and heap storage, and reject later evaluations/completions.
The first terminal reason remains observable through `poll`; repeated close is
idempotent. All nested contexts share that terminal lifetime. Before releasing
them, the session queues each outstanding read token once, ancestors first.
The platform adapter must consume `drain_cancelled_reads` and abort the actual
operations; rejecting late replies alone does not cancel I/O. The lower-level
`Vm::cancel` is a reusable VM primitive and does not
define the public evaluator's lifecycle.

The adapter supplies monotonic milliseconds to start, poll, and resume. A
backward clock and overflowing deadline are rejected. The deadline wins when
`now >= deadline`, including while waiting for a resource response. Any active
ancestor or child deadline expires the entire evaluator. Completing a nested
context removes only its own deadline. There is no timer thread inside the core: adapters must
schedule polling, including while a Host promise is pending. Cancellation and
timeouts are cooperative at poll/resume boundaries; one expensive instruction
can delay observation. A zero instruction budget is rejected.

These are architecture contracts. Java's ResourceReader SPI is synchronous;
Rust's poll/resume protocol is not the Java server protocol or a public API
compatibility claim. See [the pinned Java lifecycle experiment](m1-host-java.md)
for observations, including reentry and interruption behavior. Full 0.32 error
rendering, resource caching, module loading, asynchronous capability discovery,
and server message codecs belong to later milestones.

Native Host regressions cover suspended stack preservation, exactly-once
delivery, request identities, nested shared caches and roots, failed nested
starts, terminal state ordering, clock/budget handling, and heap reclamation.
The [platform experiment](m1-platforms.md) drives real Node/browser callbacks
and timers through the same core, including abortable fake reads with observed
promise settlement and resource release. The Java/Rust differential runner is
`python3 tools/m1_lifecycle.py --output target/m1/lifecycle-new`; its deterministic
observation protocol and separately preserved diagnostics have distinct scopes.

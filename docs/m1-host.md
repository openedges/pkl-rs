# M1 Host contract

`pkl-runtime::host::HostSession` owns one VM, its suspended frames, lazy state,
and heap. `poll` returns a completed value/error, an outstanding resource read,
or a cooperative instruction-budget yield. The adapter fulfills a read with
`resume`; it never restarts evaluation to deliver the resource. The core has no
filesystem, network, timer, Node, browser, or Java dependency.

Every read token contains a session identity and a monotonically increasing
request identity. Repeated polling preserves the token. Duplicate, stale,
noncanonical, and cross-session completions are rejected. An active evaluation
rejects a second `start`. This single-continuation restriction is a known
prototype difference: Java permits evaluation reentry from a reader callback.
The product evaluator must use a stack of evaluation contexts sharing its
module/resource caches and terminal lifetime, with separate continuations and
root sets. A Host failure finishes the current evaluation, while
a later evaluation may start on the same session.

Public cancellation, timeout, and explicit close permanently close the session,
release its VM roots and heap storage, and reject later evaluations/completions.
The first terminal reason remains observable through `poll`; repeated close is
idempotent. The lower-level `Vm::cancel` is a reusable VM primitive and does not
define the public evaluator's lifecycle.

The adapter supplies monotonic milliseconds to start, poll, and resume. A
backward clock and overflowing deadline are rejected. The deadline wins when
`now >= deadline`, including while waiting for a resource response. Completion
clears the deadline. There is no timer thread inside the core: adapters must
schedule polling, including while a Host promise is pending. Cancellation and
timeouts are cooperative at poll/resume boundaries; one expensive instruction
can delay observation. A zero instruction budget is rejected.

These are architecture contracts. Java's ResourceReader SPI is synchronous;
Rust's poll/resume protocol is not the Java server protocol or a public API
compatibility claim. See [the pinned Java lifecycle experiment](m1-host-java.md)
for observations, including reentry and interruption behavior. Full 0.32 error
rendering, resource caching, module loading, asynchronous capability discovery,
and server message codecs belong to later milestones.

Seven native Host regressions cover suspended stack preservation, exactly-once
delivery, request identities, terminal state ordering, clock/budget handling,
heap reclamation while suspended, and the actual frontend-to-VM adapter path.
The [platform experiment](m1-platforms.md) also exercises real asynchronous
Node/browser callbacks and timers through the same core.

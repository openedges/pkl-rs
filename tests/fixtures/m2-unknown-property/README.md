# Unknown property parent controls

These fixtures were captured with pinned Java Pkl 0.32.0 by `M2PowerAssertionOracle` using `output.text` and fixed `file:///stage134-unknown-property/<case>.pkl` URIs. Each case was evaluated with power assertions enabled and disabled.

The direct `unknown` property produces `Dynamic`. An alias to `unknown` preserves the ordinary “Cannot tell which parent to amend” error. The Rust harness compares the complete status and UTF-8/UTF-16 output and error fields. Java exception class fields are retained as capture evidence but are not compared by the scalar Rust API.

Authority: `target/m2/stage134-unknown-property-oracle-01/report.json`; raw readback: `target/m2/stage134-unknown-property-readback-01/report.json`. The fixture namespace contains 45 captured files, including inputs and the roster. These four observations are bounded regression evidence, not complete language or API compatibility.

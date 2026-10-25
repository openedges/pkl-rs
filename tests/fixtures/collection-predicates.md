# Collection predicate fixtures

The Java 0.32.0 authority for collection emptiness and `every` lives in
`m2-collection-emptiness-oracle/` and `m2-collection-every-oracle/`. Each sealed
root contains the authored module and cases, both Java repetitions, raw output
channels, source provenance, and its archive identity. The `every` archive also
contains the ordered logger events and their UTF-16 code units.

The Rust regression inputs remain in `m2-collection-emptiness/` and
`m2-collection-every/`. These roots contain the exact expected files consumed
by the tested Rust suites. They are kept separate because the sealed authority
roots require an exact file set. `tools/runtime_fixture_links.py` verifies the
runtime files against the Java archives, including logger fields and ordering.

`m2-string-base64/` is both the sealed Java root and the runtime input root.
Its Rust test stores 89 expectations inline; the same verifier binds each
authored expression, outcome kind, and UTF-16 expected value to the archive.

Run `python3 -B tools/runtime_fixture_links.py` in a fresh checkout to verify
these links without any `target/` history. This proves fixture provenance; it
does not claim candidate compatibility or completion of M2.

"""Additive Java Any.ifNonNull return, default, and escaped-closure observations."""
import argparse
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from tools.any_if_non_null_oracle import ROOT, acquire

FIXTURE = ROOT / "tests/fixtures/m2-any-if-non-null-supplement"
MODULE = '''// Additive Java controls; canonical 86-query authority remains frozen.
marker = "stage58-supplement"
function badReturn(x): Int = "bad"
'''
CASES = [
    ("return-value-error", "1.ifNonNull((x) -> badReturn(x))", "return-check"),
    ("return-null-skip", "null.ifNonNull((x) -> badReturn(x))", "return-check"),
    ("return-default-null-skip", 'Null(new Dynamic { marker = "default" }).ifNonNull((x) -> badReturn(x))', "return-check"),
    ("return-retry", "1.ifNonNull((x) -> x + 1)", "return-check"),
    ("default-lambda-marker", '((Null(() -> new Dynamic { marker = "deferred" }).ifNonNull((x) -> x)) { extra = 8 }).apply().marker', "default-function"),
    ("default-lambda-extra", '((Null(() -> new Dynamic { marker = "deferred" }).ifNonNull((x) -> x)) { extra = 8 }).apply().extra', "default-function"),
    ("dynamic-local-implicit", '(new Dynamic { local function ifNonNull(f) = "local"; result = ifNonNull((x) -> "base") }).result', "dispatch"),
    ("dynamic-property-implicit", '(new Dynamic { ifNonNull = "property"; result = ifNonNull((x) -> "base") }).result', "dispatch"),
    ("escaped-closure", '(new Dynamic { value = "receiver" }).ifNonNull((x) -> (suffix) -> x.value + suffix).apply("!")', "closure"),
    ("null-escaped-closure", 'null.ifNonNull((x) -> (suffix) -> "unreachable")', "closure"),
    ("constraint-int-implicit", "10 as Int(ifNonNull((x) -> x) > 0)", "constraint"),
    ("constraint-null-implicit", 'null as Null(ifNonNull((x) -> throw("never")) == null)', "constraint"),
    ("constraint-default-implicit", '(Null(new Dynamic { x = 1 }) as Null(ifNonNull((x) -> throw("never")) == null)) == null', "constraint"),
]


def rows():
    return [{"id": id, "expression": expression, "group": group} for id, expression, group in CASES]


def prepare():
    FIXTURE.mkdir(parents=True, exist_ok=True)
    manifest = {"schema": 1, "baseline": "Pkl 0.32.0", "module_uri": "file:///stage58/any-if-non-null-supplement.pkl",
                "boundary": "string", "repetitions": 2, "rows": rows()}
    (FIXTURE / "module.pkl").write_text(MODULE)
    (FIXTURE / "cases.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (FIXTURE / "queries.tsv").write_text("".join(r["id"] + "\t" + r["expression"] + "\n" for r in rows()))
    return len(CASES)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    choice = parser.add_mutually_exclusive_group(required=True)
    choice.add_argument("--prepare", action="store_true")
    choice.add_argument("--output", type=Path)
    args = parser.parse_args()
    if args.prepare:
        print(prepare())
    else:
        audit = acquire(args.output, fixture=FIXTURE, authored_rows=rows(), authored_module=MODULE,
                        run_source=ROOT / "tools/any_if_non_null_supplement_oracle.py")
        print(json.dumps(audit["statuses"], sort_keys=True))

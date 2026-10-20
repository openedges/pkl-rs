#!/usr/bin/env python3
"""Small seeded M2 property and malformed-grammar corpus; not full fuzz coverage."""
from __future__ import annotations

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SEED = 0x4D32504B4C303332


def generated_cases():
    state = SEED
    cases = []

    def next_int(bound):
        nonlocal state
        state ^= (state << 13) & ((1 << 64) - 1)
        state ^= state >> 7
        state ^= (state << 17) & ((1 << 64) - 1)
        state &= (1 << 64) - 1
        return state % bound

    def add(identifier, source, feature, kind, relation=None, failure=False):
        row = dict(id=identifier, input=f'tests/fixtures/m2/{identifier}.pkl', expression='result',
                   requirements=['semantic:' + feature, 'semantic:generated-verification'],
                   expected_outcome='failure' if failure else 'success',
                   generated=dict(seed=f'{SEED:016x}', algorithm='xorshift64-13-7-17', kind=kind))
        if relation:
            row['metamorphic_relation'] = relation
        cases.append((row, source))

    for index in range(8):
        a, b, c = [next_int(1001) - 500 for _ in range(3)]
        family = ('alpha-rename', 'recursive-sum', 'associativity', 'unused-branch')[index % 4]
        feature = {'alpha-rename': 'name-resolution', 'recursive-sum': 'functions',
                   'associativity': 'operators', 'unused-branch': 'control-flow'}[family]
        ids = [f'generated-{family}-{index}-{side}' for side in ('left', 'right')]
        if family == 'alpha-rename':
            sources = [f'result = let (x = {a}) let (y = {b}) x * 3 + y\n',
                       f'result = let (first = {a}) let (second = {b}) first * 3 + second\n']
            domain = 'Capture-free alpha renaming of bounded Int let bindings'
        elif family == 'recursive-sum':
            n = next_int(17)
            sources = [f'function sum(n) = if (n == 0) 0 else n + sum(n - 1)\nresult = sum({n})\n',
                       f'result = {n} * ({n} + 1) ~/ 2\n']
            domain = 'Sum recursion and closed form, Int n in [0,16]'
        elif family == 'associativity':
            sources = [f'result = ({a} + {b}) + {c}\n', f'result = {a} + ({b} + {c})\n']
            domain = 'Int addition associativity with each operand in [-500,500], no overflow'
        else:
            sources = [f'result = if (true) {a} else missing\n', f'result = {a}\n']
            domain = 'Unselected conditional branch must remain unforced'
        relation = dict(kind='equal-stdout', case_ids=ids, domain=domain)
        for identifier, source in zip(ids, sources, strict=True):
            add(identifier, source, feature, 'metamorphic', relation)

    # Seeded mutation of grammar delimiters/required operands. Each generated
    # module is independently executed against Java; parser acceptance or any
    # diagnostic difference is retained as an oracle error or mismatch.
    for index in range(16):
        number = next_int(1000)
        bad = (')', ']', '}', ',')[next_int(4)]
        templates = [f'result = ({number} + {bad})\n',
                     f'result = if (true) {bad} else {number}\n',
                     f'result = let (x = {number}) {bad}\n',
                     f'function f(x) = {number}\nresult = f({bad})\n']
        source = templates[next_int(len(templates))]
        if next_int(2):
            source = 'prefix = "😀한"\n' + source
        add(f'fuzz-parser-{index:02}', source, 'diagnostics', 'malformed-grammar-mutation', failure=True)

    # Keep these draws after the historical generator so its fixture bytes remain
    # unchanged. The domains state why each expression pair is equivalent.
    value, offset, bound = [next_int(101) - 50 for _ in range(3)]
    relations = [
        ('pipeline-apply', 'functions',
         [f'offset = {offset}\nf = (x) -> x + offset\ng = (x) -> x * 2\nresult = {value} |> f |> g\n',
          f'offset = {offset}\nf = (x) -> x + offset\ng = (x) -> x * 2\nresult = g.apply(f.apply({value}))\n'],
         'One-argument captured Int functions; left-associated pipeline equals nested apply, no overflow'),
        ('constraint-is', 'constraints',
         [f'result = {value} is Int(this > {bound})\n',
          f'result = ({value} is Int) && {value} > {bound}\n'],
         'Bounded Int value and bound; successful base type leaves the same Boolean predicate'),
        ('constraint-nullable', 'constraints',
         [f'result = (null is Int(this > {bound})?) && ({bound + 1} is Int(this > {bound})?)\n',
          f'result = true && {bound + 1} > {bound}\n'],
         'Nullable constrained Int accepts null and a bounded Int strictly above its bound; both checks must succeed'),
        ('constraint-union', 'unions',
         [f'result = {value} is (Int(this > {value + 1}) | Int)\n',
          f'result = {value} is Int\n'],
         'Bounded Int fails the first constraint and succeeds in the unconstrained Int union branch'),
        ('constraint-base', 'constraints',
         ['result = "wrong base" is Int(this ~/ 0 > 0)\n', 'result = false\n'],
         'A String fails the Int base check before the fatal division predicate can execute'),
    ]
    for family, feature, sources, domain in relations:
        ids = [f'generated-{family}-{side}' for side in ('left', 'right')]
        relation = dict(kind='equal-stdout', case_ids=ids, domain=domain)
        for identifier, source in zip(ids, sources, strict=True):
            add(identifier, source, feature, 'metamorphic', relation)
    return cases


def check_generated(selection, root=ROOT):
    expected = generated_cases()
    if [row for row in selection if 'generated' in row] != [row for row, _ in expected]:
        raise ValueError('seeded M2 case selection stale; run tools/m2_generate.py --write')
    for row, source in expected:
        if (root / row['input']).read_bytes() != source.encode():
            raise ValueError('seeded M2 input differs: ' + row['id'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    path = ROOT / 'tests/fixtures/m2/cases.json'
    selection = json.loads(path.read_bytes())
    if args.write:
        selection['cases'] = [row for row in selection['cases'] if 'generated' not in row]
        for row, source in generated_cases():
            selection['cases'].append(row)
            (ROOT / row['input']).write_bytes(source.encode())
        path.write_text(json.dumps(selection, ensure_ascii=False, indent=2) + '\n')
    check_generated(selection['cases'])
    print(f'{len(generated_cases())} deterministic generated cases verified')


if __name__ == '__main__':
    main()

#!/usr/bin/env python3
"""Conservative M2 requirements and case accounting, never a completion claim."""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import sys

import m2_generate
import m2_module_eval

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = Path('compatibility/m2-language.json')
SELECTION = Path('tests/fixtures/m2/cases.json')
MAPPINGS = Path('compatibility/m2-mappings.json')
MODULE_ADAPTERS = Path('compatibility/m2-module-eval.json')
INPUTS = ('references.lock', 'compatibility/inventory.json', 'compatibility/corpus.json',
          'compatibility/rev1-inventory.json', 'compatibility/regressions.json', str(SELECTION),
          str(MAPPINGS), str(MODULE_ADAPTERS))
FEATURES = {
    'lexing': 'All tokens, whitespace, nested comments, quoted and blank identifiers, Unicode and source positions',
    'numeric-literals': 'Integer bases, separators, signed boundaries, floating syntax and numerical values',
    'scalar-literals': 'Boolean and null literals and defaults',
    'strings': 'Escapes, custom delimiters, interpolation, multiline indentation and 0.32 continuation',
    'operators': 'All unary and binary operators, precedence, associativity and mixed operand semantics',
    'numeric-errors': 'Integer overflow, division, remainder, nonfinite floating values and first error selection',
    'control-flow': 'Conditional expressions, short circuiting, coalescing, null propagation and non-null assertion',
    'name-resolution': '0.32 lexical/receiver/owner resolution, this, outer, super, module and forward references',
    'lazy-evaluation': 'Member forcing, successful memoization, failed retries, cycles and error order',
    'objects': 'Object properties, entries, elements, member predicates, duplicate names and inferred new',
    'amendment': 'Object, function and null amendments, late binding and prototype chains',
    'classes': 'Class declarations, constructors, typed objects, class inheritance and runtime class identity',
    'modifiers': 'Local, hidden, fixed, const, abstract, open, external; hidden override and modifier errors',
    'functions': 'Method declarations/calls, parameter and return checks, recursion and argument errors',
    'closures': 'Function literals, lexical captures, owner/receiver lifetime and returned closures',
    'generators': 'When/for generators, nesting, scopes, spreads, nullable spreads and member predicates',
    'collections': 'List/Set/Map and lazy Listing/Mapping construction, defaults and element constraints',
    'types': 'Primitive/class/module/function/string literal/unknown/nothing types and eager/lazy checks',
    'type-aliases': 'Type aliases, recursive aliases, type defaults and nominal identities',
    'constraints': 'Constraint evaluation, this binding, composite constraints and first failing errors',
    'generics': 'Generic classes/type aliases, parameter variance, bounds and substitution',
    'unions': 'Union types, default branch, nullable types and branch selection diagnostics',
    'type-tests-casts': 'Type tests, casts, narrowing and failures',
    'reference': '0.32 Reference<Domain, Type>, domain/referent constraints and runtime identity',
    'annotations': 'Annotation declarations, type checks and metadata',
    'reflection-metadata': 'Shared class/module/member/type metadata needed by reflection',
    'modules': 'Module declarations, extends/amends/import language semantics and module types; Host I/O remains M4',
    'diagnostics': 'Whole parser/resolver/runtime diagnostics, first errors, source underlines and logical call frames',
    'tracing': 'Trace expression forcing, value identity, source frames, logger delivery, UTF-16 messages and callback lifecycle',
    'shared-engine': 'Shared semantics, values and diagnostics across native/Wasm/N-API; M6 full bindings remain open',
    'generated-verification': 'Seeded differential generation, semantic properties, metamorphic tests and fuzz minimization',
}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def encoded(value) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + '\n').encode()


def load(path: Path, root: Path = ROOT):
    return json.loads((root / path).read_bytes())


def apply_mappings(requirements: list[dict], cases: list[dict], mappings: dict,
                   baseline: dict, root: Path = ROOT):
    """Apply reviewed, additive links without replacing original case contracts.

    A mapping documents a human assessment of scope. File/symbol validation only
    checks provenance; it cannot prove that the implementation covers that scope.
    Execution and the unchanged full gate remain separate requirements.
    """
    if (not isinstance(mappings, dict)
            or set(mappings) != {'schema_version', 'baseline', 'requirements'}
            or type(mappings['schema_version']) is not int or mappings['schema_version'] != 1
            or mappings['baseline'] != {'version': baseline['version'], 'revision': baseline['revision']}
            or not isinstance(mappings['requirements'], list)):
        raise ValueError('invalid reviewed mapping schema or baseline')
    by_requirement = {row['id']: row for row in requirements}
    by_case = {row['id']: row for row in cases}
    seen = set()
    for mapping in mappings['requirements']:
        if not isinstance(mapping, dict) or set(mapping) != {
                'requirement', 'implementation', 'tests', 'scope', 'remaining'}:
            raise ValueError('invalid reviewed requirement mapping')
        identifier = mapping['requirement']
        if not isinstance(identifier, str) or identifier not in by_requirement or identifier in seen:
            raise ValueError('unknown or duplicate reviewed requirement')
        seen.add(identifier)
        implementation, tests = mapping['implementation'], mapping['tests']
        if (not isinstance(implementation, dict) or set(implementation) != {'state', 'links'}
                or implementation['state'] not in ('partial', 'implemented')
                or not isinstance(tests, dict) or set(tests) != {'state', 'case_ids'}
                or tests['state'] not in ('partial', 'mapped')
                or not isinstance(mapping['scope'], str) or not mapping['scope'].strip()):
            raise ValueError('reviewed mapping needs explicit scope and states: ' + identifier)
        remaining = mapping['remaining']
        if (not isinstance(remaining, list)
                or any(not isinstance(item, str) or not item.strip() for item in remaining)):
            raise ValueError('invalid remaining mapping scope: ' + identifier)
        complete = implementation['state'] == 'implemented' and tests['state'] == 'mapped'
        if complete == bool(remaining):
            raise ValueError('reviewed states contradict remaining scope: ' + identifier)
        raw_links = implementation['links']
        if not isinstance(raw_links, list) or not raw_links:
            raise ValueError('reviewed implementation has no source links: ' + identifier)
        links, seen_links = [], set()
        for link in raw_links:
            if not isinstance(link, dict) or set(link) != {'path', 'symbol'}:
                raise ValueError('invalid implementation source link: ' + identifier)
            relative, symbol = link['path'], link['symbol']
            if (not isinstance(relative, str) or not isinstance(symbol, str)
                    or not re.fullmatch(r'[A-Za-z_][A-Za-z_0-9]*', symbol)):
                raise ValueError('invalid implementation source identity: ' + identifier)
            path = Path(relative)
            if (path.is_absolute() or '..' in path.parts or path.as_posix() != relative
                    or path.parts[:1] != ('crates',) or path.suffix != '.rs'
                    or not (root / path).resolve().is_relative_to(root.resolve())
                    or (relative, symbol) in seen_links):
                raise ValueError('invalid or duplicate implementation source path: ' + identifier)
            seen_links.add((relative, symbol))
            source = (root / path).read_bytes()
            if not re.search(r'\b' + re.escape(symbol) + r'\b', source.decode('utf-8')):
                raise ValueError('implementation source symbol missing: ' + identifier)
            links.append(dict(path=relative, symbol=symbol, sha256=digest(source)))
        ids = tests['case_ids']
        if (not isinstance(ids, list) or not ids or any(not isinstance(item, str) for item in ids)
                or len(ids) != len(set(ids)) or not set(ids) <= by_case.keys()):
            raise ValueError('missing, duplicate, or unknown reviewed test case: ' + identifier)
        requirement = by_requirement[identifier]
        existing = requirement['tests']['case_ids']
        # Preserve selected supporting cases even when the review focuses on
        # another contract. Nothing may disappear from the completion gate.
        linked_ids = list(dict.fromkeys([*existing, *ids]))
        for case_id in linked_ids:
            case = by_case[case_id]
            if identifier not in case['requirements']:
                case['requirements'].append(identifier)
        requirement['implementation'] = dict(state=implementation['state'], links=links)
        requirement['tests'] = dict(state=tests['state'], case_ids=linked_ids)
        requirement['reviewed_mapping'] = dict(source=str(MAPPINGS), scope=mapping['scope'],
                                               remaining=remaining)
        requirement['reason'] = 'Reviewed source/test links; completion still requires the full recorded scope and executions.'


def build_manifest(root: Path = ROOT) -> dict:
    inventory = load(Path('compatibility/inventory.json'), root)
    corpus = load(Path('compatibility/corpus.json'), root)
    rev1 = load(Path('compatibility/rev1-inventory.json'), root)
    selected = load(SELECTION, root)['cases']
    module_cases = m2_module_eval.registry_cases(root, corpus, load(MODULE_ADAPTERS, root))
    m2_generate.check_generated(selected, root)
    requirements = []
    for name, description in FEATURES.items():
        ids = [row['id'] for row in selected if 'semantic:' + name in row['requirements']]
        requirements.append(dict(id='semantic:' + name, kind='semantic-family', description=description,
                                 required=True, implementation=dict(state='partial' if ids else 'unimplemented', links=[]),
                                 tests=dict(state='partial' if ids else 'unmapped', case_ids=ids),
                                 reason='A selected probe does not establish implementation or complete edge-case coverage.'))
    for item in inventory['items']:
        if item['category'] not in ('language_token', 'language_parser') and not (
                item['category'] == 'specification' and '/language-reference/' in item['source']['path']):
            continue
        requirements.append(dict(id=item['id'], kind=item['category'], description=item['symbol'],
                                 source=item['source'], required=True,
                                 implementation=dict(state='unimplemented', links=[]),
                                 tests=dict(state='unmapped', case_ids=[]),
                                 reason='Source-derived declaration/heading retained conservatively; semantic mapping requires review.'))
    cases = []
    for row in selected:
        source = root / row['input']
        cases.append(dict(**row, required=True, adapter='language-api-string',
                          input_sha256=digest(source.read_bytes()), execution=dict(state='ready', reason=None)))
    for row in corpus['cases']:
        if row['suite'] != 'language-snippet':
            continue
        executable = row['id'] in module_cases
        cases.append(dict(id=row['id'], required=True, adapter=m2_module_eval.ADAPTER if executable else None, input='external/pkl/' + row['source']['path'],
                          input_sha256=row['source']['sha256'], requirements=[],
                          execution=dict(state='ready' if executable else 'unimplemented', reason=None if executable else row['execution']['reason'] or
                                         'Full upstream snippet/output contract adapter and Rust renderer are unimplemented.'),
                          upstream_case=row))
    for row in rev1['files']:
        if row['kind'] != 'regression-candidate':
            continue
        cases.append(dict(id='rev1-candidate:' + row['path'], required=True, adapter=None, requirements=[],
                          source=dict(repository=rev1['repository'], revision=rev1['revision'], **row),
                          execution=dict(state='unmapped', reason='Historical candidate awaits semantic selection/import and fresh Java execution; historical passes are not evidence.')))
    for row in load(Path('compatibility/regressions.json'), root)['cases']:
        cases.append(dict(id='m2:' + row['id'], required=True, adapter=None, input=row['source']['path'],
                          input_sha256=row['source']['sha256'], requirements=[],
                          execution=dict(state='unimplemented', reason='Selected original regression retained; M2 full module/CLI adapter is unimplemented.'),
                          regression_case=row))
    apply_mappings(requirements, cases, load(MAPPINGS, root), inventory['baseline'], root)
    result = dict(schema_version=1, baseline=inventory['baseline'],
                  scope='Complete M2 language requirement accounting with a conservative superset of all pinned language snippets; initial executable probes are a separate selection.',
                  module_comparison_boundary='Original module eval argv/cwd/env/resources in one physical staged workspace; complete stdout/stderr/exit/files and unexpected workspace writes. Parser success is unsupported ModuleOutput evaluation, never eval success. No golden replay or diagnostic normalization.',
                  comparison_boundary='evaluateExpressionString(ModuleSource.path(same absolute URI), expression) UTF-8 stdout; whole PklException.getMessage() UTF-8 stderr and exit 1; no trimming. Power assertions default to disabled; an explicit per-case power_assertions boolean enables the same option in both probes. Stringification alone is not typed value/API compatibility.',
                  sources={path: digest((root / path).read_bytes()) for path in INPUTS},
                  requirements=requirements, cases=cases,
                  completion='Every required semantic/source requirement needs implementation/test mappings and complete passing execution. Pending/unmapped/unimplemented/mismatch remains a full-gate failure.')
    validate_manifest(result, root)
    result['counts'] = dict(requirements=len(requirements), requirements_by_kind=dict(Counter(x['kind'] for x in requirements)),
                            cases=len(cases), cases_by_execution=dict(Counter(x['execution']['state'] for x in cases)))
    return result


def validate_manifest(manifest: dict, root: Path = ROOT):
    for collection in ('requirements', 'cases'):
        ids = [row['id'] for row in manifest[collection]]
        if not ids or len(ids) != len(set(ids)):
            raise ValueError('empty or duplicate ' + collection)
    corpus = load(Path('compatibility/corpus.json'), root)
    registered = m2_module_eval.registry_cases(root, corpus, load(MODULE_ADAPTERS, root))
    originals = {row['id']: row for row in corpus['cases']}
    requirement_ids = {row['id'] for row in manifest['requirements']}
    case_ids = {row['id'] for row in manifest['cases']}
    if not set(registered) <= case_ids:
        raise ValueError('registered original module-eval coverage missing')
    for row in manifest['requirements']:
        if not set(row['tests']['case_ids']) <= case_ids:
            raise ValueError('requirement maps missing case: ' + row['id'])
    for row in manifest['cases']:
        if row.get('adapter') == m2_module_eval.ADAPTER:
            if (row['id'] not in registered or row.get('upstream_case') != originals[row['id']]
                    or row['execution'] != dict(state='ready', reason=None)
                    or row['input_sha256'] != registered[row['id']]['input_sha256']):
                raise ValueError('module-eval original execution contract differs')
        if row['id'] in registered and row.get('adapter') != m2_module_eval.ADAPTER:
            raise ValueError('registered original module-eval case is missing its adapter')
        if not set(row['requirements']) <= requirement_ids:
            raise ValueError('case maps unknown requirement: ' + row['id'])
        if row.get('adapter') == 'language-api-string':
            path = Path(row['input'])
            if path.is_absolute() or '..' in path.parts or not row['requirements']:
                raise ValueError('invalid executable input or missing requirement')
            if digest((root / path).read_bytes()) != row['input_sha256']:
                raise ValueError('input bytes changed: ' + row['id'])
            if row['expected_outcome'] not in ('success', 'failure'):
                raise ValueError('expected Java outcome is unspecified')
            if not isinstance(row.get('power_assertions', False), bool):
                raise ValueError('power_assertions must be a boolean')


def full_gate_failures(manifest: dict, results: list[dict]) -> list[str]:
    failures = []
    by_id = {row['id']: row for row in results}
    if len(by_id) != len(results):
        failures.append('duplicate execution results')
    for row in manifest['requirements']:
        if not row['required']:
            continue
        if row['implementation']['state'] != 'implemented' or not row['implementation']['links']:
            failures.append('implementation incomplete: ' + row['id'])
        if row['tests']['state'] != 'mapped' or not row['tests']['case_ids']:
            failures.append('test mapping incomplete: ' + row['id'])
        elif any(by_id.get(i, {}).get('status') != 'pass' for i in row['tests']['case_ids']):
            failures.append('requirement execution incomplete: ' + row['id'])
    for row in manifest['cases']:
        if row['required'] and (not row['requirements'] or row['execution']['state'] != 'ready'
                                or by_id.get(row['id'], {}).get('status') != 'pass'):
            failures.append('required case incomplete: ' + row['id'])
    return failures


def current_manifest() -> dict:
    manifest = load(MANIFEST)
    if manifest != build_manifest():
        raise ValueError('M2 manifest stale; regenerate with tools/m2_manifest.py')
    return manifest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true', help='inventory freshness only, not compatibility')
    parser.add_argument('--full-gate', action='store_true', help='strict completion with no execution evidence')
    args = parser.parse_args()
    manifest = build_manifest()
    if args.check or args.full_gate:
        current_manifest()
    else:
        (ROOT / MANIFEST).write_bytes(encoded(manifest))
    print(json.dumps(manifest['counts'], sort_keys=True))
    if args.full_gate:
        failures = full_gate_failures(manifest, [])
        print(f'Full M2 gate: {len(failures)} incomplete requirements/cases', file=sys.stderr)
        return bool(failures)
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError) as error:
        print(f'M2 manifest error: {error}', file=sys.stderr)
        sys.exit(1)

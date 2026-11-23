#!/usr/bin/env python3
"""Build and execute shared M1 probes in native Rust, Node, and Chromium.

Strict comparison exits 1 for any oracle mismatch. --verify-contract checks the
experiment and its exact mismatch inventory; neither proves full Pkl API coverage.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys

from m1_semantics import decode_capture, generate_cases

ROOT = Path(__file__).resolve().parents[1]
# Stage76–78 strict raw result (target/m2/stage76-78-m1-validation-01/strict/
# experiment/report.json, SHA256 777e1f700559f400b76adf5764f080daf85944a0ffdfdd95779abefb8177e384)
# matches Java for all 6,078 rows, including the two previously known Regex cases.
KNOWN_GAPS = set()
REQUIRED_SCENARIOS = {
    'shared-parser-resolver-vm', 'utf8-utf16-string-boundary',
    'lossless-i64-bigint-range', 'owned-byte-array-boundary',
    'same-evaluator-nested-read-resume', 'cross-session-completion-rejected',
    'async-resource-bytes', 'host-error-bytes',
    'live-host-cancel-abort-release-settle', 'live-host-close-abort-release-settle',
    'live-host-timeout-abort-release-settle',
    'nested-terminal-aborts-all-live-host-operations',
    'cooperative-loop-cancel', 'invalid-clock-budget-boundary',
    'm2-recursive-methods-and-closures', 'm2-lazy-control-and-interpolation',
    'm2-shared-language-diagnostics', 'm2-class-method-and-type-alias',
    'm2-pipeline-and-constraint-values',
    'm2-constraint-diagnostics-and-power-assertions',
    'm2-ordered-collection-members-and-amendments',
    'm2-collection-generators-and-spread',
    'm2-collection-diagnostics-and-power-assertions',
    'm2-lazy-generic-collection-values',
    'm2-strict-list-nested-wrappers',
    'm2-generic-collection-diagnostics',
    'm2-member-predicate-values',
    'm2-member-predicate-owner-cache',
    'm2-member-predicate-diagnostics',
    'm2-structural-equality',
    'm2-structural-keys',
    'm2-structural-equality-diagnostics',
    'm2-typed-default-equality-and-keys',
    'm2-comparison-cache-and-class-order',
    'm2-deep-force-before-key-hash',
    'm2-local-cache-comparison',
    'm2-hashed-key-buckets',
    'm2-cache-fifth-publication',
    'm2-let-diagnostic-scopes',
    'm2-reference-paths-and-rendering',
    'm2-reference-equality-and-type-checks',
    'm2-reference-stdlib-diagnostics',
    'm2-fixed-inheritance-and-virtual-receivers',
    'm2-fixed-reference-declaration-metadata',
    'm2-fixed-property-diagnostics',
    'm2-class-publication-sequences',
    'm2-type-alias-values',
    'm2-function-type-metadata',
    'm2-generic-alias-diagnostics',
    'm2-type-resolution-error-order',
    'm2-type-parameter-shadowing',
    'm2-unfinished-alias-arguments',
    'm2-recursive-constraint-sequences',
    'm2-erased-class-argument-sequences',
    'm2-shared-constraint-slots-sequences',
    'm2-constraint-call-boundaries-sequences',
    'm2-local-resolution-inline-sequences',
    'm2-local-resolution-call-targets-sequences',
    'm2-local-resolution-recursive-sequences',
    'm2-local-resolution-owners-sequences',
    'm2-local-resolution-wrappers-sequences',
    'm2-generic-recursive-sequences',
    'm2-generic-type-variables-sequences',
    'm2-generic-let-sharing-sequences',
    'm2-generic-use-sites-sequences',
    'm2-generic-wrappers-sequences',
    'm2-generic-defaults-sequences',
    'm2-generic-reference-sequences',
    'm2-generic-metadata-deep-sequences',
    'm2-generic-metadata-default-sequences',
    'm2-generic-metadata-consumer-sequences',
    'm2-nullable-default-listing-sequences',
    'm2-nullable-default-mapping-sequences',
    'm2-nullable-default-nested-sequences',
    'm2-generic-constructors-sequences',
    'm2-generic-constructors-closure-sequences',
    'm2-strict-map-sequences',
    'm2-strict-map-host-sequences',
    'm2-strict-set-sequences',
    'm2-strict-set-host-sequences',
    'm2-collection-type-sequences',
    'm2-collection-type-host-sequences',
    'm2-frontend-depth-sequences',
    'm2-frontend-depth-host-sequences',
    'm2-string-frontend-sequences',
    'm2-lexical-diagnostic-recovery',
    'm2-unicode-diagnostic-recovery',
    'm2-lossless-string-representation',
    'm2-lossless-diagnostic-frame-identity',
    'm2-numeric-value-frame-spans',
    'm2-frontend-annotation-owners',
    'm2-frontend-const-owner-and-parent-order',
}


def check_scenarios(execution):
    for target in ['nodeChecks', 'browserChecks']:
        names = execution.get(target, [])
        if len(names) != len(REQUIRED_SCENARIOS) or set(names) != REQUIRED_SCENARIOS:
            raise ValueError(f'{target}: missing, duplicate, or unexpected platform scenarios')
    if execution.get('browserErrors') != []:
        raise ValueError('browser error inventory missing or nonempty')
    pin = json.loads((ROOT/'tools/platform/package.json').read_text())['devDependencies']['playwright']
    if any(execution.get(name) != pin for name in ['playwright', 'playwrightCore', 'playwrightPin']):
        raise ValueError('platform execution did not use the pinned Playwright installation')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def cargo_products():
    # Cargo commands below run with cwd=ROOT; relative target directories must
    # be interpreted against that same directory, not the caller's cwd.
    target = ROOT / os.environ.get('CARGO_TARGET_DIR', 'target')
    libraries = {'Darwin': 'libpkl_napi.dylib', 'Linux': 'libpkl_napi.so', 'Windows': 'pkl_napi.dll'}
    return {
        'wasm': target/'wasm32-unknown-unknown/release/pkl_wasm.wasm',
        'napi': target/'release'/libraries[platform.system()],
        'native': target/'debug/examples'/('semantics_probe.exe' if os.name == 'nt' else 'semantics_probe'),
    }


def compare(cases, captures):
    if not cases or len({case['id'] for case in cases}) != len(cases):
        raise ValueError('empty or duplicate case inventory')
    if set(captures) != {'java', 'rust', 'node', 'browser'}:
        raise ValueError('all four execution targets are required')
    decoded = {name: decode_capture(data, len(cases)) for name, data in captures.items()}
    rows = []
    for index, case in enumerate(cases):
        values = {name: data[index] for name, data in decoded.items()}
        matches = {name: value == values['java'] for name, value in values.items() if name != 'java'}
        rows.append({'id': case['id'], 'oracle_matches': matches,
                     'shared_engine_match': values['rust'] == values['node'] == values['browser'],
                     'sha256': {name: hashlib.sha256(value).hexdigest() for name, value in values.items()},
                     **({'mismatched_bytes_hex': {name: value.hex() for name, value in values.items()}} if not all(matches.values()) else {})})
    return rows


def contract_matches(rows):
    if not rows or not all(row['shared_engine_match'] for row in rows):
        return False
    return all({row['id'] for row in rows if not row['oracle_matches'][target]} == KNOWN_GAPS
               for target in ['rust', 'node', 'browser'])


def source_paths(root=ROOT):
    paths = {root / name for name in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
             'tools/m1_platforms.py', 'tools/m1_semantics.py', 'tools/java/M1SemanticsProbe.java',
             'tests/fixtures/m1/semantics/cases.json', 'crates/pkl-runtime/Cargo.toml',
             'crates/pkl-runtime/examples/semantics_probe.rs',
             'crates/pkl-runtime/src/stdlib/modules.txt']}
    suffixes = {'.rs', '.pkl', '.toml', '.mjs', '.json', '.bin', '.tsv'}
    excluded = {'node_modules', 'target', '.cache'}
    for folder in ['crates/pkl-runtime/src', 'crates/pkl-wasm', 'crates/pkl-napi', 'tools/platform']:
        for directory, directories, names in os.walk(root / folder, topdown=True):
            # Exclude dependency/build caches before descending into them.
            directories[:] = sorted(name for name in directories if name not in excluded)
            for name in sorted(names):
                path = Path(directory) / name
                if path.suffix in suffixes and path.is_file():
                    paths.add(path)
    # Retain binary fixtures embedded by inline runtime test modules as well.
    # These are source provenance, not executed platform compatibility cases.
    for folder in ['crates/pkl-runtime/tests/fixtures/m2-glob-lossless-source',
                   'crates/pkl-runtime/tests/fixtures/m2-supplementary-identifier/parser',
                   'crates/pkl-runtime/tests/fixtures/utf16-authority']:
        paths.update(path for path in (root / folder).rglob('*') if path.is_file())
    from vendor_sources import vendor_source_paths
    paths.add(root / 'tools/vendor_sources.py')
    paths.update(vendor_source_paths(root))
    return sorted(paths)


def run(output, bindgen, verify_contract):
    output.mkdir(parents=True, exist_ok=False)
    paths = source_paths()
    source_hashes = {}
    for path in sorted(paths):
        relative = path.relative_to(ROOT)
        destination = output / 'sources' / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(path, destination)
        source_hashes[str(relative)] = sha(destination)
    (output/'sources.json').write_text(json.dumps(source_hashes, indent=2)+'\n')
    commands = []

    def execute(name, command, allowed=(0,), timeout=600):
        result = subprocess.run(command, cwd=ROOT, capture_output=True, timeout=timeout)
        (output/f'{name}.stdout.bin').write_bytes(result.stdout)
        (output/f'{name}.stderr.bin').write_bytes(result.stderr)
        commands.append({'name': name, 'command': [str(x) for x in command], 'exit_code': result.returncode})
        (output/'commands.json').write_text(json.dumps(commands, indent=2)+'\n')
        if result.returncode not in allowed:
            raise ValueError(f'{name} failed with {result.returncode}; raw logs: {output}')
        return result

    version = execute('bindgen-version', [str(bindgen), '--version']).stdout
    if version != b'wasm-bindgen 0.2.108\n':
        raise ValueError('wasm-bindgen CLI must be exactly 0.2.108')
    execute('rust-version', ['rustc', '-Vv'])
    execute('native-build', ['cargo', 'build', '-p', 'pkl-runtime', '--example', 'semantics_probe', '--locked'])
    execute('napi-build', ['cargo', 'build', '-p', 'pkl-napi', '--release', '--locked'])
    execute('wasm-build', ['cargo', 'build', '-p', 'pkl-wasm', '--target', 'wasm32-unknown-unknown', '--release', '--locked'])
    products = cargo_products()
    wasm_dir = output/'wasm'
    execute('wasm-bindgen', [str(bindgen), '--target', 'web', '--out-dir', str(wasm_dir), str(products['wasm'])])
    addon = output/'pkl_napi.node'
    shutil.copyfile(products['napi'], addon)
    native = products['native']
    copied_native = output/native.name
    shutil.copy2(native, copied_native)
    native = copied_native
    execute('oracle-native', [sys.executable, 'tools/m1_semantics.py', '--candidate', str(native), '--output', str(output/'oracle-native')], allowed=(0, 1))
    execute('platforms', ['node', 'tools/platform/run.mjs', str(output/'execution'), str(wasm_dir), str(addon), str(ROOT/'tests/fixtures/m1/semantics/cases.json')])
    cases = generate_cases()['cases']
    captures = {'java': (output/'oracle-native/java.stdout.bin').read_bytes(),
                'rust': (output/'oracle-native/rust.stdout.bin').read_bytes(),
                'node': (output/'execution/node.stdout.bin').read_bytes(),
                'browser': (output/'execution/browser.stdout.bin').read_bytes()}
    rows = compare(cases, captures)
    execution = json.loads((output/'execution/execution.json').read_text())
    check_scenarios(execution)
    stable = all(sha(ROOT/path) == digest for path, digest in source_hashes.items())
    mismatch = any(not all(row['oracle_matches'].values()) for row in rows)
    report = {'schema': 1, 'scope': 'M1 architecture experiment, not product API or full Pkl compatibility',
              'platform': platform.platform(), 'source_stability': stable,
              'source_hashes': source_hashes,
              'artifacts': {str(path.relative_to(output)): sha(path) for path in [addon, native, *wasm_dir.glob('*')] if path.is_file()},
              'wasm_bindgen_sha256': sha(bindgen),
              'total': len(rows), 'oracle_passed': {name: sum(row['oracle_matches'][name] for row in rows) for name in ['rust', 'node', 'browser']},
              'shared_engine_passed': sum(row['shared_engine_match'] for row in rows),
              'compatibility_status': 'fail' if mismatch else 'pass',
              'known_gaps': sorted(KNOWN_GAPS),
              'experiment_contract_status': 'pass' if stable and contract_matches(rows) else 'fail',
              'execution': execution, 'comparisons': rows}
    (output/'report.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps({key: report[key] for key in ['total', 'oracle_passed', 'shared_engine_passed', 'compatibility_status', 'experiment_contract_status']}))
    if not stable:
        raise ValueError('source changed during acquisition; evidence inconclusive')
    return int(not contract_matches(rows)) if verify_contract else int(mismatch)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--wasm-bindgen', type=Path, default=ROOT/'.cache/m1-tools/bin/wasm-bindgen')
    parser.add_argument('--verify-contract', action='store_true')
    args = parser.parse_args()
    return run(args.output.resolve(), args.wasm_bindgen.resolve(), args.verify_contract)


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f'M1 platforms: {error}', file=sys.stderr)
        sys.exit(2)

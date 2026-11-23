#!/usr/bin/env python3
"""M1 primitive numeric/regex evidence. Never treats known mismatches as passes."""
import argparse
import hashlib
import json
from pathlib import Path
import struct
import subprocess
import sys

from oracle import ensure_oracle, sha256

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "tests/fixtures/m1/semantics/cases.json"


def generate_cases():
    cases = []
    def add(identifier, family, value):
        cases.append(dict(id=identifier, family=family, input=value))
    for op in ['add', 'sub', 'mul', 'div', 'rem']:
        for i, (a, b) in enumerate([(2, 3), (-7, 3), (7, -3), (-7, -3), (2**63-1, 1), (-2**63, -1), (-2**63, 0), (2**62, 4)]):
            add(f'int-{op}-{i}', 'numeric', f'{op}\t{a}\t{b}')
    bits = [0, 0x8000000000000000, 1, 2, 3, 0x000fffffffffffff, 0x0010000000000000, 0x7fefffffffffffff, 0x7ff0000000000000, 0xfff0000000000000, 0x7ff8000000000000, 0xfff0000000000001]
    values = [0.001, 0.0001, 9999999., 1e7, 1e23, -0.5, 0.5, 1.5, 2.5, -1.5, -2.5, 2**53, float(2**63), -float(2**63), 709.782712893384, 710., -745., 1e20, 1e100, 3.141592653589793]
    bits += [struct.unpack('>Q', struct.pack('>d', x))[0] for x in values]
    for op in ['format', 'sqrt', 'sin', 'cos', 'tan', 'exp', 'log', 'cbrt', 'rint', 'ceil', 'floor']:
        for b in bits:
            add(f'{op}-{b:016x}', 'numeric', f'{op}\t{b:016x}')
    regex_cases = [
        ('ascii-digit', 'find', r'\d+', '1١２3'), ('unicode-digit', 'find', r'(?U)\d+', '1١２3'),
        ('ascii-word', 'find', r'\w+', 'aé_β9'), ('unicode-word', 'find', r'(?U)\w+', 'aé_β9'),
        ('unicode-case', 'find', '(?i)k', 'KkK'), ('unicode-category', 'find', r'\p{L}+', 'é漢😀'),
        ('unicode-15-vs-16', 'find', r'\p{L}+', '\U00001c89\U00011f02'),
        ('supplementary', 'find', '(.)', 'a😀b'), ('zero-width-supplementary', 'find', '', '😀'),
        ('unmatched-capture', 'find', '(a)?(b)', 'b ab'), ('lookbehind', 'find', '(?<=a)b', 'ab cb'),
        ('variable-lookbehind', 'find', '(?<=a+)b', 'aaab'), ('backreference', 'find', r'(a+)b\1', 'aabaa aba'),
        ('named-backreference', 'find', r'(?<x>a+)b\k<x>', 'aabaa'), ('possessive', 'matches', 'a++a', 'aaa'),
        ('atomic', 'matches', '(?>a+)a', 'aaa'), ('intersection', 'find', '[a-z&&[^aeiou]]+', 'abcdef'),
        ('quoted', 'find', r'\Qa.b\E', 'a.b axb'), ('linebreak', 'find', r'\R', '\r\n\n\u2028'),
        ('dot-linebreak', 'find', '.', 'a\r\n\u0085\u2028b'), ('end-anchor', 'find', '$', 'a\r\n'),
        ('invalid-group', 'find', '(', 'a'), ('invalid-class', 'find', '[', 'a'),
        ('invalid-range', 'find', '[z-a]', 'a'), ('invalid-escape', 'find', r'\q', 'a'),
        ('invalid-unicode', 'find', '😀(', 'a'), ('invalid-named', 'find', '(?<1>a)', 'a'),
        ('numbered-replace', 'replace', '(a)(b)', 'ab ab', '$2$1'),
        ('long-number-replace', 'replace', '(a)', 'a', '$12'),
        ('named-replace', 'replace', '(?<x>a)', 'a', '${x}${x}'),
        ('optional-named-replace', 'replace', '(?<x>a)?b', 'b', '${x}'),
        ('escaped-replace', 'replace', 'a', 'a', r'\$\\'),
        ('bad-dollar-replace', 'replace', 'a', 'a', '$'),
        ('bad-slash-replace', 'replace', 'a', 'a', '\\'),
        ('bad-number-replace', 'replace', '(a)', 'a', '$2'),
        ('bad-name-replace', 'replace', 'a', 'a', '${missing}'),
        ('empty-name-replace', 'replace', 'a', 'a', '${}'),
        ('unclosed-name-replace', 'replace', 'a', 'a', '${missing'),
        ('no-match-bad-replace', 'replace', 'a', 'b', '$'),
        ('zero-width-replace', 'replace', '(?=b)', 'ab', 'X'),
        ('quoted-reserved-escape', 'find', r'\Q\q\E', r'\q'),
        ('commented-reserved-escape', 'find', '(?x)a # \\q\nb', 'ab'),
        ('scoped-comment-escape', 'find', '(?x:a #\\q\n)b', 'ab'),
        ('scoped-flag-reset', 'find', r'(?x:a)#\q', 'a#q'),
        ('invalid-range-before-escape', 'find', r'[z-a]\q', ''),
        ('unclosed-group-after-escape', 'find', r'(\q', ''),
        ('reserved-U', 'find', r'\U', '')]
    for i, text in enumerate(['', 'a', '😀', 'a😀b']):
        regex_cases.append((f'cursor-empty-{i}', 'cursor', '', text))
    for i, text in enumerate(['', 'a😀b', 'a\r\n\u0085\u2028b']):
        regex_cases.append((f'cursor-dot-{i}', 'cursor', '.', text))
    for identifier, op, pattern, source, *replacement in regex_cases:
        add('regex-'+identifier, 'regex', '\t'.join([op, *[x.encode().hex() for x in [pattern, source, *replacement]]]))
    x = 0x4d31504b4c32303236
    for i in range(4096):
        x ^= (x << 13) & ((1 << 64)-1)
        x ^= x >> 7
        x ^= (x << 17) & ((1 << 64)-1)
        x &= (1 << 64)-1
        add(f'format-generated-{i:04}', 'numeric', f'format\t{x:016x}')
        if i < 256:
            for op in ['sin', 'cos', 'tan', 'exp', 'log', 'cbrt']:
                add(f'{op}-generated-{i:04}', 'numeric', f'{op}\t{x:016x}')
    return dict(schema=1, seed='4d31504b4c32303236', scope='primitive operations; not full Pkl evaluation', cases=cases)


def decode_capture(data, count):
    """Validate the complete hex-line transport; never trim bytes or omit cases."""
    if not data.endswith(b'\n'):
        raise ValueError('missing final transport newline')
    lines = data[:-1].split(b'\n')
    if len(lines) != count:
        raise ValueError(f'expected {count} outputs, got {len(lines)}')
    result = []
    for line in lines:
        if len(line) % 2 or any(c not in b'0123456789abcdef' for c in line):
            raise ValueError('noncanonical hex transport')
        result.append(bytes.fromhex(line.decode('ascii')))
    return result


def run(output, candidate):
    metadata = ensure_oracle(ROOT, fetch=False)
    output.mkdir(parents=True, exist_ok=False)
    manifest = json.loads(FIXTURE.read_text())
    if manifest != generate_cases():
        raise ValueError('case inventory is stale')
    cases = manifest['cases']
    if not cases or len({c['id'] for c in cases}) != len(cases):
        raise ValueError('empty or duplicate case inventory')
    payload = ''.join(c['family']+'\t'+c['input']+'\n' for c in cases).encode()
    (output / 'input.tsv').write_bytes(payload)
    (output / 'cases.json').write_bytes(FIXTURE.read_bytes())
    probe = ROOT / 'tools/java/M1SemanticsProbe.java'
    java = [*metadata['command'][:-1], str(probe)]
    commands = {'java': java, 'rust': [str(candidate)]}
    records = {}
    for name, command in commands.items():
        process = subprocess.run(command, input=payload, capture_output=True, cwd=ROOT,
                                 env=metadata['environment'], timeout=180)
        for channel in ('stdout', 'stderr'):
            (output / f'{name}.{channel}.bin').write_bytes(getattr(process, channel))
        records[name] = dict(command=command, exit_code=process.returncode,
                             stdout_sha256=hashlib.sha256(process.stdout).hexdigest(),
                             stderr_sha256=hashlib.sha256(process.stderr).hexdigest())
        if process.returncode != 0 or process.stderr:
            (output / 'process-failure.json').write_text(json.dumps(records, indent=2)+'\n')
            raise ValueError(f'{name} failed; raw captures preserved')
    expected = decode_capture((output / 'java.stdout.bin').read_bytes(), len(cases))
    actual = decode_capture((output / 'rust.stdout.bin').read_bytes(), len(cases))
    comparisons = []
    for case, a, b in zip(cases, expected, actual, strict=True):
        comparison = dict(id=case['id'], family=case['family'], operation=case['input'].split('\t')[0],
                          status='pass' if a == b else 'mismatch',
                          java_sha256=hashlib.sha256(a).hexdigest(), rust_sha256=hashlib.sha256(b).hexdigest())
        if a != b:
            comparison.update(java_hex=a.hex(), rust_hex=b.hex(), input=case['input'])
        comparisons.append(comparison)
    source_paths = ['Cargo.lock', 'Cargo.toml', 'rust-toolchain.toml', 'crates/pkl-runtime/Cargo.toml',
                    'crates/pkl-runtime/examples/semantics_probe.rs', 'crates/pkl-runtime/src/numeric.rs',
                    'crates/pkl-runtime/src/regex.rs', 'tools/java/M1SemanticsProbe.java', 'tools/m1_semantics.py',
                    'tests/fixtures/m1/semantics/references/fdlibm-s_cbrt.c']
    source_paths += [p.relative_to(ROOT).as_posix() for p in sorted(
        (ROOT/'tests/fixtures/m1/semantics/references').iterdir())
        if p.is_file() and p.relative_to(ROOT).as_posix() not in source_paths]
    from vendor_sources import vendor_source_paths
    source_paths.append('tools/vendor_sources.py')
    source_paths.extend(path.relative_to(ROOT).as_posix() for path in vendor_source_paths(ROOT))
    java_regex_dir = ROOT / 'crates/pkl-runtime/src/java_regex'
    if java_regex_dir.is_dir():
        source_paths.extend(
            p.relative_to(ROOT).as_posix()
            for p in sorted(java_regex_dir.iterdir())
            if p.is_file()
        )
    report = dict(schema=1, scope=manifest['scope'], oracle=metadata,
                  candidate=dict(path=str(candidate), sha256=sha256(candidate)),
                  sources={p: sha256(ROOT/p) for p in source_paths}, fixture_sha256=sha256(FIXTURE),
                  records=records, total=len(cases), passed=sum(c['status']=='pass' for c in comparisons),
                  mismatched=sum(c['status']=='mismatch' for c in comparisons), comparisons=comparisons)
    for relative in source_paths:
        destination = output / 'sources' / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes((ROOT / relative).read_bytes())
    (output / 'report.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps({k: report[k] for k in ['total', 'passed', 'mismatched']}))
    return 1 if report['mismatched'] else 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--generate', action='store_true')
    parser.add_argument('--check', action='store_true')
    parser.add_argument('--output', type=Path)
    parser.add_argument('--candidate', type=Path, default=ROOT/'target/debug/examples/semantics_probe')
    args = parser.parse_args()
    content = json.dumps(generate_cases(), ensure_ascii=False, indent=2)+'\n'
    if args.generate:
        FIXTURE.write_text(content)
        return 0
    if args.check:
        if FIXTURE.read_text() != content:
            raise ValueError('case inventory is stale')
        print(f'{len(generate_cases()["cases"])} primitive cases; inventory fresh')
        return 0
    if args.output is None:
        parser.error('--output is required for execution')
    return run(args.output.resolve(), args.candidate.resolve())


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f'M1 semantics: {error}', file=sys.stderr)
        sys.exit(2)

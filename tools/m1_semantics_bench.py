#!/usr/bin/env python3
"""Bounded numeric primitive costs, with prior raw-byte correctness and explicit instability."""
import argparse
import json
from pathlib import Path
import statistics
import subprocess
import sys

from oracle import ensure_oracle, sha256

ROOT = Path(__file__).resolve().parents[1]
OPERATIONS = ('format', 'cbrt', 'sin', 'checked_add')
INPUTS = ROOT/'tests/fixtures/m1/semantics/cost-inputs.txt'
CONTRACT = dict(forks=3, repetitions=2048, warmups=12, measurements=12,
                convergence_window=4, maximum_window_drift=0.10, maximum_relative_mad=0.10,
                minimum_warmup_seconds=5, maximum_warmup_seconds=20)


def stability(samples):
    if len(samples) != CONTRACT['measurements'] or any(value <= 0 for value in samples):
        raise ValueError('missing or nonpositive measurements')
    median = statistics.median(samples)
    width = CONTRACT['convergence_window']
    first = statistics.median(samples[:width])
    last = statistics.median(samples[-width:])
    drift = abs(last-first)/first
    mad = statistics.median(abs(value-median) for value in samples)/median
    return dict(median_ns=median, first_window_median_ns=first, last_window_median_ns=last,
                relative_window_drift=drift, relative_mad=mad,
                status='stable' if drift <= CONTRACT['maximum_window_drift'] and mad <= CONTRACT['maximum_relative_mad'] else 'inconclusive')


def parse_samples(data, expected_results):
    if not data.startswith(expected_results):
        raise ValueError('timed process correctness output differs from prior oracle output')
    tail = data[len(expected_results):]
    if not tail.endswith(b'\n'):
        raise ValueError('missing final sample delimiter')
    lines = tail[:-1].split(b'\n')
    markers = [i for i, line in enumerate(lines) if line.startswith(b'warmup-status\t')]
    if len(markers) != 1:
        raise ValueError('missing or duplicate warmup status')
    marker = markers[0]
    status = lines[marker].split(b'\t')
    if len(status)!=4 or status[1] not in (b'stable',b'inconclusive') or not status[2].isdigit() or not status[3].isdigit():
        raise ValueError('invalid warmup status')
    elapsed, rounds = int(status[2]), int(status[3])
    if rounds < 1 or marker != rounds*len(OPERATIONS):
        raise ValueError('incomplete warmup inventory')
    if status[1]==b'stable' and (rounds<CONTRACT['warmups'] or elapsed<CONTRACT['minimum_warmup_seconds']*1e9):
        raise ValueError('claimed stable warmup below minimum duration or rounds')
    expected = [(operation,'warmup',sample) for sample in range(rounds) for operation in OPERATIONS]
    expected += [(operation,'measurement',sample) for operation in OPERATIONS for sample in range(CONTRACT['measurements'])]
    payload = lines[:marker]+lines[marker+1:]
    if len(payload)!=len(expected):
        raise ValueError('incomplete sample inventory')
    parsed = {operation: [] for operation in OPERATIONS}
    for line, (operation,phase,sample) in zip(payload,expected,strict=True):
        fields=line.split(b'\t')
        if len(fields)!=6 or fields[:4]!=[b'sample',operation.encode(),phase.encode(),str(sample).encode()]:
            raise ValueError('unexpected sample order, identity, or phase')
        if not fields[4].isdigit() or int(fields[4])<=0:
            raise ValueError('invalid duration')
        if len(fields[5])!=16 or any(c not in b'0123456789abcdef' for c in fields[5]):
            raise ValueError('invalid checksum')
        parsed[operation].append(dict(phase=phase,sample=sample,elapsed_ns=int(fields[4]),checksum=fields[5].decode()))
    last_windows={op:stability([s['elapsed_ns'] for s in parsed[op] if s['phase']=='warmup'][-12:])
                  for op in OPERATIONS} if rounds>=12 else {}
    if status[1]==b'stable' and any(row['status']!='stable' for row in last_windows.values()):
        raise ValueError('claimed stable warmup has nonconverged final windows')
    return parsed, dict(status=status[1].decode(), elapsed_ns=elapsed, rounds=rounds, last_windows=last_windows)


def run(output, candidate):
    output.mkdir(parents=True, exist_ok=False)
    oracle = ensure_oracle(ROOT, fetch=False, profile='performance')
    inputs = INPUTS.read_bytes()
    if not inputs or not inputs.endswith(b'\n'):
        raise ValueError('invalid numeric cost inputs')
    rows = inputs[:-1].split(b'\n')
    if any(len(row) != 16 or any(c not in b'0123456789abcdef' for c in row) for row in rows):
        raise ValueError('noncanonical numeric cost inputs')
    (output/'inputs.txt').write_bytes(inputs)
    parameters = [str(INPUTS), str(CONTRACT['repetitions']), str(CONTRACT['warmups']), str(CONTRACT['measurements'])]
    commands = dict(java=[*oracle['command'][:-1], str(ROOT/'tools/java/M1SemanticsBench.java')], rust=[str(candidate)])
    report = dict(schema=1, scope='selected numeric primitive loops, not Pkl evaluation or product performance',
                  contract=CONTRACT, oracle=oracle, inputs_sha256=sha256(INPUTS), input_count=len(rows),
                  candidate=dict(path=str(candidate), sha256=sha256(candidate)), processes=[], status='running')
    source_paths = ['crates/pkl-runtime/src/numeric.rs', 'crates/pkl-runtime/examples/semantics_bench.rs',
                    'tools/java/M1SemanticsBench.java', 'tools/m1_semantics_bench.py', 'Cargo.lock', 'rust-toolchain.toml']
    report['sources'] = {path: sha256(ROOT/path) for path in source_paths}
    for path in source_paths:
        target = output/'sources'/path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes((ROOT/path).read_bytes())
    def capture(runtime, mode, label):
        command = [*commands[runtime], mode, *parameters]
        process = subprocess.run(command, cwd=ROOT, env=oracle['environment'], capture_output=True, timeout=180)
        for channel in ('stdout', 'stderr'):
            (output/f'{label}.{channel}.bin').write_bytes(getattr(process, channel))
        report['processes'].append(dict(label=label, command=command, exit_code=process.returncode,
                                       stdout_sha256=sha256(output/f'{label}.stdout.bin'), stderr_sha256=sha256(output/f'{label}.stderr.bin')))
        if process.returncode or process.stderr:
            raise ValueError(f'{label} process failed; raw captures preserved')
        return process.stdout
    try:
        expected = capture('java', 'check', 'java-check')
        actual = capture('rust', 'check', 'rust-check')
        expected_lines = expected.splitlines(keepends=True)
        if len(expected_lines) != len(rows)*len(OPERATIONS) or any(not row.startswith(b'result\t') or not row.endswith(b'\n') for row in expected_lines):
            raise ValueError('incomplete correctness output')
        if expected != actual:
            report['status'] = 'correctness-mismatch'
            raise ValueError('prior raw-byte comparison failed; timing not executed')
        report['correctness'] = 'complete raw stdout/stderr/exit comparison passed before timing'
        all_samples = dict(java=[], rust=[])
        warmup_records = dict(java=[], rust=[])
        for fork in range(CONTRACT['forks']):
            order = ('rust', 'java') if fork % 2 == 0 else ('java', 'rust')
            for runtime in order:
                output_bytes = capture(runtime, 'bench', f'{runtime}-fork-{fork}')
                samples, warmup = parse_samples(output_bytes, expected)
                all_samples[runtime].append(samples)
                warmup_records[runtime].append(warmup)
        checksums = {op: {sample['checksum'] for runtime in all_samples.values() for fork in runtime for sample in fork[op]} for op in OPERATIONS}
        if any(len(values) != 1 for values in checksums.values()):
            raise ValueError('timed operation checksums differ; timings ineligible')
        report['operations_per_sample'] = len(rows) * CONTRACT['repetitions']
        summaries = {}
        for operation in OPERATIONS:
            summaries[operation] = {}
            for runtime in ('rust', 'java'):
                summaries[operation][runtime] = [stability([s['elapsed_ns'] for s in fork[operation] if s['phase']=='measurement']) for fork in all_samples[runtime]]
            rust_median = statistics.median(row['median_ns'] for row in summaries[operation]['rust'])
            java_median = statistics.median(row['median_ns'] for row in summaries[operation]['java'])
            summaries[operation]['observed_rust_java_ratio'] = rust_median/java_median
            summaries[operation]['rust_ns_per_operation'] = rust_median/report['operations_per_sample']
            summaries[operation]['java_ns_per_operation'] = java_median/report['operations_per_sample']
            summaries[operation]['status'] = 'stable-primitive-observation' if all(row['status']=='stable' for runtime in ('rust','java') for row in summaries[operation][runtime]) and all(row['status']=='stable' for runtime in ('rust','java') for row in warmup_records[runtime]) else 'inconclusive'
        report.update(samples=all_samples, warmup=warmup_records, summaries=summaries,
                      status='complete-primitive-observation' if all(s['status']=='stable-primitive-observation' for s in summaries.values()) else 'inconclusive',
                      limitations=['No Pkl evaluator, renderer, diagnostic, FFI, startup, or RSS costs',
                                   'String formatting includes identical ASCII checksum loops; integer workload includes overflow outcomes',
                                   'Java JIT warmup assessed only by measured window drift/MAD, not compilation-log proof',
                                   'No product performance-target verdict follows from these primitive observations'])
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        if report['status']=='running': report['status']='environment-or-contract-error'
        report['error']=str(error)
        (output/'report.json').write_text(json.dumps(report, indent=2)+'\n')
        raise
    (output/'report.json').write_text(json.dumps(report, indent=2)+'\n')
    print(json.dumps({op: {'status': value['status'], 'observed_rust_java_ratio': value['observed_rust_java_ratio']} for op,value in report['summaries'].items()}))
    return 0 if report['status']=='complete-primitive-observation' else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--candidate', type=Path, default=ROOT/'target/release/examples/semantics_bench')
    args = parser.parse_args()
    return run(args.output.resolve(), args.candidate.resolve())


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f'M1 numeric costs: {error}', file=sys.stderr)
        sys.exit(2)

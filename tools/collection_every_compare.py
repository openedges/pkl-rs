"""Strict raw comparator for the 64-row collection every authority.

The caller must separately verify fixture provenance, source/artifact identities,
process commands and captured process-log identities. This module verifies complete
per-row raw observations and the process protocol; it does not assign a release gate.
"""
from __future__ import annotations

from pathlib import Path
import re

COMMON = frozenset({
    'expression.bin', 'expression.code-units.bin', 'result.status.bin',
    'result.stdout.bin', 'result.stdout.code-units.bin',
    'result.stderr.bin', 'result.stderr.code-units.bin',
})
CANDIDATE_EVENT = frozenset({
    'candidate.disposition.bin', 'candidate.phase.bin',
    'candidate.reason.bin', 'candidate.callback-origin.bin',
})
CANDIDATE_API_NATIVE = frozenset({'candidate.api-error-kind.bin', 'candidate.callback-origin.bin'})
CANDIDATE_API_PORTABLE = frozenset({'candidate.api-error-name.bin', 'candidate.callback-origin.bin'})
LOG_FIELDS = ('level', 'message', 'module-uri', 'member', 'position')
LOG_FILE = re.compile(r'log-[0-9]{3}\.(?:level|message|module-uri|member|position)\.(?:bin|code-units\.bin)\Z')
MAX_LOGS_PER_ROW = 999


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def exact_names(folder: Path) -> set[str]:
    require(folder.is_dir() and not folder.is_symlink(), f'missing/linked row directory: {folder}')
    names = set()
    for path in folder.iterdir():
        require(path.is_file() and not path.is_symlink(), f'nonregular row member: {path}')
        names.add(path.name)
    return names


def count(folder: Path) -> int:
    raw = (folder / 'logs.count.bin').read_bytes()
    require(re.fullmatch(rb'(?:0|[1-9][0-9]*)\n', raw) is not None, 'invalid logger count bytes')
    value = int(raw)
    require(value <= MAX_LOGS_PER_ROW, 'logger count exceeds file-name bound')
    return value


def log_names(total: int) -> set[str]:
    require(0 <= total <= MAX_LOGS_PER_ROW, 'invalid logger count')
    return {'logs.count.bin'} | {
        f'log-{index:03d}.{field}.{suffix}'
        for index in range(total)
        for field in LOG_FIELDS
        for suffix in ('bin', 'code-units.bin')
    }


def compare_row(java: Path, candidate: Path) -> dict:
    java = Path(java)
    candidate = Path(candidate)
    java_names, candidate_names = exact_names(java), exact_names(candidate)
    java_total = count(java)
    candidate_total = count(candidate)
    java_expected_logs = log_names(java_total)
    candidate_expected_logs = log_names(candidate_total)
    java_logs = {name for name in java_names if name == 'logs.count.bin' or LOG_FILE.fullmatch(name)}
    candidate_logs = {name for name in candidate_names if name == 'logs.count.bin' or LOG_FILE.fullmatch(name)}
    require(java_logs == java_expected_logs, 'missing or extra Java logger channel')
    require(candidate_logs == candidate_expected_logs, 'missing or extra candidate logger channel')
    require(COMMON <= java_names and java_names - java_expected_logs - COMMON == {
        'mode.bin', 'mode.code-units.bin', 'result.error-class.bin',
        'result.error-class.code-units.bin', 'result.exception-to-string.bin',
        'result.exception-to-string.code-units.bin', 'result.cause-class.bin',
        'result.cause-class.code-units.bin', 'result.cause-message.bin',
        'result.cause-message.code-units.bin', 'result.cause-to-string.bin',
        'result.cause-to-string.code-units.bin', 'result.java-type.bin',
        'result.java-type.code-units.bin',
    }, 'Java row channel set differs')
    java_status = (java / 'result.status.bin').read_bytes()
    require(java_status in (b'success\n', b'error\n'), 'invalid Java row status')
    status = (candidate / 'result.status.bin').read_bytes()
    require(status in (b'success\n', b'error\n', b'candidate-error\n', b'api-exception\n'),
            'invalid candidate status')
    if status == b'api-exception\n':
        # Rust exposes its ErrorKind; JS exposes the thrown exception name.
        # Neither channel is a Java exception class or a compatibility pass.
        api_meta = (CANDIDATE_API_NATIVE if 'candidate.api-error-kind.bin' in candidate_names
                    else CANDIDATE_API_PORTABLE)
        meta = api_meta
    else:
        meta = CANDIDATE_EVENT
    require(candidate_names == COMMON | meta | candidate_expected_logs,
            'candidate row channel set differs')
    if status != b'api-exception\n':
        disposition = (candidate / 'candidate.disposition.bin').read_bytes()
        require((status != b'error\n' or disposition == b'language-error')
                and (status != b'candidate-error\n' or disposition != b'language-error')
                and (status != b'success\n' or disposition == b''),
                'candidate status/disposition inconsistent')
    shared = COMMON | java_expected_logs | candidate_expected_logs
    mismatches = [name for name in sorted(shared)
                  if name not in java_names or name not in candidate_names
                  or (java / name).read_bytes() != (candidate / name).read_bytes()]
    return {'java_logs': java_total, 'candidate_logs': candidate_total,
            'status': status.decode('ascii').strip(),
            'different_channels': mismatches, 'raw_match': not mismatches}


def compare_session(java: Path, candidate: Path, rows: list[dict]) -> dict:
    java, candidate = Path(java), Path(candidate)
    require(rows and all(re.fullmatch('[a-z0-9-]+', row['id']) for row in rows),
            'invalid ordered row identities')
    directories = {f'{i:03d}-{row["id"]}' for i, row in enumerate(rows)}
    require({p.name for p in candidate.iterdir() if p.is_dir()} == directories,
            'candidate row coverage/order differs')
    require({p.name for p in java.iterdir() if p.is_dir()} == directories,
            'Java row coverage/order differs')
    outcomes = []
    protocols = {label: [f'session-open mode=string rows={len(rows)}']
                 for label in ('Java', 'candidate')}
    successes = {'Java': 0, 'candidate': 0}
    for index, row in enumerate(rows):
        name = f'{index:03d}-{row["id"]}'
        actual = compare_row(java / name, candidate / name)
        require((java / name / 'expression.bin').read_bytes()
                == row['expression'].encode('utf-8', errors='replace')
                and (java / name / 'expression.code-units.bin').read_bytes()
                == row['expression'].encode('utf-16-be', errors='surrogatepass'),
                f'Java expression differs from authored row {index}')
        statuses = {'Java': (java / name / 'result.status.bin').read_bytes().decode('ascii').strip(),
                    'candidate': actual['status']}
        for label, status in statuses.items():
            successes[label] += status == 'success'
            protocols[label].extend((f'call-enter=string-{index}-{row["id"]}',
                                     f'call-exit=string-{index}-{row["id"]} status={status}'))
        outcomes.append({'id': row['id'], **actual})
    for label, protocol in protocols.items():
        protocol.append(f'session-closed mode=string rows={len(rows)} public-calls={len(rows)} '
                        f'success={successes[label]} errors={len(rows)-successes[label]}')
        base = java if label == 'Java' else candidate
        expected_stdout = ('\n'.join(protocol) + '\n').encode()
        require((base / 'process.stdout.bin').read_bytes() == expected_stdout,
                f'{label} complete stdout protocol differs')
        require((base / 'process.stderr.bin').read_bytes() == b'',
                f'{label} process stderr differs')
    differences = [channel for channel in ('stdout', 'stderr')
                   if (java / f'process.{channel}.bin').read_bytes()
                   != (candidate / f'process.{channel}.bin').read_bytes()]
    return {'rows': outcomes, 'raw_matches': sum(row['raw_match'] for row in outcomes),
            'different_process_channels': differences,
            'full_raw_match': all(row['raw_match'] for row in outcomes) and not differences}

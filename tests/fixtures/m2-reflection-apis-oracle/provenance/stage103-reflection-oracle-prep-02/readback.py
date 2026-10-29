"""Bounded offline readback of the Stage102 Mapping raw authority cohort."""
import base64, collections, stat
import hashlib
import json
import subprocess
import time
from pathlib import Path

PREP = Path(__file__).resolve().parent
ROOT = PREP.parents[2]
OUT = ROOT/'target/m2/stage103-reflection-oracle-readback-01'
COHORTS = [('stage103-reflection-main166-oracle-01', 166, 12, 'target/m2/stage103-reflection-oracle-prep-02/inputs/main166.json'), ('stage103-reflection-metadata47-oracle-01', 47, 26, 'target/m2/stage103-reflection-oracle-prep-02/inputs/metadata47.json'), ('stage103-reflection-reference6-oracle-01', 6, 2, 'target/m2/stage103-reflection-oracle-prep-02/inputs/reference6.json'), ('stage103-reflection-legal-reference6-oracle-01', 6, 2, 'target/m2/stage103-reflection-oracle-prep-02/inputs/legal-reference6.json')]
if OUT.exists():raise ValueError('fresh offline readback output required')
OUT.mkdir()


def identity(path):
    path = Path(path)
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return dict(path=str(path), bytes=path.stat().st_size, sha256=h.hexdigest())


def checked(record):
    actual = identity(record['path'])
    if any(actual[k] != record[k] for k in ['bytes', 'sha256']):
        raise ValueError('changed record: ' + record['path'])
    return actual


report = dict(schema_version=1, kind='Stage103-required-reflection225-independent-full-raw-readback',
              complete=False, started_epoch=time.time(), reports=[], errors=[],
              Rust_evaluations=0, performance_claim=False)
statuses = collections.Counter()
rows = logs = records = 0
try:
    for name, queries, session_count, matrix_path in COHORTS:
        folder = ROOT / 'target/m2' / name
        path = folder / 'report.json'
        before = identity(path)
        data = json.loads(path.read_bytes())
        if not all(data[k] for k in ['complete', 'source_and_runtime_stable', 'materialized_stable']) or data['repeat_differences']:
            raise ValueError('incomplete acquisition: ' + name)
        if len(data['sessions']) != session_count or sum(len(p['queries']) for p in data['programs']) != queries:
            raise ValueError('cohort count: ' + name)
        matrix = ROOT/matrix_path
        wanted = json.loads(matrix.read_bytes())['programs']
        if [{k:v for k,v in p.items() if k != 'actual_inputs'} for p in data['programs']] != wanted:
            raise ValueError('original program/source/query/URI/profile contract changed')
        if [(s['program'],s['repeat']) for s in data['sessions']] != [(p['id'],r) for p in wanted for r in range(2)]:
            raise ValueError('program-repeat order differs')
        for dependency in data.get('auxiliary_inputs',[]):
            dep=ROOT/dependency['path']; actual=identity(dep)
            if dep.is_symlink() or not dep.is_file() or stat.S_IMODE(dep.stat().st_mode)!=dependency['mode'] or any(actual[k]!=dependency[k] for k in ['bytes','sha256']) or dep.as_uri()!=dependency['source_uri']:raise ValueError('auxiliary changed after acquisition')
        for program in wanted:
            pair=[session for session in data['sessions'] if session['program']==program['id']]
            if len(pair)!=2:raise ValueError('repeat pair')
            for left,right in zip(pair[0]['rows'],pair[1]['rows']):
                if {k:(v['bytes'],v['sha256']) for k,v in left['raw_files'].items()}!={k:(v['bytes'],v['sha256']) for k,v in right['raw_files'].items()}:raise ValueError('raw repeat differs')
        if len(data['query_dispositions']) != queries*2 or data['query_disposition_counts'] != {'recorded':queries*2}:
            raise ValueError('unrun or partial query disposition retained; acquisition incomplete')
        for p in wanted:
            group=folder/'programs'/p['id']
            if (group/'module.pkl').read_bytes()!=p['source'].encode() or (group/'queries.tsv').read_bytes()!=''.join('string\t'+q['id']+'\t'+base64.b64encode(q['expression'].encode()).decode('ascii')+'\n' for q in p['queries']).encode():
                raise ValueError('physical source/TSV bytes differ')
        for field in ['input_before', 'input_after', 'materialized_before', 'materialized_after']:
            for entry in data[field].values():
                checked(entry)
                records += 1
        for field in ['stdout', 'stderr']:
            checked(data['jdk_preflight'][field])
        capture = json.loads((folder / 'capture-process.json').read_bytes())
        checked(capture['report'])
        if capture['exit_code'] != 0:
            raise ValueError('capture self-reported failure: ' + name)
        cohort_counts = collections.Counter()
        sessions = []
        for session in data['sessions']:
            if session['exit_code'] != 0 or not session['owned_child_reaped'] or session['cleanup_events'] or not session['owned_processes']['root_observed'] or session['owned_processes']['terminal_live']:
                raise ValueError('invalid Java lifecycle: ' + name)
            group = folder / 'programs' / session['program'] / ('repeat-' + str(session['repeat']))
            process_file = group / 'process.json'
            if json.loads(process_file.read_bytes()) != session:
                raise ValueError('process report mismatch: ' + str(process_file))
            checked(session['stdout'])
            checked(session['stderr'])
            n=len(session['rows']);success=sum(r['status']=='success' for r in session['rows'])
            protocol=f'session-open mode=string rows={n}\n'+''.join(f'call-enter=string-{i}-{r["id"]}\ncall-exit=string-{i}-{r["id"]} status={r["status"]}\n' for i,r in enumerate(session['rows']))+f'session-closed mode=string rows={n} public-calls={n} success={success} errors={n-success}\n'
            if (group/'process.stdout.bin').read_bytes()!=protocol.encode() or (group/'process.stderr.bin').read_bytes():
                raise ValueError('complete raw child lifecycle protocol differs')
            for row in session['rows']:
                raw = row['raw_files']
                for entry in raw.values():
                    checked(entry)
                    records += 1
                directory = ROOT / row['directory']
                if set(raw) != {p.name for p in directory.iterdir()}:
                    raise ValueError('row file membership')
                actual_status = (directory / 'result.status.bin').read_bytes()
                if actual_status != (row['status'] + '\n').encode() or row['status'] not in ['success', 'error', 'oracle-error', 'api-exception']:
                    raise ValueError('raw status mismatch')
                if (directory / 'expression.bin').read_bytes() != row['expression'].encode():
                    raise ValueError('expression mismatch')
                count = int((directory / 'logs.count.bin').read_bytes())
                if count != row['logger_events']:
                    raise ValueError('logger count mismatch')
                for key in raw:
                    if key.endswith('.code-units.bin'):
                        base = key[:-len('.code-units.bin')] + '.bin'
                        units = (directory / key).read_bytes()
                        if units.decode('utf-16-be', errors='surrogatepass').encode('utf-8', errors='replace') != (directory / base).read_bytes():
                            raise ValueError('UTF8/UTF16 disagreement')
                cohort_counts[row['status']] += 1
                rows += 1
                logs += count
            observation = subprocess.run(['/bin/ps', '-p', str(session['pid']), '-o', 'pid=,lstart=,command='], capture_output=True, check=False)
            sessions.append(dict(process_record=identity(process_file), captured_pid=session['pid'],
                                 ps_exit_code=observation.returncode, stdout_hex=observation.stdout.hex(),
                                 stderr_hex=observation.stderr.hex(),
                                 scope='Actual Popen reaping is recorded above; later PID reuse is observational only.'))
        if dict(cohort_counts) != data['statuses'] or sum(cohort_counts.values()) != queries * 2:
            raise ValueError('status/count recomputation')
        statuses.update(cohort_counts)
        if identity(path) != before:
            raise ValueError('report changed during readback')
        report['reports'].append(dict(report=before, capture_process=identity(folder / 'capture-process.json'),
                                      queries=queries, observations=queries * 2, statuses=dict(cohort_counts),
                                      sessions=sessions))
    report['complete'] = True
except Exception as error:
    report['errors'].append(dict(type=type(error).__name__, message=str(error)))
report.update(finished_epoch=time.time(), queries=sum(x[1] for x in COHORTS), observations=rows,
              statuses=dict(statuses), ordered_logger_events=logs, checked_record_occurrences=records,
              limitations=['Pinned Java authority establishes reference observations, not Rust/API/M2 compatibility.',
                           'Separate166+47+6+6 cohorts preserve225 required inputs and450 planned observations; original219/possible source diagnostics/14whole-original requirements/full reflection scope remain unchanged.',
                           'Any observed upstream internal outcomes remain oracle-error, never ordinary error or compatibility pass.',
                           'All14 original reflection/Reference modules and full20-mirror reflection/API obligations remain required; bounded219 Java authority is not their candidate success.',
                           'No giant allocations, frozen native executables, protected archives or global cache copies are inputs.'])
(OUT / 'report.json').write_text(json.dumps(report, indent=2, sort_keys=True) + '\n')
print(json.dumps({k: report[k] for k in ['complete', 'queries', 'observations', 'statuses', 'ordered_logger_events', 'errors']}))
raise SystemExit(0 if report['complete'] else 1)

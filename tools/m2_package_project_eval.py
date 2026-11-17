"""Reviewed package, project, and HTTPS execution profile adapter for all 68 blocked cases."""
import argparse
import json
import os
from pathlib import Path
import stat
import subprocess
import time

import compat
import oracle
import m2_module_eval
import m2_package_fixtures as fixtures

ADAPTER = 'package-project-eval'
REGISTRY = Path('compatibility/m2-package-project-eval.json')
PROFILE = 'pkl-0.32-package-project-eval-v1'

CASE_IDS = (
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/api/analyze1.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/basic/importGlob.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/basic/readGlob.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/badImport1.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/badImport10.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/badImport11.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/badImport12.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/badImport2.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/badImport3.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/badImport4.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/badImport6.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/badImport7.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/badImport8.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/badImport9.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/badRead1.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/badRead2.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/badRead3.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/badRead4.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/badRead8.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/badRead9.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/packages1.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/packages2.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/redirects.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/badLocalProject/dog.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/badPklProject1/bug.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/badPklProject2/bug.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/badPklProject3/bug.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/badPklProject4/bug.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/badPklProject5/bug.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/badProjectDeps1/bug.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/badProjectDeps2/bug.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/badProjectDeps3/bug.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/badProjectDeps4/bug.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/badProjectDeps5/bug.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/badProjectDeps6/bug.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/badProjectDeps7/bug.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/compactTraceMode/compactTraceMode.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/evaluatorSettings/basic.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/evaluatorSettings/nullTraceMode.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/evaluatorSettings2/badRead.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/evaluatorSettings2/goodRead.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/missingProjectDeps/bug.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/notAProject/@child/theChild.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/notAProject/badImport.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/notAProject/goodImport.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/packageWithSpaces/module with spaces.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/prettyTraceMode/prettyTraceMode.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project1/badImport1.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project1/badImport2.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project1/badImport3.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project1/badRead1.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project1/basic.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project1/directPackageImport.error.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project1/globbing.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project1/localProject.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project1/localProject2.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project1/localProjectRead.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project1/localProjectRead2.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project2/penguin.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project3/basic.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project4/main.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project5/main.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project6/children.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project6/children/a.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project6/children/b.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project6/children/c.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project7/globWildcards.pkl',
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project7/spacesInImport.pkl',
)

CASE_ID_SET = frozenset(CASE_IDS)


def build_registry(root):
    root = Path(root).resolve()
    corpus = json.loads((root / 'compatibility/corpus.json').read_bytes())
    selected = [case for case in corpus['cases'] if case['id'] in CASE_ID_SET]
    if len(selected) != len(CASE_IDS):
        raise ValueError(f'Expected {len(CASE_IDS)} cases in corpus, found {len(selected)}')
    cases = []
    for case in selected:
        cases.append(dict(
            id=case['id'],
            input=case['source']['path'],
            input_sha256=case['source']['sha256'],
            upstream_case_sha256=compat.digest(compat.encoded(case)),
        ))
    return dict(
        schema_version=1,
        adapter=ADAPTER,
        profile=PROFILE,
        baseline=corpus['baseline'],
        scope='All 68 reviewed package, project, and HTTPS execution profile cases executed with PackageFixtureServer over 12 upstream packages.',
        cases=cases,
    )


def load_registry(root):
    root = Path(root).resolve()
    return json.loads((root / REGISTRY).read_bytes())


def registry_cases(root, registry=None):
    root = Path(root).resolve()
    if registry is None:
        registry = load_registry(root)
    expected = build_registry(root)
    if (registry.get('schema_version') != expected['schema_version']
            or registry.get('adapter') != expected['adapter']
            or registry.get('profile') != expected['profile']
            or registry.get('baseline') != expected['baseline']
            or len(registry.get('cases', [])) != len(expected['cases'])):
        raise ValueError('package-project-eval registry differs from expected')
    expected_by_id = {c['id']: c for c in expected['cases']}
    result = {}
    for entry in registry['cases']:
        cid = entry['id']
        if cid not in expected_by_id:
            raise ValueError(f'unexpected case {cid} in package-project-eval registry')
        exp = expected_by_id[cid]
        if (entry['input'] != exp['input']
                or entry['input_sha256'] != exp['input_sha256']
                or entry['upstream_case_sha256'] != exp['upstream_case_sha256']):
            raise ValueError(f'package-project-eval case {cid} contract changed')
        result[cid] = entry
    return result


def profile_cases(root):
    """Generate entries for compatibility/m2-execution-profiles.json."""
    root = Path(root).resolve()
    registry = load_registry(root)
    reg_bytes = (root / REGISTRY).read_bytes()
    profile_sha = compat.digest(reg_bytes)
    return [
        dict(
            adapter=ADAPTER,
            id=c['id'],
            input_sha256=c['input_sha256'],
            profile=REGISTRY.as_posix(),
            profile_sha256=profile_sha,
            scope='Reviewed package/project/HTTPS execution profile with FixtureServer over 12 upstream packages; original CLI command/blocked record remains provenance.',
            upstream_case_sha256=c['upstream_case_sha256'],
        )
        for c in registry['cases']
    ]


def find_project_dir(workspace, input_path):
    """Walk ancestors up to input/projects looking for a PklProject regular file."""
    workspace = Path(workspace)
    target = workspace / input_path
    if not target.exists() and (workspace / 'external/pkl' / input_path).exists():
        target = workspace / 'external/pkl' / input_path
    curr = target.parent
    projects_anchor = (workspace / 'external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/projects').resolve()
    while True:
        if (curr / 'PklProject').is_file():
            try:
                return curr.resolve().relative_to(workspace.resolve()).as_posix()
            except ValueError:
                return str(curr.resolve())
        curr_resolved = curr.resolve()
        if curr_resolved == projects_anchor or curr_resolved.parent == curr_resolved:
            return None
        curr = curr.parent


def command_args(workspace, case_input, port, ca_cert_path):
    """Follow LanguageSnippetTestsEngine.kt L258-295 verbatim."""
    input_rel = Path(case_input)
    parts = input_rel.parts
    is_project = 'projects' in parts

    args = ['eval', '--no-cache']
    if is_project:
        project_dir = find_project_dir(workspace, case_input)
        if project_dir is not None:
            args.extend(['--project-dir', project_dir])
    else:
        args.append('--no-project')
        args.extend([
            '--property', 'name1=value1',
            '--property', 'name2=value2',
            '--property', '/foo/bar=foobar',
            '--env-var', 'NAME1=value1',
            '--env-var', 'NAME2=value2',
            '--env-var', '/foo/bar=foobar',
            '--env-var', 'foo bar=foo bar',
            '--env-var', 'file:///foo/bar=file:///foo/bar',
        ])
    args.extend([
        '--settings', 'pkl:settings',
        '--ca-certificates', str(ca_cert_path),
        '--test-mode',
        '--test-port', str(port),
        case_input.as_posix() if hasattr(case_input, 'as_posix') else str(case_input),
    ])
    return args


def prepare_workspace(root, workspace, output, corpus):
    """Prepare minimal workspace for package-project evaluation."""
    compat.validate_paths(root, workspace, output)
    if workspace.exists() or workspace.is_symlink():
        raise ValueError('package-project-eval workspace must be fresh')
    compat.prepare_workspace(root, workspace, include_project_fixtures=False)
    compat.write_json(workspace / compat.MARKER, dict(
        schema_version=1,
        repository=str(root.resolve()),
        workspace=str(workspace.resolve()),
        adapter=ADAPTER,
        profile=PROFILE,
        registry_sha256=oracle.sha256(root / REGISTRY),
    ))
    return compat.snapshot(workspace)


def acquire_case(root, workspace, case, metadata, binary, directory, port, ca_cert_path, timeout=60, output=None):
    root = Path(root).resolve()
    workspace = Path(workspace).resolve()
    directory = Path(directory).resolve()
    binary = Path(binary).resolve()

    input_path = Path('external/pkl') / case['upstream_case']['source']['path']
    args = command_args(workspace, input_path, port, ca_cert_path)
    state = workspace / '.state' / compat.case_directory(case['id'])
    environment = {
        **metadata['environment'],
        **case['upstream_case'].get('environment', {}),
        'HOME': str(state / 'home'),
        'TMPDIR': str(state / 'tmp'),
        'TMP': str(state / 'tmp'),
        'TEMP': str(state / 'tmp'),
    }
    prefix_java = [
        f'-Duser.home={state / "home"}' if arg.startswith('-Duser.home=')
        else f'-Djava.io.tmpdir={state / "tmp"}' if arg.startswith('-Djava.io.tmpdir=')
        else arg for arg in metadata['command']
    ]

    directory.mkdir(parents=True, exist_ok=True)
    (directory / 'input.pkl').write_bytes((workspace / input_path).read_bytes())

    execution_case = dict(case['upstream_case'], cwd='.', command=args)
    synthetic_case = dict(
        id=case['id'],
        cwd='.',
        command=args,
        environment=case['upstream_case'].get('environment', {}),
        input=str(input_path),
        input_sha256=case['input_sha256'],
        upstream_case=execution_case,
    )

    # Capture Java and Rust
    proc_java, cap_java = m2_module_eval.capture_process(
        root, workspace, synthetic_case, prefix_java, environment, directory / 'java', timeout
    )
    proc_rust, cap_rust = m2_module_eval.capture_process(
        root, workspace, synthetic_case, [str(binary)], environment, directory / 'rust', timeout
    )

    expected_exit = case['upstream_case']['expected_exit']
    st = m2_module_eval.status(proc_java, proc_rust, cap_java, cap_rust, expected_exit)
    raw_channels_match = not compat.differences(cap_java, cap_rust)
    upstream_expected_exit_match = not proc_java['timed_out'] and proc_java['exit_code'] in expected_exit

    if output is not None:
        dir_field = str(directory.relative_to(Path(output).resolve()))
    else:
        try:
            dir_field = str(directory.relative_to(directory.parents[1]))
        except Exception:
            dir_field = directory.name

    return dict(
        id=case['id'],
        adapter=ADAPTER,
        input=str(input_path),
        input_sha256=case['input_sha256'],
        directory=dir_field,
        status=st,
        raw_channels_match=raw_channels_match,
        upstream_expected_exit_match=upstream_expected_exit_match,
        java=proc_java,
        rust=proc_rust,
    )


def acquire(root, output, cases, metadata, binary, workspace=None):
    root = Path(root).resolve()
    output = Path(output).resolve()
    corpus = json.loads((root / 'compatibility/corpus.json').read_bytes())
    registry = load_registry(root)
    entries = registry_cases(root, registry)
    workspace = workspace or output.with_name(output.name + '-package-project-workspace')
    baseline = prepare_workspace(root, workspace, output, corpus)
    compat.write_json(output / 'package-project-workspace.json', baseline)
    compat.write_json(output / 'package-project-registry.json', registry)

    stage_dir = output / 'test-packages'
    fixtures.build_upstream_packages(stage_dir)

    server_output = output / 'fixture-server'
    results = []
    with fixtures.FixtureServer(stage_dir, metadata, server_output) as server:
        for case in cases:
            case_dir = output / 'cases' / compat.case_directory(case['id'])
            row = acquire_case(root, workspace, case, metadata, binary, case_dir, server.port, server.ca_path, output=output)
            results.append(row)
            print(f"{row['id']}: {row['status']}", flush=True)

    context = dict(
        profile=PROFILE,
        workspace=str(workspace),
        workers=1,
        watchdog_seconds=m2_module_eval.CAPTURE_TIMEOUT_SECONDS,
        registry_sha256=oracle.sha256(output / 'package-project-registry.json'),
        baseline_sha256=oracle.sha256(output / 'package-project-workspace.json'),
        original_case_ids=[c['id'] for c in cases],
        server_output=str(server_output),
    )
    return results, context


def verify_context(root, output, context):
    root = Path(root).resolve()
    output = Path(output).resolve()
    reg_path = output / 'package-project-registry.json'
    ws_path = output / 'package-project-workspace.json'
    if not reg_path.is_file() or not ws_path.is_file():
        raise ValueError('package-project-eval registry or workspace evidence missing')
    if (context.get('profile') != PROFILE
            or context.get('workers') != 1
            or context.get('watchdog_seconds') != m2_module_eval.CAPTURE_TIMEOUT_SECONDS
            or context.get('registry_sha256') != oracle.sha256(reg_path)
            or context.get('baseline_sha256') != oracle.sha256(ws_path)):
        raise ValueError('package-project-eval context provenance differs')
    return json.loads(ws_path.read_bytes())


def verify_case(root, output, context, case, row, metadata, binary, baseline):
    root = Path(root).resolve()
    output = Path(output).resolve()
    if case.get('adapter') != ADAPTER or row.get('adapter') != ADAPTER:
        raise ValueError('package-project-eval adapter identity changed')
    folder = output / 'cases' / compat.case_directory(case['id'])
    if not folder.is_dir():
        raise ValueError(f'package-project-eval case directory missing: {folder}')

    input_file = folder / 'input.pkl'
    if not input_file.is_file():
        raise ValueError(f'package-project-eval input file missing: {input_file}')
    if 'input_sha256' in case and oracle.sha256(input_file) != case['input_sha256']:
        raise ValueError(f'package-project-eval input snapshot changed: {case["id"]}')

    for runtime in ('java', 'rust'):
        process_path = folder / runtime / 'process.json'
        if not process_path.is_file():
            raise ValueError(f'package-project-eval {runtime} process.json missing: {case["id"]}')
        process = json.loads(process_path.read_bytes())
        if process != row[runtime]:
            raise ValueError(f'package-project-eval {runtime} process metadata differs: {case["id"]}')
        cmd = process.get('command', [])
        for flag in ('--test-mode', '--test-port', '--ca-certificates'):
            if flag not in cmd:
                raise ValueError(f'package-project-eval {runtime} command missing {flag}: {case["id"]}')
        input_path = case.get('input') or (Path('external/pkl') / case.get('upstream_case', {}).get('source', {}).get('path', '')).as_posix()
        if 'projects' in Path(input_path).parts:
            project_dir = find_project_dir(root, input_path)
            if project_dir is not None and '--project-dir' not in cmd:
                raise ValueError(f'package-project-eval {runtime} command missing --project-dir: {case["id"]}')

    captured_java = compat.load_capture(folder / 'java')
    captured_rust = compat.load_capture(folder / 'rust')
    expected_exit = case.get('upstream_case', {}).get('expected_exit') if 'upstream_case' in case else case.get('expected_exit')
    expected_status = m2_module_eval.status(row['java'], row['rust'], captured_java, captured_rust, expected_exit)
    if row['status'] != expected_status:
        raise ValueError(f'package-project-eval comparison status changed: {case["id"]}')

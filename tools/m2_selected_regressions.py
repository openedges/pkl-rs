"""Exactly two retained original t40 CLI contracts; no historical pass reuse."""
import json
import os
import stat
from pathlib import Path

import compat
import m2_module_eval
import oracle

ADAPTER = 'selected-regression-module-eval'
REGISTRY = Path('compatibility/m2-selected-regressions.json')
PROFILE = 'pkl-0.32-original-t40-file-cli-v1'
SOURCE_PROFILE = 'selected-original-t40-sources-v1'
SELECTED = ('rev1:t40-imported-declaration-authority', 'rev1:t40-foreign-namesake')
ORIGINAL_CONTRACTS = {
    'rev1:t40-imported-declaration-authority': '9822595d43e291cd42a554065aa4dc47a9a3672dc499d7c01e7c02aee845d5da',
    'rev1:t40-foreign-namesake': 'ac4587a9286abae38855980e0dacee4fef7c633b7cb3fc32b1c531b77d94ca1b',
}


def relative(name):
    path = Path(name)
    if (path.is_absolute() or '..' in path.parts or path.as_posix() != name
            or any(part == 'stage65-b-native-regressions-evidence.tar.gz.partial'
                   or 'stage86' in part.lower() or 'private8' in part.lower()
                   for part in path.parts)
            or name.endswith(('.tar', '.tar.gz', '.tar.zst', '.tgz', '.zip', '.partial'))):
        raise ValueError('excluded selected-regression path before metadata')
    return path


def source_file(root, name):
    path = root
    for part in relative(name).parts:
        path /= part
        if stat.S_ISLNK(path.lstat().st_mode):
            raise ValueError('selected original resource path contains a symlink')
    if not stat.S_ISREG(path.stat().st_mode):
        raise ValueError('selected original resource is not regular')
    return path


def build_registry(root):
    originals = json.loads((root/'compatibility/regressions.json').read_bytes())
    selected = [row for row in originals['cases'] if row['id'] in SELECTED]
    if tuple(row['id'] for row in selected) != SELECTED:
        raise ValueError('selected original t40 coverage/order differs')
    by_path = {row['path']: row for row in originals['files']}
    names = {row['path'] for case in selected for row in case['resources']}
    names.update(row['path'] for row in originals['files'] if row['kind'] == 'license')
    resources = []
    for name in sorted(names):
        path = source_file(root, name)
        row = by_path[name]
        if oracle.sha256(path) != row['sha256']:
            raise ValueError('selected original resource missing or changed: '+name)
        resources.append(dict(path=name, kind='file', sha256=row['sha256'],
                              bytes=path.stat().st_size, original=row['original']))
    cases = []
    for original in selected:
        if compat.digest(compat.encoded(original)) != ORIGINAL_CONTRACTS[original['id']]:
            raise ValueError('pinned original t40 contract changed')
        source = original['source']
        if (original['suite'] != 'selected-historical-regression'
                or original['cwd'] != '.' or original['environment'] != {}
                or original['command'] != ['eval', '--settings', 'pkl:settings', '--color=never',
                                           '--no-project', source['path']]
                or original['channels'] != ['stdout', 'stderr', 'exit_code', 'files']
                or original['host_requirements'] != ['filesystem']):
            raise ValueError('original t40 argv/cwd/env/channels/Host contract differs')
        if oracle.sha256(source_file(root, source['path'])) != source['sha256']:
            raise ValueError('original t40 input differs')
        cases.append(dict(id='m2:'+original['id'], input=source['path'], input_sha256=source['sha256'],
                          contract=original, contract_sha256=compat.digest(compat.encoded(original))))
    return dict(schema_version=1, adapter=ADAPTER, profile=PROFILE,
                source_registry_sha256=oracle.sha256(root/'compatibility/regressions.json'),
                scope='Exactly two unchanged original t40 CLI contracts. T36/project remains unimplemented; historical observations are provenance, not expected output.',
                watchdog_seconds=m2_module_eval.CAPTURE_TIMEOUT_SECONDS,
                resources=resources, cases=cases)


def registry_cases(root, registry=None):
    if registry is None:
        registry = json.loads((root/REGISTRY).read_bytes())
    if registry != build_registry(root):
        raise ValueError('selected original registry/profile/resource closure differs')
    return {case['id']: case for case in registry['cases']}


def manifest_case(original, entry=None):
    case = dict(id='m2:'+original['id'], required=True, adapter=None,
                input=original['source']['path'], input_sha256=original['source']['sha256'],
                requirements=[], execution=dict(state='unimplemented', reason=
                'Selected original regression retained; M2 full module/CLI adapter is unimplemented.'),
                regression_case=original)
    if entry is not None:
        case.update(adapter=ADAPTER, upstream_case=original,
                    selected_regression_profile=dict(profile=PROFILE, registry=REGISTRY.as_posix(),
                                                     contract_sha256=entry['contract_sha256']),
                    execution=dict(state='ready', reason=None))
    return case


def validate_manifest(root, manifest):
    entries = registry_cases(root)
    originals = json.loads((root/'compatibility/regressions.json').read_bytes())['cases']
    by_id = {case['id']: case for case in manifest['cases']}
    for original in originals:
        identifier = 'm2:'+original['id']
        if identifier not in by_id:
            raise ValueError('retained original regression missing')
        expected = manifest_case(original, entries.get(identifier))
        expected['requirements'] = by_id[identifier]['requirements']
        if by_id[identifier] != expected:
            raise ValueError('retained original execution contract changed: '+identifier)
    if any(case.get('adapter') == ADAPTER and case['id'] not in entries
           for case in manifest['cases']):
        raise ValueError('selected original adapter assigned outside its two IDs')


def resource_baseline(registry):
    result = {}
    for row in registry['resources']:
        name = relative(row['path'])
        result[name.as_posix()] = dict(kind='file', sha256=row['sha256'])
        for parent in name.parents:
            if parent != Path('.'):
                result[parent.as_posix()] = dict(kind='directory', sha256=None)
    return dict(sorted(result.items()))


def prepare_workspace(root, workspace, output, registry):
    compat.validate_paths(root, workspace, output)
    if workspace.exists() or workspace.is_symlink():
        raise ValueError('selected original workspace must be fresh')
    workspace.mkdir(parents=True)
    compat.write_json(workspace/compat.MARKER, dict(schema_version=1, repository=str(root.resolve()),
        workspace=str(workspace.resolve()), selected_regression_profile=PROFILE,
        registry_sha256=oracle.sha256(root/REGISTRY)))
    for row in registry['resources']:
        name = relative(row['path'])
        destination = workspace/name
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes((root/name).read_bytes())
        destination.chmod(0o644)
    baseline = compat.snapshot(workspace)
    if baseline != resource_baseline(registry):
        raise ValueError('selected original minimal workspace closure differs')
    return baseline


def acquire(root, output, cases, metadata, binary, workspace=None):
    registry = json.loads((root/REGISTRY).read_bytes())
    entries = registry_cases(root, registry)
    if [case['id'] for case in cases] != list(entries):
        raise ValueError('selected original acquisition must retain both IDs in order')
    workspace = workspace or output.with_name(output.name+'-selected-regression-workspace')
    baseline = prepare_workspace(root, workspace, output, registry)
    compat.write_json(output/'selected-regression-workspace.json', baseline)
    compat.write_json(output/'selected-regression-registry.json', registry)
    results = []
    for case in cases:
        if case['upstream_case'] != entries[case['id']]['contract'] or compat.snapshot(workspace) != baseline:
            raise ValueError('selected original case/workspace changed before acquisition')
        row = m2_module_eval.acquire_case(root, workspace, case, metadata, binary, output)
        row['adapter'] = ADAPTER
        results.append(row)
    return results, dict(profile=PROFILE, workspace=str(workspace), workers=1,
        watchdog_seconds=m2_module_eval.CAPTURE_TIMEOUT_SECONDS,
        registry_sha256=oracle.sha256(output/'selected-regression-registry.json'),
        baseline_sha256=oracle.sha256(output/'selected-regression-workspace.json'),
        original_case_ids=list(entries), historical_results_reused=False)


def verify_context(root, output, context):
    registry = json.loads((output/'selected-regression-registry.json').read_bytes())
    entries = registry_cases(root, registry)
    if (context.get('profile') != PROFILE or context.get('workers') != 1
            or context.get('watchdog_seconds') != m2_module_eval.CAPTURE_TIMEOUT_SECONDS
            or context.get('original_case_ids') != list(entries)
            or context.get('historical_results_reused') is not False
            or context.get('registry_sha256') != oracle.sha256(output/'selected-regression-registry.json')
            or context.get('baseline_sha256') != oracle.sha256(output/'selected-regression-workspace.json')):
        raise ValueError('selected original context/profile changed')
    baseline = json.loads((output/'selected-regression-workspace.json').read_bytes())
    if baseline != resource_baseline(registry):
        raise ValueError('selected original resource baseline changed')
    return baseline


def verify_case(root, output, context, case, row, metadata, binary, baseline):
    if case.get('adapter') != ADAPTER or row.get('adapter') != ADAPTER:
        raise ValueError('selected original adapter identity changed')
    m2_module_eval.verify_case(root, Path(context['workspace']), case,
        dict(row, adapter=m2_module_eval.ADAPTER), output, metadata, binary, baseline)


def source_paths(root):
    """Bounded compiler/tool/metadata and exact seven-resource snapshot for these two cases."""
    import m2_source_inputs
    paths = set(m2_source_inputs.INPUTS) | {m2_source_inputs.MANIFEST, 'oracle.lock.json',
        'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo/config.toml', 'xtask/Cargo.toml',
        'tools/m2_source_inputs.py', 'tools/m2_manifest.py', 'tools/m2_selected_regressions.py',
        'tools/m2_language.py', 'tools/m2_module_eval.py', 'tools/m2_historical.py',
        'tools/tests/test_m2_selected_regressions.py',
        'tools/m2_jvm_classpath.py', 'tools/m2_execution_profiles.py', 'tools/m2_workspace.py',
        'tools/m2_generate.py', 'tools/compat.py', 'tools/oracle.py', 'tools/vendor_sources.py'}
    paths.add('crates/pkl-runtime/src/stdlib/modules.txt')
    registry_cases(root)
    paths.update(row['path'] for row in json.loads((root/REGISTRY).read_bytes())['resources'])
    def visit(folder):
        try:
            info = folder.lstat()
        except FileNotFoundError:
            return
        if not stat.S_ISDIR(info.st_mode):
            raise ValueError('selected source root is not a real directory')
        for entry in os.scandir(folder):
            path = Path(entry.path)
            name = path.relative_to(root).as_posix()
            if entry.name in ('target', '.git', 'node_modules', '.cache', '__pycache__'):
                continue
            relative(name)  # reject forbidden/archive paths before metadata
            info = entry.stat(follow_symlinks=False)
            if stat.S_ISLNK(info.st_mode):
                raise ValueError('selected source snapshot cannot follow symlinks')
            if stat.S_ISDIR(info.st_mode):
                visit(path)
            elif stat.S_ISREG(info.st_mode) and (path.suffix in ('.rs', '.pkl', '.bin', '.toml')
                                                 or path.name in ('LICENSE', 'NOTICE')):
                paths.add(name)
    for folder in ('crates', 'vendor'):
        visit(root/folder)
    return sorted(relative(name).as_posix() for name in paths)

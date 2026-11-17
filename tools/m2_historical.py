"""Explicit admission of eight pinned historical modules; no historical pass reuse."""
from pathlib import Path
import json

import compat
import m2_module_eval
import oracle

ADAPTER = 'historical-collections-module-eval'
REGISTRY = Path('compatibility/m2-historical-collections.json')
FIXTURES = Path('tests/fixtures/rev1-collections')
REVISION = '008930df07f457fefd5d6f4ac9308ff3063daabc'
PROFILE = 'pkl-0.32-historical-collections-no-cache-no-project-v1'
ORIGINAL = 'crates/pkl-core/src/tests/bytecode_tests/'
SELECTED = (
    'collection_default_observation/listingDefault.pkl',
    'collection_default_observation/typedListingAmend.pkl',
    'collection_default_observation/mappingDefault.pkl',
    'collection_default_observation/lazyObservation.pkl',
    'nested_typed_collections/nestedTypedCollections.pkl',
    'nested_typed_collections/nestedTypedCollectionAmends.pkl',
    'nested_typed_collections/nestedTypedClassProperties.pkl',
    'nested_typed_collections/nestedTypedValidation.pkl',
)
RESOURCE_ONLY = 'nested_typed_collections/nestedLib.pkl'

# Comprehensive mapping of all 8 non-executable support library resources
RESOURCE_ONLY_MAP = {
    'crates/pkl-core/src/tests/bytecode_tests/nested_typed_collections/nestedLib.pkl': 'tests/fixtures/rev1-collections/nested_typed_collections/nestedLib.pkl',
    'crates/pkl-cli/tests/fixtures/t43_043_inherited_method_lib.pkl': 'tests/fixtures/rev1-cli/t43_043_inherited_method_lib.pkl',
    'crates/pkl-cli/tests/fixtures/t43_064_imported_function_lib.pkl': 'tests/fixtures/rev1-cli/t43_064_imported_function_lib.pkl',
    'crates/pkl-core/tests/fixtures/class_authority/t40/repro_lib.pkl': 'tests/fixtures/rev1/class_authority/t40/repro_lib.pkl',
    'crates/pkl-core/tests/fixtures/class_authority/t40/foreign_namesake/repro_lib.pkl': 'tests/fixtures/rev1/class_authority/t40/foreign_namesake/repro_lib.pkl',
    'crates/pkl-core/tests/fixtures/class_authority/t40/foreign_namesake/foreign.pkl': 'tests/fixtures/rev1/class_authority/t40/foreign_namesake/foreign.pkl',
    'crates/pkl-core/tests/fixtures/diagnostics/t36/foreign.pkl': 'tests/fixtures/rev1/diagnostics/t36/app/foreign.pkl',
    'crates/pkl-core/tests/fixtures/diagnostics/t36/form.pkl': 'tests/fixtures/rev1/diagnostics/t36/dep/form.pkl',
}
ALL_RESOURCE_ONLY = tuple(RESOURCE_ONLY_MAP.keys())

# The 3 existing tracked regressions in regressions.json
TRACKED_REGRESSIONS = {
    'crates/pkl-core/tests/fixtures/class_authority/t40/repro.pkl': 'm2:rev1:t40-imported-declaration-authority',
    'crates/pkl-core/tests/fixtures/class_authority/t40/foreign_namesake/repro.pkl': 'm2:rev1:t40-foreign-namesake',
    'crates/pkl-core/tests/fixtures/diagnostics/t36/negative.pkl': 'm2:rev1:t36-foreign-nominal-identity',
}

# The 21 standalone historical CLI regression candidates
CLI_ADAPTER = 'historical-cli-module-eval'
CLI_FIXTURES = Path('tests/fixtures/rev1-cli')
CLI_PROFILE = 'pkl-0.32-historical-cli-no-cache-no-project-v1'
CLI_ORIGINAL = 'crates/pkl-cli/tests/fixtures/'
CLI_SELECTED = (
    'bytecode_compound_expression_frames/main.pkl',
    'nested_function_render_span/deeper.pkl',
    'nested_function_render_span/direct.pkl',
    'nested_function_render_span/main.pkl',
    'nested_function_render_span/multiline.pkl',
    'reference_function_render_frame/precedence.pkl',
    'reference_function_render_frame/simple.pkl',
    't43_013_constraint_frame.pkl',
    't43_013_default_frame.pkl',
    't43_043_inherited_method_main.pkl',
    't43_064_imported_function_main.pkl',
    't43_103_spread_ordinal.pkl',
    'textproto_dynamic_default/dynamic_first.pkl',
    'textproto_dynamic_default/expression.pkl',
    'textproto_dynamic_default/function.pkl',
    'textproto_dynamic_default/nested.pkl',
    'textproto_dynamic_default/nested_function_first.pkl',
    'textproto_dynamic_default/nested_throw_first.pkl',
    'textproto_dynamic_default/precedence.pkl',
    'textproto_dynamic_default/scalar.pkl',
    'textproto_dynamic_default/two_functions.pkl',
)


def build_registry(root):
    inventory = json.loads((root/'compatibility/rev1-inventory.json').read_bytes())
    if inventory['repository'] != 'rev1' or inventory['revision'] != REVISION:
        raise ValueError('historical inventory revision differs')
    source = {x['path']:x for x in inventory['files'] + inventory['licenses']}
    resources = []
    for local in (*SELECTED, RESOURCE_ONLY, 'LICENSE', 'NOTICE'):
        original = local if local in ('LICENSE','NOTICE') else ORIGINAL + local
        row = source[original]
        path = FIXTURES/local
        raw = (root/path).read_bytes()
        if oracle.sha256(root/path) != row['sha256'] or ('bytes' in row and len(raw) != row['bytes']):
            raise ValueError('historical fixture missing or changed: '+str(path))
        resources.append({'path':path.as_posix(),'sha256':row['sha256'],'kind':'file',
            'bytes':len(raw),'original':{'repository':'rev1','revision':REVISION,'path':original,'sha256':row['sha256']}})
    actual = {p.relative_to(root).as_posix() for p in (root/FIXTURES).rglob('*') if p.is_file()}
    if actual != {x['path'] for x in resources} or any(p.is_symlink() for p in (root/FIXTURES).rglob('*')):
        raise ValueError('historical fixture closure has missing/extra/symlink payloads')
    cases = []
    selected_paths = {ORIGINAL+local for local in SELECTED}
    selected_order = [row['path'].removeprefix(ORIGINAL) for row in inventory['files'] if row['path'] in selected_paths]
    for local in selected_order:
        original = ORIGINAL+local
        if source[original]['kind'] != 'regression-candidate':
            raise ValueError('historical input is not an original candidate')
        identifier = 'rev1-candidate:'+original
        path = (FIXTURES/local).as_posix()
        contract = {'id':identifier,'suite':'historical-collections','source':{'repository':'pkl-rs','path':path,'sha256':source[original]['sha256']},
            'command':['eval','--no-cache','--no-project','--settings','pkl:settings',path],
            'cwd':'.','environment':{},'output':None,'expected_exit':[0],
            'resources':[{'path':r['path'],'sha256':r['sha256']} for r in resources],
            'channels':['stdout','stderr','exit_code','files','workspace_changes'],
            'execution':{'status':'ready','reason':None},
            'contract_origin':'Explicit new pinned Java0.32 historical profile; not the three selected regression CLI contracts and not a rev1 Rust expectation.'}
        cases.append({'id':identifier,'input':path,'input_sha256':source[original]['sha256'],
            'contract':contract,'contract_sha256':compat.digest(compat.encoded(contract))})
    return {'schema_version':1,'adapter':ADAPTER,'profile':PROFILE,
        'baseline':{'version':'0.32.0','revision':'e51fb2f91774265666f94980ec630db794a4db2d'},
        'historical_revision':REVISION,'snapshot_sha256':oracle.sha256(root/'compatibility/rev1-inventory.json'),
        'scope':'Exactly eight original candidate IDs become executable. Registration is readiness to attempt, never semantic pass or requirement completion. Other32 historical candidates and all three selected CLI contracts remain unchanged.',
        'watchdog_seconds':m2_module_eval.CAPTURE_TIMEOUT_SECONDS,
        'artifact_policy':'Common module_eval_probe binding, with explicit recorded-build or honest caller-supplied provenance. Reused Stage90 artifact source authority is recorded separately; no fresh compilation is inferred.',
        'prior_authority':{'stage90':'Same source modules were observed twice under a distinct exploration contract; stdout/stderr/exit only. No generated-files/unexpected-write verification is inferred. Fresh four-channel acquisition required.',
            'rev1':'Pinned source/harness selection context only; historical Rust passes are not expected values.'},
        'resources':resources,'cases':cases}


def registry_cases(root, registry=None):
    if registry is None:
        registry = json.loads((root/REGISTRY).read_bytes())
    expected = build_registry(root)
    if registry != expected:
        raise ValueError('historical registry coverage/order/source/CLI contract differs')
    rows = {x['id']:x for x in registry['cases']}
    if len(rows) != len(SELECTED):
        raise ValueError('duplicate historical ID')
    return rows


def manifest_case(row, entry=None, cli_entry=None):
    case = {'id': 'rev1-candidate:' + row['path'], 'required': True, 'adapter': None, 'requirements': [],
        'source': dict(repository='rev1', revision=REVISION, **row),
        'execution': {'state': 'unmapped', 'reason': 'Historical candidate awaits semantic selection/import and fresh Java execution; historical passes are not evidence.'}}
    if entry is not None:
        case.update(adapter=ADAPTER, input=entry['input'], input_sha256=entry['input_sha256'],
            upstream_case=entry['contract'], historical_execution_profile={'profile': PROFILE, 'contract_sha256': entry['contract_sha256'],
                'historical_revision': REVISION, 'scope': 'New explicit historical contract; old selected regression commands unchanged.'},
            execution={'state': 'ready', 'reason': None})
    elif cli_entry is not None:
        case.update(adapter=CLI_ADAPTER, input=cli_entry['input'], input_sha256=cli_entry['input_sha256'],
            upstream_case=cli_entry['contract'], historical_execution_profile={'profile': CLI_PROFILE, 'contract_sha256': cli_entry['contract_sha256'],
                'historical_revision': REVISION, 'scope': 'Twenty-one historical CLI candidates admitted with differential comparison against Java 0.32.0 Oracle.'},
            execution={'state': 'ready', 'reason': None})
    elif row['path'] in RESOURCE_ONLY_MAP:
        case['required'] = False
        case['execution'] = {'state': 'unmapped', 'reason': 'Non-executable support library resource imported by historical or regression modules; not a standalone evaluation target.'}
    elif row['path'] in TRACKED_REGRESSIONS:
        case['required'] = False
        case['execution'] = {'state': 'unmapped', 'reason': f'Tracked regression candidate admitted under {TRACKED_REGRESSIONS[row["path"]]}.'}
    return case


def validate_manifest(root, manifest):
    registered = registry_cases(root)
    cli_registered = cli_registry_cases(root)
    inventory = json.loads((root / 'compatibility/rev1-inventory.json').read_bytes())
    originals = {('rev1-candidate:' + r['path']): r for r in inventory['files'] if r['kind'] == 'regression-candidate'}
    cases = {r['id']: r for r in manifest['cases'] if r['id'].startswith('rev1-candidate:')}
    if set(cases) != set(originals):
        raise ValueError('historical inventory IDs missing/added')
    for identifier, row in cases.items():
        expected = manifest_case(originals[identifier], registered.get(identifier), cli_registered.get(identifier))
        # Reviewed requirement mappings may be additive; execution/profile/source cannot change.
        expected['requirements'] = row['requirements']
        if row != expected:
            raise ValueError('historical selected/unselected execution contract differs: ' + identifier)


def resource_baseline(registry):
    baseline = {}
    for resource in registry['resources']:
        baseline[resource['path']] = {'kind':'file','sha256':resource['sha256']}
        for parent in Path(resource['path']).parents:
            if parent != Path('.'):
                baseline[parent.as_posix()] = {'kind':'directory','sha256':None}
    return dict(sorted(baseline.items()))


def prepare_workspace(root, workspace, output, registry):
    compat.validate_paths(root, workspace, output)
    if workspace.exists():
        raise ValueError('historical acquisition requires fresh workspace')
    workspace.mkdir(parents=True)
    marker = {'schema_version':1,'repository':str(root.resolve()),'workspace':str(workspace.resolve()),
        'historical_profile':PROFILE,'registry_sha256':oracle.sha256(root/REGISTRY)}
    compat.write_json(workspace/compat.MARKER, marker)
    for resource in registry['resources']:
        source, destination = root/resource['path'], workspace/resource['path']
        destination.parent.mkdir(parents=True,exist_ok=True)
        destination.write_bytes(source.read_bytes())
        destination.chmod(0o644)
    baseline = compat.snapshot(workspace)
    if baseline != resource_baseline(registry):
        raise ValueError('historical minimal workspace closure differs')
    return baseline


def acquire(root, output, cases, metadata, binary, workspace=None):
    registry = json.loads((root/REGISTRY).read_bytes())
    registered = registry_cases(root,registry)
    expected_ids = [x['id'] for x in registry['cases']]
    if [x['id'] for x in cases] != expected_ids:
        raise ValueError('historical acquisition must retain all eight original cases in registry order')
    workspace = workspace or output.with_name(output.name+'-historical-workspace')
    baseline = prepare_workspace(root,workspace,output,registry)
    compat.write_json(output/'historical-workspace.json',baseline)
    compat.write_json(output/'historical-registry.json',registry)
    results = []
    for case in cases:
        if case['upstream_case'] != registered[case['id']]['contract'] or compat.snapshot(workspace) != baseline:
            raise ValueError('historical case or workspace changed before acquisition')
        result = m2_module_eval.acquire_case(root,workspace,case,metadata,binary,output)
        result['adapter'] = ADAPTER
        results.append(result)
    context = {'profile':PROFILE,'workspace':str(workspace),'workers':1,
        'watchdog_seconds':m2_module_eval.CAPTURE_TIMEOUT_SECONDS,
        'registry_sha256':oracle.sha256(output/'historical-registry.json'),
        'baseline_sha256':oracle.sha256(output/'historical-workspace.json'),
        'files_boundary':'Complete generated public files and unexpected workspace changes; private home/tmp retained separately, not silently treated as public output.',
        'prior_Stage90_three_channel_results_reused':False}
    return results, context


def verify_context(root, output, context):
    registry = json.loads((output/'historical-registry.json').read_bytes())
    registry_cases(root,registry)
    if (not isinstance(context,dict) or context.get('profile')!=PROFILE or context.get('workers')!=1
            or context.get('watchdog_seconds')!=m2_module_eval.CAPTURE_TIMEOUT_SECONDS
            or context.get('registry_sha256')!=oracle.sha256(output/'historical-registry.json')
            or context.get('baseline_sha256')!=oracle.sha256(output/'historical-workspace.json')
            or context.get('prior_Stage90_three_channel_results_reused') is not False):
        raise ValueError('historical context/profile/registry changed')
    baseline = json.loads((output/'historical-workspace.json').read_bytes())
    if baseline != resource_baseline(registry):
        raise ValueError('historical baseline resource closure changed')
    return baseline


def verify_case(root, output, context, case, row, metadata, binary, baseline):
    if row.get('adapter')!=ADAPTER or case.get('adapter')!=ADAPTER:
        raise ValueError('historical adapter identity differs')
    # Reuse the original four-channel verifier without changing its upstream-only registry.
    wrapped = dict(row,adapter=m2_module_eval.ADAPTER)
    m2_module_eval.verify_case(root,Path(context['workspace']),case,wrapped,output,metadata,binary,baseline)


def build_cli_registry(root):
    inventory = json.loads((root / 'compatibility/rev1-inventory.json').read_bytes())
    if inventory['repository'] != 'rev1' or inventory['revision'] != REVISION:
        raise ValueError('historical inventory revision differs')
    source = {x['path']: x for x in inventory['files'] + inventory['licenses']}
    resources = []
    cli_resource_files = (*CLI_SELECTED, 't43_043_inherited_method_lib.pkl', 't43_064_imported_function_lib.pkl', 'LICENSE', 'NOTICE')
    for local in cli_resource_files:
        original = local if local in ('LICENSE', 'NOTICE') else CLI_ORIGINAL + local
        row = source[original]
        path = CLI_FIXTURES / local
        raw = (root / path).read_bytes()
        if oracle.sha256(root / path) != row['sha256'] or ('bytes' in row and len(raw) != row['bytes']):
            raise ValueError('historical CLI fixture missing or changed: ' + str(path))
        resources.append({'path': path.as_posix(), 'sha256': row['sha256'], 'kind': 'file',
            'bytes': len(raw), 'original': {'repository': 'rev1', 'revision': REVISION, 'path': original, 'sha256': row['sha256']}})
    actual = {p.relative_to(root).as_posix() for p in (root / CLI_FIXTURES).rglob('*') if p.is_file()}
    if actual != {x['path'] for x in resources} or any(p.is_symlink() for p in (root / CLI_FIXTURES).rglob('*')):
        raise ValueError('historical CLI fixture closure has missing/extra/symlink payloads')
    cases = []
    for local in CLI_SELECTED:
        original = CLI_ORIGINAL + local
        if source[original]['kind'] != 'regression-candidate':
            raise ValueError('historical input is not an original candidate')
        identifier = 'rev1-candidate:' + original
        path = (CLI_FIXTURES / local).as_posix()
        contract = {'id': identifier, 'suite': 'historical-cli', 'source': {'repository': 'pkl-rs', 'path': path, 'sha256': source[original]['sha256']},
            'command': ['eval', '--no-cache', '--no-project', '--settings', 'pkl:settings', path],
            'cwd': '.', 'environment': {}, 'output': None, 'expected_exit': [0, 1],
            'resources': [{'path': r['path'], 'sha256': r['sha256']} for r in resources],
            'channels': ['stdout', 'stderr', 'exit_code', 'files', 'workspace_changes'],
            'execution': {'status': 'ready', 'reason': None},
            'contract_origin': 'Explicit new pinned Java 0.32 historical CLI profile.'}
        cases.append({'id': identifier, 'input': path, 'input_sha256': source[original]['sha256'],
            'contract': contract, 'contract_sha256': compat.digest(compat.encoded(contract))})
    return {'schema_version': 1, 'adapter': CLI_ADAPTER, 'profile': CLI_PROFILE,
        'baseline': {'version': '0.32.0', 'revision': 'e51fb2f91774265666f94980ec630db794a4db2d'},
        'historical_revision': REVISION, 'snapshot_sha256': oracle.sha256(root / 'compatibility/rev1-inventory.json'),
        'scope': 'Twenty-one historical CLI candidates admitted with differential comparison against Java 0.32.0 Oracle.',
        'watchdog_seconds': m2_module_eval.CAPTURE_TIMEOUT_SECONDS,
        'resources': resources, 'cases': cases}


def cli_registry_cases(root):
    registry = build_cli_registry(root)
    return {x['id']: x for x in registry['cases']}


def prepare_cli_workspace(root, workspace, output, registry):
    compat.validate_paths(root, workspace, output)
    if workspace.exists():
        raise ValueError('historical CLI acquisition requires fresh workspace')
    workspace.mkdir(parents=True)
    marker = {'schema_version': 1, 'repository': str(root.resolve()), 'workspace': str(workspace.resolve()),
        'historical_profile': CLI_PROFILE}
    compat.write_json(workspace / compat.MARKER, marker)
    for resource in registry['resources']:
        source, destination = root / resource['path'], workspace / resource['path']
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(source.read_bytes())
        destination.chmod(0o644)
    baseline = compat.snapshot(workspace)
    if baseline != resource_baseline(registry):
        raise ValueError('historical CLI minimal workspace closure differs')
    return baseline


def acquire_cli(root, output, cases, metadata, binary, workspace=None):
    registry = build_cli_registry(root)
    registered = cli_registry_cases(root)
    expected_ids = [x['id'] for x in registry['cases']]
    if [x['id'] for x in cases] != expected_ids:
        raise ValueError('historical CLI acquisition must retain all 21 cases in registry order')
    workspace = workspace or output.with_name(output.name + '-historical-cli-workspace')
    baseline = prepare_cli_workspace(root, workspace, output, registry)
    compat.write_json(output / 'historical-cli-workspace.json', baseline)
    compat.write_json(output / 'historical-cli-registry.json', registry)
    results = []
    for case in cases:
        if case['upstream_case'] != registered[case['id']]['contract'] or compat.snapshot(workspace) != baseline:
            raise ValueError('historical CLI case or workspace changed before acquisition')
        result = m2_module_eval.acquire_case(root, workspace, case, metadata, binary, output)
        result['adapter'] = CLI_ADAPTER
        results.append(result)
    context = {'profile': CLI_PROFILE, 'workspace': str(workspace), 'workers': 1,
        'watchdog_seconds': m2_module_eval.CAPTURE_TIMEOUT_SECONDS,
        'files_boundary': 'Complete generated public files and unexpected workspace changes.',
        'prior_Stage90_three_channel_results_reused': False}
    return results, context


def verify_cli_context(root, output, context):
    registry = build_cli_registry(root)
    if (not isinstance(context, dict) or context.get('profile') != CLI_PROFILE or context.get('workers') != 1
            or context.get('watchdog_seconds') != m2_module_eval.CAPTURE_TIMEOUT_SECONDS):
        raise ValueError('historical CLI context/profile changed')
    baseline = json.loads((output / 'historical-cli-workspace.json').read_bytes())
    if baseline != resource_baseline(registry):
        raise ValueError('historical CLI baseline resource closure changed')
    return baseline


def verify_cli_case(root, output, context, case, row, metadata, binary, baseline):
    if row.get('adapter') != CLI_ADAPTER or case.get('adapter') != CLI_ADAPTER:
        raise ValueError('historical CLI adapter identity differs')
    wrapped = dict(row, adapter=m2_module_eval.ADAPTER)
    m2_module_eval.verify_case(root, Path(context['workspace']), case, wrapped, output, metadata, binary, baseline)


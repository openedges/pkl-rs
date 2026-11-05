"""Explicit staging boundaries for original upstream module experiments."""
from pathlib import Path
import re

import compat

DEFAULT_PROFILE = 'legacy-full'
ORIGINAL_PROFILE = 'upstream-original-modules-v1'
PROFILES = (DEFAULT_PROFILE, ORIGINAL_PROFILE)
SOURCE_ROOTS = {'pkl': 'external/pkl', 'pkl-pantry': 'external/pkl-pantry'}


def _path(relative):
    if not isinstance(relative, str):
        raise ValueError('workspace resource path must be a string')
    path = Path(relative)
    if path.is_absolute() or '..' in path.parts or path.as_posix() != relative or relative == '.':
        raise ValueError('invalid workspace resource path: ' + relative)
    return path


def _sources(corpus):
    sources = corpus['sources']
    if len(sources) != len(SOURCE_ROOTS) or {source['repository'] for source in sources} != SOURCE_ROOTS.keys():
        raise ValueError('workspace profile requires complete pkl and pantry source inventories')
    result, files = [], {}
    for repository, relative in SOURCE_ROOTS.items():
        source = next(source for source in sources if source['repository'] == repository)
        if source['path'] != relative or not isinstance(source['revision'], str) or not source['revision']:
            raise ValueError('workspace retained source identity differs')
        resources = source['resources']
        if not resources or compat.digest(compat.encoded(resources)) != source['resource_set_sha256']:
            raise ValueError('workspace source inventory digest differs')
        root = Path(relative)
        for resource in resources:
            path = _path(resource['path'])
            if (not path.is_relative_to(root) or path == root or resource['path'] in files
                    or resource['kind'] not in ('file', 'symlink')
                    or not isinstance(resource['sha256'], str)
                    or not re.fullmatch('[0-9a-f]{64}', resource['sha256'])):
                raise ValueError('invalid retained workspace resource: ' + resource['path'])
            files[resource['path']] = dict(kind=resource['kind'], sha256=resource['sha256'])
        result.append(dict(repository=repository, path=relative, revision=source['revision'],
                           resource_set_sha256=source['resource_set_sha256']))
    return result, files


def _record(profile, corpus):
    if profile not in PROFILES:
        raise ValueError('unsupported module workspace profile: ' + str(profile))
    sources, files = _sources(corpus)
    return dict(schema_version=1, profile=profile, retained_sources=sources,
                project_fixtures='include' if profile == DEFAULT_PROFILE else 'exclude'), files


def original_source_profile(corpus):
    """Describe the retained filesystem without classifying any case as ready.

    Callers must separately validate their original input and execution profile.
    This record excludes the project's unrelated local fixture tree.
    """
    return _record(ORIGINAL_PROFILE, corpus)[0]


def profile_record(profile, corpus, cases):
    """Bind an explicit staging profile to inventories and selected case contracts.

    Inventory closure is static evidence; it does not establish equivalence with
    executions that also expose the project's unrelated local fixture tree.
    """
    record, files = _record(profile, corpus)
    if profile == ORIGINAL_PROFILE:
        if not cases:
            raise ValueError('original module workspace profile requires original cases')
        originals = {case['id']: case for case in corpus['cases']}
        identifiers = set()
        for case in cases:
            original = case.get('upstream_case')
            if (case.get('adapter') != 'language-module-eval' or not isinstance(original, dict)
                    or case['id'] in identifiers or originals.get(case['id']) != original
                    or original.get('suite') != 'language-snippet'
                    or original.get('execution', {}).get('status') != 'ready'
                    or case.get('execution', {}).get('state') != 'ready'):
                raise ValueError('original module workspace profile requires unchanged original-ready module cases')
            identifiers.add(case['id'])
            source = original['source']
            expected_input = SOURCE_ROOTS.get(source['repository'], '') + '/' + source['path']
            if case.get('input') != expected_input or case.get('input_sha256') != source['sha256']:
                raise ValueError('workspace original input contract differs')
            for resource in [dict(path=case['input'], kind='file', sha256=case['input_sha256']),
                             *original['resources']]:
                path = _path(resource['path'])
                identity = files.get(path.as_posix())
                if (identity is None or identity['sha256'] != resource['sha256']
                        or ('kind' in resource and identity['kind'] != resource['kind'])):
                    raise ValueError('original module input/resource is outside the complete retained inventory: ' + resource['path'])
            cwd = original['cwd']
            if cwd != '.':
                cwd_path = _path(cwd)
                if not any(cwd_path == Path(root) or cwd_path.is_relative_to(Path(root))
                           for root in SOURCE_ROOTS.values()):
                    raise ValueError('original module cwd is outside retained source trees')
    return record


def workspace_identity(root, workspace, record):
    if (not isinstance(record, dict) or record.get('profile') not in PROFILES
            or type(record.get('schema_version')) is not int or record['schema_version'] != 1):
        raise ValueError('invalid module workspace profile record')
    if (set(record) != {'schema_version', 'profile', 'retained_sources', 'project_fixtures'}
            or record['project_fixtures'] != ('include' if record['profile'] == DEFAULT_PROFILE else 'exclude')
            or not isinstance(record['retained_sources'], list)
            or len(record['retained_sources']) != len(SOURCE_ROOTS)):
        raise ValueError('invalid module workspace profile metadata')
    for source, (repository, relative) in zip(record['retained_sources'], SOURCE_ROOTS.items()):
        if (not isinstance(source, dict)
                or set(source) != {'repository', 'path', 'revision', 'resource_set_sha256'}
                or source['repository'] != repository or source['path'] != relative
                or not isinstance(source['revision'], str) or not source['revision']
                or not isinstance(source['resource_set_sha256'], str)
                or not re.fullmatch('[0-9a-f]{64}', source['resource_set_sha256'])):
            raise ValueError('invalid retained workspace source metadata')
    identity = dict(schema_version=1, repository=str(root.resolve()), workspace=str(workspace.resolve()))
    if record['profile'] != DEFAULT_PROFILE:
        identity['workspace_profile'] = record
    return identity


def prepare(root, workspace, record):
    identity = workspace_identity(root, workspace, record)
    compat.prepare_workspace(root, workspace,
                             include_project_fixtures=record['profile'] == DEFAULT_PROFILE,
                             workspace_profile=identity.get('workspace_profile'))


def validate_baseline(baseline, corpus, record):
    expected_record, files = _record(record.get('profile'), corpus)
    if record != expected_record:
        raise ValueError('module workspace profile differs from retained source inventories')
    expected = dict(files)
    for relative in files:
        for parent in Path(relative).parents:
            if parent == Path('.'):
                continue
            name = parent.as_posix()
            if name in files:
                raise ValueError('retained workspace file/symlink is also a resource ancestor')
            expected[name] = dict(kind='directory', sha256=None)
    if record['profile'] == ORIGINAL_PROFILE:
        if baseline != expected:
            raise ValueError('original module workspace baseline differs from complete retained source trees')
    elif any(baseline.get(name) != identity for name, identity in expected.items()):
        raise ValueError('legacy module workspace retained source baseline differs')

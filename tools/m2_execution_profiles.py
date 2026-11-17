"""Reviewed original-case execution profiles; historical CLI contracts stay intact."""
import json
from pathlib import Path

import compat
import m2_jvm_classpath as jvm
import m2_module_eval as mod_eval
import m2_package_project_eval as pkg_proj
from m2_source_inputs import EXECUTION_PROFILES

LIST_CASE_ID = 'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/api/list.pkl'
LIST_PROFILE = 'compatibility/experiments/list-large-heap.json'


def expected_registry(root):
    root = Path(root).resolve()
    profile = jvm.load_profile(root)
    jvm_case = dict(
        adapter=jvm.ADAPTER,
        id=profile['original_case_id'],
        input_sha256=profile['input']['sha256'],
        profile=jvm.PROFILE,
        profile_sha256=compat.digest((root / jvm.PROFILE).read_bytes()),
        scope='Reviewed JVM API/classpath setup for the unchanged original success input; '
              'the historical corpus CLI command/blocked record remains provenance, not the execution boundary.',
        upstream_case_sha256=profile['original_case_sha256'],
    )
    corpus = json.loads((root / 'compatibility/corpus.json').read_bytes())
    list_original = next(c for c in corpus['cases'] if c['id'] == LIST_CASE_ID)
    list_case = dict(
        adapter=mod_eval.ADAPTER,
        id=LIST_CASE_ID,
        input_sha256=list_original['source']['sha256'],
        profile=LIST_PROFILE,
        profile_sha256=compat.digest((root / LIST_PROFILE).read_bytes()),
        scope='Reviewed JVM 2 GiB heap allocation profile for large Paguro list evaluation; original 512 MiB CLI record remains provenance.',
        upstream_case_sha256=compat.digest(compat.encoded(list_original)),
    )
    cases = [jvm_case, list_case] + pkg_proj.profile_cases(root)
    corpus_order = {c['id']: i for i, c in enumerate(corpus['cases'])}
    cases.sort(key=lambda c: corpus_order.get(c['id'], 0))
    return dict(schema=1, baseline=profile['baseline'], cases=cases)


def registry_cases(root, corpus):
    root = Path(root).resolve()
    registry = json.loads((root / EXECUTION_PROFILES).read_bytes())
    if (compat.encoded(registry) != compat.encoded(expected_registry(root))
            or registry['baseline'] != corpus['baseline']):
        raise ValueError('original execution-profile registry/binding differs')
    originals = {case['id']: case for case in corpus['cases']}
    for entry in registry['cases']:
        if (entry['id'] not in originals or compat.digest(compat.encoded(originals[entry['id']]))
                != entry['upstream_case_sha256']):
            raise ValueError('profile original case contract changed')
    return {entry['id']: entry for entry in registry['cases']}


def execution_state():
    return dict(state='ready', reason=None)

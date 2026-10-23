"""Reviewed original-case execution profiles; historical CLI contracts stay intact."""
import json
from pathlib import Path

import compat
import m2_jvm_classpath as jvm
from m2_source_inputs import EXECUTION_PROFILES


def expected_registry(root):
    profile = jvm.load_profile(root)
    return dict(schema=1, baseline=profile['baseline'], cases=[dict(
        id=profile['original_case_id'], adapter=jvm.ADAPTER, profile=jvm.PROFILE,
        profile_sha256=compat.digest((root / jvm.PROFILE).read_bytes()),
        upstream_case_sha256=profile['original_case_sha256'], input_sha256=profile['input']['sha256'],
        scope='Reviewed JVM API/classpath setup for the unchanged original success input; '
              'the historical corpus CLI command/blocked record remains provenance, not the execution boundary.')])


def registry_cases(root, corpus):
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

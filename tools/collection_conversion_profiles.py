"""Pinned collection conversion, construction, and reversal string profiles."""
from __future__ import annotations

import importlib
import json
from pathlib import Path
import hashlib

ROOT = Path(__file__).resolve().parents[1]
CATALOG = 'compatibility/collection-conversion-profiles.json'
INPUTS = ('cases.json', 'module.pkl', 'queries.tsv')
VALIDATORS = {
    'collection_conversions_fixtures': 'tools/collection_conversions_fixtures.py',
    'collection_conversion_controls_fixtures': 'tools/collection_conversion_controls_fixtures.py',
    'collection_lookups_fixtures': 'tools/collection_lookups_fixtures.py',
    'collection_default_contracts_fixtures': 'tools/collection_default_contracts_fixtures.py',
    'collection_dynamic_render_fixtures': 'tools/collection_dynamic_render_fixtures.py',
    'collection_property_names_fixtures': 'tools/collection_property_names_fixtures.py',
    'collection_addition_fixtures': 'tools/collection_addition_fixtures.py',
    'set_endpoints_fixtures': 'tools/set_endpoints_fixtures.py',
    'amendment_construction_fixtures': 'tools/amendment_construction_fixtures.py',
    'inferred_new_fixtures': 'tools/inferred_new_fixtures.py',
    'reversal_values_fixtures': 'tools/reversal_values_fixtures.py',
    'reversal_implicit_fixtures': 'tools/reversal_implicit_fixtures.py',
    'reversal_subscript_fixtures': 'tools/reversal_subscript_fixtures.py',
    'base_constructor_constraints_fixtures': 'tools/base_constructor_constraints_fixtures.py',
}
PROFILE_TOOL_CLOSURES = {
    'collection_conversions_fixtures': {
        'tools/collection_conversions_fixtures.py',
        'tools/collection_conversions_oracle.py',
        'tools/tests/test_collection_conversions_fixtures.py',
        'tools/java/CollectionConversionsOracle.java',
    },
    'collection_conversion_controls_fixtures': {
        'tools/collection_conversion_controls_fixtures.py',
        'tools/collection_conversion_controls_oracle.py',
        'tools/tests/test_collection_conversion_controls_fixtures.py',
        'tools/java/CollectionConversionsOracle.java',
    },
    'collection_lookups_fixtures': {
        'tools/collection_lookups_fixtures.py',
        'tools/collection_lookups_oracle.py',
        'tools/tests/test_collection_lookups_fixtures.py',
        'tools/java/CollectionConversionsOracle.java',
    },
    'collection_default_contracts_fixtures': {
        'tools/collection_default_contracts_fixtures.py',
        'tools/collection_default_contracts_oracle.py',
        'tools/tests/test_collection_default_contracts_fixtures.py',
        'tools/java/CollectionConversionsOracle.java',
    },
    'collection_dynamic_render_fixtures': {
        'tools/collection_dynamic_render_fixtures.py',
        'tools/collection_dynamic_render_oracle.py',
        'tools/tests/test_collection_dynamic_render_fixtures.py',
        'tools/java/CollectionConversionsOracle.java',
    },
    'collection_property_names_fixtures': {
        'tools/collection_property_names_fixtures.py',
        'tools/collection_property_names_oracle.py',
        'tools/tests/test_collection_property_names_fixtures.py',
        'tools/java/CollectionConversionsOracle.java',
    },
    'collection_addition_fixtures': {
        'tools/collection_addition_fixtures.py',
        'tools/collection_addition_oracle.py',
        'tools/tests/test_collection_addition_fixtures.py',
        'tools/java/CollectionConversionsOracle.java',
        'tools/collection_conversions_fixtures.py',
        'tools/collection_conversions_oracle.py',
    },
    'set_endpoints_fixtures': {
        'tools/set_endpoints_fixtures.py',
        'tools/set_endpoints_oracle.py',
        'tools/tests/test_set_endpoints_fixtures.py',
        'tools/java/CollectionConversionsOracle.java',
        'tools/collection_conversions_fixtures.py',
        'tools/collection_conversions_oracle.py',
    },
    'amendment_construction_fixtures': {
        'tools/amendment_construction_fixtures.py',
        'tools/amendment_construction_oracle.py',
        'tools/tests/test_amendment_construction_fixtures.py',
        'tools/java/CollectionConversionsOracle.java',
        'tools/collection_conversions_fixtures.py',
        'tools/collection_conversions_oracle.py',
    },
    'inferred_new_fixtures': {
        'tools/inferred_new_fixtures.py',
        'tools/inferred_new_oracle.py',
        'tools/amendment_construction_oracle.py',
        'tools/tests/test_inferred_new_fixtures.py',
        'tools/java/CollectionConversionsOracle.java',
    },
    'reversal_values_fixtures': {
        'tools/reversal_values_fixtures.py',
        'tools/reversal_values_oracle.py',
        'tools/tests/test_reversal_values_fixtures.py',
        'tools/java/CollectionConversionsOracle.java',
        'tools/collection_conversions_fixtures.py',
        'tools/collection_conversions_oracle.py',
    },
    'reversal_implicit_fixtures': {
        'tools/reversal_implicit_fixtures.py',
        'tools/reversal_implicit_oracle.py',
        'tools/reversal_values_oracle.py',
        'tools/tests/test_reversal_implicit_fixtures.py',
        'tools/java/CollectionConversionsOracle.java',
        'tools/collection_conversions_fixtures.py',
        'tools/collection_conversions_oracle.py',
    },
    'reversal_subscript_fixtures': {
        'tools/reversal_subscript_fixtures.py',
        'tools/reversal_subscript_oracle.py',
        'tools/reversal_values_oracle.py',
        'tools/tests/test_reversal_subscript_fixtures.py',
        'tools/java/CollectionConversionsOracle.java',
        'tools/collection_conversions_fixtures.py',
        'tools/collection_conversions_oracle.py',
    },
    'base_constructor_constraints_fixtures': {
        'tools/base_constructor_constraints_fixtures.py',
        'tools/base_constructor_constraints_oracle.py',
        'tools/reversal_values_oracle.py',
        'tools/tests/test_base_constructor_constraints_fixtures.py',
        'tools/java/CollectionConversionsOracle.java',
        'tools/collection_conversions_fixtures.py',
        'tools/collection_conversions_oracle.py',
    },
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def load_profiles(path=ROOT / CATALOG):
    catalog = json.loads(Path(path).read_bytes())
    require(type(catalog) is dict and set(catalog) == {'schema', 'baseline', 'profiles'}
            and catalog['schema'] == 1 and catalog['baseline'] == 'Pkl 0.32.0'
            and type(catalog['profiles']) is list and catalog['profiles'],
            'invalid collection conversion profile catalog')
    profiles = {}
    fixtures = set()
    for profile in catalog['profiles']:
        require(type(profile) is dict and set(profile) == {
            'module_uri', 'fixture', 'rows', 'repetitions', 'validator_module',
            'validator_script', 'archive_prefix', 'input_hashes', 'tool_sources'},
            'invalid collection conversion profile fields')
        uri, fixture = profile['module_uri'], profile['fixture']
        module, script = profile['validator_module'], profile['validator_script']
        require(type(uri) is str and
                (uri.startswith('file:///stage62/') or
                 uri in ('file:///stage63/collection-addition.pkl',
                         'file:///stage63/set-endpoints.pkl',
                         'file:///stage64/amendment-construction.pkl',
                         'file:///stage65/inferred-new-values.pkl',
                         'file:///stage66/reversal-values.pkl',
                         'file:///stage66/reversal-implicit.pkl',
                         'file:///stage66/reversal-subscript.pkl',
                         'file:///stage66/base-constructors-in-constraints.pkl')) and uri.endswith('.pkl')
                and type(fixture) is str and fixture.startswith('tests/fixtures/m2-')
                and '..' not in Path(fixture).parts and fixture not in fixtures
                and uri not in profiles and type(profile['rows']) is int and profile['rows'] > 0
                and type(profile['repetitions']) is int and profile['repetitions'] == 2
                and module in VALIDATORS and script == VALIDATORS[module]
                and type(profile['archive_prefix']) is str
                and profile['archive_prefix'].startswith('capture-')
                and type(profile['input_hashes']) is dict
                and set(profile['input_hashes']) == set(INPUTS)
                and all(type(value) is str and len(value) == 64
                        and all(char in '0123456789abcdef' for char in value)
                        for value in profile['input_hashes'].values()),
                'invalid or duplicate collection conversion profile')
        sources = profile['tool_sources']
        require(type(sources) is list
                and all(type(name) is str and name.startswith('tools/')
                        and '..' not in Path(name).parts and (ROOT / name).is_file()
                        for name in sources)
                and len(sources) == len(set(sources))
                and set(sources) == PROFILE_TOOL_CLOSURES[module],
                'invalid collection conversion profile source closure')
        profiles[uri] = profile
        fixtures.add(fixture)
    return profiles


PROFILES = load_profiles()
PROFILE_SOURCES = {CATALOG, 'tools/collection_conversion_profiles.py'} | {
    name for profile in PROFILES.values() for name in profile['tool_sources']
}


def profile_for_fixture(fixture):
    fixture = Path(fixture)
    cases = json.loads((fixture / 'cases.json').read_bytes())
    require(type(cases) is dict and type(cases.get('module_uri')) is str
            and cases['module_uri'] in PROFILES, 'unknown collection conversion profile URI')
    profile = PROFILES[cases['module_uri']]
    require(cases.get('schema') == 1 and cases.get('baseline') == 'Pkl 0.32.0'
            and cases.get('boundary') == 'string'
            and type(cases.get('rows')) is list and len(cases['rows']) == profile['rows']
            and cases.get('repetitions') == profile['repetitions'],
            'collection conversion profile row count or boundary differs')
    require(all(digest(fixture / name) == profile['input_hashes'][name] for name in INPUTS),
            'collection conversion profile input identity differs')
    return profile


def authority_for_fixture(fixture):
    profile = profile_for_fixture(fixture)
    authority = importlib.import_module(profile['validator_module'])
    if profile['validator_module'] == 'inferred_new_fixtures':
        require(profile['module_uri'] == 'file:///stage65/inferred-new-values.pkl'
                and profile['archive_prefix'] == 'capture-01',
                'inferred-new values authority boundary differs')
        class ValuesAuthority:
            ARCHIVE_PREFIX = 'capture-01'

            @staticmethod
            def verify(selected_fixture):
                return authority.verify('values', selected_fixture)

        return ValuesAuthority
    require(authority.ARCHIVE_PREFIX == profile['archive_prefix'],
            'collection conversion archive prefix differs')
    return authority


def validator_args(profile, fixture):
    args = [str(ROOT / profile['validator_script'])]
    if profile['validator_module'] == 'inferred_new_fixtures':
        require(profile['module_uri'] == 'file:///stage65/inferred-new-values.pkl',
                'inferred-new validator profile differs')
        args.extend(['--profile', 'values'])
    args.extend(['--fixture', str(fixture)])
    return args

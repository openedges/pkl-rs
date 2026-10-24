"""Shared source catalog for M2 acquisition; independent of runners/manifests."""
import json
from pathlib import Path

MANIFEST = 'compatibility/m2-language.json'
PROFILE = 'compatibility/m2-jvm-classpath.json'
EXECUTION_PROFILES = 'compatibility/m2-execution-profiles.json'
INPUTS = ('references.lock', 'compatibility/inventory.json', 'compatibility/corpus.json',
          'compatibility/rev1-inventory.json', 'compatibility/regressions.json',
          'tests/fixtures/m2/cases.json', 'compatibility/m2-mappings.json',
          'compatibility/m2-module-eval.json', PROFILE, EXECUTION_PROFILES)


def source_paths(root: Path):
    paths = set(INPUTS) | {MANIFEST, 'tools/m2_source_inputs.py', 'tools/m2_manifest.py',
        'tools/m2_language.py', 'tools/m2_module_eval.py', 'tools/m2_workspace.py', 'tools/m2_jvm_classpath.py',
        'tools/m2_execution_profiles.py', 'tools/compat.py', 'tools/m2_generate.py',
        'tools/parameterized_object_fixtures.py', 'tools/parameterized_object_compare.py',
        'tools/platform/compare-parameterized-objects.mjs',
        'tools/platform/compare-parameterized-objects.test.mjs',
        'tools/platform/parameterized-object-session.mjs',
        'tools/platform/module-policy-session.mjs',
        'tools/platform/parameterized-object-session.test.mjs',
        'tools/tests/test_parameterized_object_fixtures.py',
        'tools/tests/test_parameterized_object_compare.py',
        'tools/m2_identifier_tables.py', 'tools/java/M2IdentifierProbe.java',
        'tools/m2_unicode_escape_tables.py', 'tools/java/M2UnicodeEscapeProbe.java',
        'tools/java/M2LanguageProbe.java', 'tools/oracle.py', 'oracle.lock.json',
        'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', '.cargo/config.toml', 'xtask/Cargo.toml',
        'tools/java/M2JvmSnippetOutputProbe.java',
        'tools/java/classpath/org/pkl/core/module/TestModuleKeyFactory.java',
        'tools/java/classpath/org/pkl/core/resource/TestResourceReader.java',
        'tools/tests/test_m2_jvm_classpath.py', 'tools/tests/test_m2_execution_profiles.py',
        'docs/m2-jvm-classpath.md', 'crates/pkl-runtime/src/stdlib/modules.txt'}
    for directory, pattern in [('crates', '*.rs'), ('crates', '*.pkl'), ('crates', 'Cargo.toml'),
                               ('tests/fixtures/m2', '*'),
                               ('tests/fixtures/m2-parameterized-object-bodies', '*')]:
        paths.update(path.relative_to(root).as_posix() for path in (root / directory).rglob(pattern) if path.is_file())
    profile = json.loads((root / PROFILE).read_bytes())
    paths.update(profile['reference']['sources'])
    paths.update(profile['classpath_resources'])
    paths.add(profile['input']['path'])
    return sorted(paths)

"""Shared source catalog for M2 acquisition; independent of runners/manifests."""
import json
from pathlib import Path

MANIFEST = 'compatibility/m2-language.json'
PROFILE = 'compatibility/m2-jvm-classpath.json'
EXECUTION_PROFILES = 'compatibility/m2-execution-profiles.json'
HISTORICAL = 'compatibility/m2-historical-collections.json'
INPUTS = ('references.lock', 'compatibility/inventory.json', 'compatibility/corpus.json',
          'compatibility/rev1-inventory.json', 'compatibility/regressions.json',
          'tests/fixtures/m2/cases.json', 'compatibility/m2-mappings.json',
          'compatibility/m2-module-eval.json', PROFILE, EXECUTION_PROFILES, HISTORICAL)


def source_paths(root: Path):
    paths = set(INPUTS) | {MANIFEST, 'tools/m2_source_inputs.py', 'tools/m2_manifest.py',
        'crates/pkl-runtime/src/vm/string_word_break.bin',
        'crates/pkl-runtime/src/vm/string_word_break.provenance.json',
        'crates/pkl-runtime/src/vm/string_word_break.README.md',
        'tools/m2_language.py', 'tools/m2_module_eval.py', 'tools/m2_historical.py', 'tools/m2_workspace.py', 'tools/m2_jvm_classpath.py',
        'tools/m2_execution_profiles.py', 'tools/compat.py', 'tools/m2_generate.py',
        'tools/benchmark.py',
        'tools/parameterized_object_fixtures.py', 'tools/parameterized_object_compare.py',
        'tools/platform/compare-parameterized-objects.mjs',
        'tools/platform/compare-parameterized-objects.test.mjs',
        'tools/platform/parameterized-object-session.mjs',
        'tools/platform/module-policy-session.mjs',
        'tools/platform/parameterized-object-session.test.mjs',
        'tools/tests/test_parameterized_object_fixtures.py',
        'tools/tests/test_parameterized_object_compare.py',
        'tools/deprecation_fixtures.py', 'tools/deprecation_compare.py', 'tools/m2_deprecation.py',
        'tools/java/M2DeprecationProbe.java', 'tools/java/M2DeprecationCyclesProbe.java',
        'tools/tests/test_deprecation_fixtures.py', 'tools/tests/test_deprecation_compare.py',
        'tools/platform/deprecation-session.mjs', 'tools/platform/deprecation-session.test.mjs',
        'tools/platform/capture-deprecation.mjs', 'tools/platform/capture-deprecation.test.mjs',
        'tools/frame_transformer_oracle.py', 'tools/frame_transformer_fixtures.py',
        'tools/frame_transformer_compare.py', 'tools/java/M2FrameTransformerProbe.java',
        'tools/tests/test_frame_transformer_fixtures.py',
        'tools/tests/test_frame_transformer_compare.py',
        'tools/platform/frame-transformer-session.mjs',
        'tools/platform/frame-transformer-session.test.mjs',
        'tools/platform/capture-frame-transformer.mjs',
        'tools/platform/capture-frame-transformer.test.mjs',
        'tools/class_values_oracle.py', 'tools/class_values_exports.py',
        'tools/class_values_fixtures.py', 'tools/class_values_compare.py',
        'tools/class_metadata_oracle.py', 'tools/class_metadata_fixtures.py',
        'tools/tests/test_class_metadata_fixtures.py',
        'tools/java/M2ClassValuesProbe.java', 'tools/java/M2ClassExportProbe.java',
        'tools/tests/test_class_values_fixtures.py', 'tools/tests/test_class_values_compare.py',
        'tools/platform/class-values-session.mjs', 'tools/platform/class-values-session.test.mjs',
        'tools/platform/capture-class-values.mjs', 'tools/platform/capture-class-values.test.mjs',
        'tools/platform/resource-session.mjs',
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
                               ('tests/fixtures/m2-string-apis-oracle', '*'),
                               ('tests/fixtures/m2-numeric-apis-oracle', '*'),
                               ('tests/fixtures/m2-set-apis-oracle', '*'),
                               ('tests/fixtures/m2-list-apis-oracle', '*'),
                               ('tests/fixtures/m2-map-apis-oracle', '*'),
                               ('tests/fixtures/m2-mapping-apis-oracle', '*'),
                               ('tests/fixtures/m2-typed-hasproperty-oracle', '*'),
                               ('tests/fixtures/m2-typed-property-oracle', '*'),
                               ('tests/fixtures/m2-module-relativepath-oracle', '*'),
                               ('tests/fixtures/m2-annotation-const-oracle', '*'),
                               ('tests/fixtures/m2-regex-pcf-oracle', '*'),
                               ('tests/fixtures/m2-base-failures-oracle', '*'),
                               ('tests/fixtures/m2-original-diagnostics-stage113', '*'),
                               ('tests/fixtures/m2-stage113-unicode-class', '*'),
                               ('tests/fixtures/m2-reflection-apis-oracle', '*'),
                               ('tests/fixtures/m2-deprecation', '*'),
                               ('tests/fixtures/m2-frame-transformers', '*'),
                               ('tests/fixtures/m2-class-values', '*'),
                               ('tests/fixtures/m2-class-metadata-completion', '*'),
                               ('tests/fixtures/m2-parameterized-object-bodies', '*')]:
        paths.update(path.relative_to(root).as_posix() for path in (root / directory).rglob(pattern) if path.is_file())
    historical = json.loads((root / HISTORICAL).read_bytes())
    paths.update(resource['path'] for resource in historical['resources'])
    profile = json.loads((root / PROFILE).read_bytes())
    paths.update(profile['reference']['sources'])
    paths.update(profile['classpath_resources'])
    paths.add(profile['input']['path'])
    from vendor_sources import vendor_source_paths
    paths.add('tools/vendor_sources.py')
    paths.update(path.relative_to(root).as_posix() for path in vendor_source_paths(root))
    return sorted(paths)

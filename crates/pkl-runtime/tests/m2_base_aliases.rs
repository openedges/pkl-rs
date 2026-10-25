//! Exact Java 0.32.0 results for separate pkl:base alias modules.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::{probe::SourceProbe, value::PklString};
struct Row {
    id: &'static str,
    uri: &'static str,
    source: &'static str,
    kind: &'static str,
    utf8: &'static [u8],
    utf16: &'static [u8],
}
fn units(bytes: &[u8]) -> Vec<u16> {
    let (pairs, tail) = bytes.as_chunks::<2>();
    assert!(tail.is_empty());
    pairs.iter().map(|pair| u16::from_be_bytes(*pair)).collect()
}
#[test]
fn exact_java_base_alias_modules() {
    let rows = [
        Row {
            id: "module-instance",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-000-module-instance.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-000-module-instance.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-000-module-instance.utf16be.bin"
            ),
        },
        Row {
            id: "qualified-alias-values",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-001-qualified-alias-values.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-001-qualified-alias-values.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-001-qualified-alias-values.utf16be.bin"
            ),
        },
        Row {
            id: "implicit-alias-values",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-002-implicit-alias-values.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-002-implicit-alias-values.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-002-implicit-alias-values.utf16be.bin"
            ),
        },
        Row {
            id: "alias-value-identity",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-003-alias-value-identity.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-003-alias-value-identity.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-003-alias-value-identity.utf16be.bin"
            ),
        },
        Row {
            id: "comparable-int",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-004-comparable-int.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-004-comparable-int.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-004-comparable-int.utf16be.bin"
            ),
        },
        Row {
            id: "comparable-duration",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-005-comparable-duration.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-005-comparable-duration.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-005-comparable-duration.utf16be.bin"
            ),
        },
        Row {
            id: "comparable-data-size",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-006-comparable-data-size.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-006-comparable-data-size.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-006-comparable-data-size.utf16be.bin"
            ),
        },
        Row {
            id: "comparable-implicit-string",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-007-comparable-implicit-string.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-007-comparable-implicit-string.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-007-comparable-implicit-string.utf16be.bin"
            ),
        },
        Row {
            id: "comparable-qualified-error",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-008-comparable-qualified-error.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-008-comparable-qualified-error.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-008-comparable-qualified-error.utf16be.bin"
            ),
        },
        Row {
            id: "comparable-implicit-error",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-009-comparable-implicit-error.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-009-comparable-implicit-error.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-009-comparable-implicit-error.utf16be.bin"
            ),
        },
        Row {
            id: "comparable-alias-frames",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-010-comparable-alias-frames.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-010-comparable-alias-frames.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-010-comparable-alias-frames.utf16be.bin"
            ),
        },
        Row {
            id: "comparable-argument-error-order",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-011-comparable-argument-error-order.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-011-comparable-argument-error-order.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-011-comparable-argument-error-order.utf16be.bin"
            ),
        },
        Row {
            id: "char-ascii",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!("../../../tests/fixtures/m2-base-aliases/main-012-char-ascii.pkl"),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-012-char-ascii.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-012-char-ascii.utf16be.bin"
            ),
        },
        Row {
            id: "char-supplementary",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-013-char-supplementary.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-013-char-supplementary.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-013-char-supplementary.utf16be.bin"
            ),
        },
        Row {
            id: "char-empty-error",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-014-char-empty-error.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-014-char-empty-error.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-014-char-empty-error.utf16be.bin"
            ),
        },
        Row {
            id: "char-combining-error",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-015-char-combining-error.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-015-char-combining-error.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-015-char-combining-error.utf16be.bin"
            ),
        },
        Row {
            id: "char-nonstring-error",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-016-char-nonstring-error.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-016-char-nonstring-error.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-016-char-nonstring-error.utf16be.bin"
            ),
        },
        Row {
            id: "char-implicit-error",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-017-char-implicit-error.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-017-char-implicit-error.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-017-char-implicit-error.utf16be.bin"
            ),
        },
        Row {
            id: "uri-qualified",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-018-uri-qualified.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-018-uri-qualified.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-018-uri-qualified.utf16be.bin"
            ),
        },
        Row {
            id: "uri-implicit",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-019-uri-implicit.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-019-uri-implicit.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-019-uri-implicit.utf16be.bin"
            ),
        },
        Row {
            id: "uri-type-error",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-020-uri-type-error.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-020-uri-type-error.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-020-uri-type-error.utf16be.bin"
            ),
        },
        Row {
            id: "mixin-qualified-default",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-021-mixin-qualified-default.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-021-mixin-qualified-default.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-021-mixin-qualified-default.utf16be.bin"
            ),
        },
        Row {
            id: "mixin-implicit-default",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-022-mixin-implicit-default.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-022-mixin-implicit-default.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-022-mixin-implicit-default.utf16be.bin"
            ),
        },
        Row {
            id: "mixin-qualified-amendment",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-023-mixin-qualified-amendment.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-023-mixin-qualified-amendment.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-023-mixin-qualified-amendment.utf16be.bin"
            ),
        },
        Row {
            id: "mixin-qualified-arity-error",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-024-mixin-qualified-arity-error.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-024-mixin-qualified-arity-error.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-024-mixin-qualified-arity-error.utf16be.bin"
            ),
        },
        Row {
            id: "alias-shadow",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-025-alias-shadow.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-025-alias-shadow.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-025-alias-shadow.utf16be.bin"
            ),
        },
        Row {
            id: "alias-qualified-shadow",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-026-alias-qualified-shadow.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-026-alias-qualified-shadow.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-026-alias-qualified-shadow.utf16be.bin"
            ),
        },
        Row {
            id: "unused-invalid-alias-value",
            uri: "file:///stage71/base-alias-oracle.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/main-027-unused-invalid-alias-value.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-027-unused-invalid-alias-value.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/main-027-unused-invalid-alias-value.utf16be.bin"
            ),
        },
        Row {
            id: "qualified-typed-public",
            uri: "file:///stage71/base-alias-supplement.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/supplement-000-qualified-typed-public.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/supplement-000-qualified-typed-public.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/supplement-000-qualified-typed-public.utf16be.bin"
            ),
        },
        Row {
            id: "implicit-typed-public",
            uri: "file:///stage71/base-alias-supplement.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/supplement-001-implicit-typed-public.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/supplement-001-implicit-typed-public.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/supplement-001-implicit-typed-public.utf16be.bin"
            ),
        },
        Row {
            id: "qualified-explicit-apply",
            uri: "file:///stage71/base-alias-supplement.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/supplement-002-qualified-explicit-apply.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/supplement-002-qualified-explicit-apply.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/supplement-002-qualified-explicit-apply.utf16be.bin"
            ),
        },
        Row {
            id: "qualified-alias-pipe",
            uri: "file:///stage71/base-alias-supplement.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/supplement-003-qualified-alias-pipe.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/supplement-003-qualified-alias-pipe.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/supplement-003-qualified-alias-pipe.utf16be.bin"
            ),
        },
        Row {
            id: "qualified-amended-mixin",
            uri: "file:///stage71/base-alias-supplement.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/supplement-004-qualified-amended-mixin.pkl"
            ),
            kind: "value",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/supplement-004-qualified-amended-mixin.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/supplement-004-qualified-amended-mixin.utf16be.bin"
            ),
        },
        Row {
            id: "qualified-wrong-apply-arity",
            uri: "file:///stage71/base-alias-supplement.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/supplement-005-qualified-wrong-apply-arity.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/supplement-005-qualified-wrong-apply-arity.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/supplement-005-qualified-wrong-apply-arity.utf16be.bin"
            ),
        },
        Row {
            id: "comparable-missing",
            uri: "file:///stage71/base-alias-argument-order.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/arguments-000-comparable-missing.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/arguments-000-comparable-missing.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/arguments-000-comparable-missing.utf16be.bin"
            ),
        },
        Row {
            id: "char-missing",
            uri: "file:///stage71/base-alias-argument-order.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/arguments-001-char-missing.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/arguments-001-char-missing.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/arguments-001-char-missing.utf16be.bin"
            ),
        },
        Row {
            id: "uri-missing",
            uri: "file:///stage71/base-alias-argument-order.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/arguments-002-uri-missing.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/arguments-002-uri-missing.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/arguments-002-uri-missing.utf16be.bin"
            ),
        },
        Row {
            id: "comparable-excess-known",
            uri: "file:///stage71/base-alias-argument-order.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/arguments-003-comparable-excess-known.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/arguments-003-comparable-excess-known.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/arguments-003-comparable-excess-known.utf16be.bin"
            ),
        },
        Row {
            id: "char-excess-known",
            uri: "file:///stage71/base-alias-argument-order.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/arguments-004-char-excess-known.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/arguments-004-char-excess-known.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/arguments-004-char-excess-known.utf16be.bin"
            ),
        },
        Row {
            id: "uri-excess-known",
            uri: "file:///stage71/base-alias-argument-order.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/arguments-005-uri-excess-known.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/arguments-005-uri-excess-known.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/arguments-005-uri-excess-known.utf16be.bin"
            ),
        },
        Row {
            id: "mixin-missing-and-excess",
            uri: "file:///stage71/base-alias-argument-order.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/arguments-006-mixin-missing-and-excess.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/arguments-006-mixin-missing-and-excess.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/arguments-006-mixin-missing-and-excess.utf16be.bin"
            ),
        },
        Row {
            id: "mixin-excess-known",
            uri: "file:///stage71/base-alias-argument-order.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/arguments-007-mixin-excess-known.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/arguments-007-mixin-excess-known.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/arguments-007-mixin-excess-known.utf16be.bin"
            ),
        },
        Row {
            id: "qualified-mixin-missing-and-excess",
            uri: "file:///stage71/base-alias-argument-order.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/arguments-008-qualified-mixin-missing-and-excess.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/arguments-008-qualified-mixin-missing-and-excess.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/arguments-008-qualified-mixin-missing-and-excess.utf16be.bin"
            ),
        },
        Row {
            id: "qualified-identity-wrong",
            uri: "file:///stage71/mixin-default-origin.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/mixin-origin-000-qualified-identity-wrong.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/mixin-origin-000-qualified-identity-wrong.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/mixin-origin-000-qualified-identity-wrong.utf16be.bin"
            ),
        },
        Row {
            id: "implicit-identity-wrong",
            uri: "file:///stage71/mixin-default-origin.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/mixin-origin-001-implicit-identity-wrong.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/mixin-origin-001-implicit-identity-wrong.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/mixin-origin-001-implicit-identity-wrong.utf16be.bin"
            ),
        },
        Row {
            id: "user-alias-identity-wrong",
            uri: "file:///stage71/mixin-default-origin.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/mixin-origin-002-user-alias-identity-wrong.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/mixin-origin-002-user-alias-identity-wrong.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/mixin-origin-002-user-alias-identity-wrong.utf16be.bin"
            ),
        },
        Row {
            id: "nested-list-int-wrong",
            uri: "file:///stage71/mixin-default-origin-nested.pkl",
            source: include_str!(
                "../../../tests/fixtures/m2-base-aliases/mixin-nested-000-nested-list-int-wrong.pkl"
            ),
            kind: "error",
            utf8: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/mixin-nested-000-nested-list-int-wrong.utf8.bin"
            ),
            utf16: include_bytes!(
                "../../../tests/fixtures/m2-base-aliases/mixin-nested-000-nested-list-int-wrong.utf16be.bin"
            ),
        },
    ];
    let mut failures = Vec::new();
    for row in rows {
        let mut probe = SourceProbe::new_module_evaluator("compact", true, false, false).unwrap();
        probe
            .evaluate_module_source(row.uri, Some(row.source), "result", 0, None)
            .unwrap();
        let mut terminal = None;
        for _ in 0..100_000 {
            probe.collect();
            let event = probe.poll(0, 16).unwrap();
            probe.collect();
            if event.kind == "yield" {
                continue;
            }
            if event.kind == "module-request"
                && module_keys::complete_probe(&mut probe, event.token.as_deref().unwrap(), 0)
                    .unwrap()
            {
                continue;
            }
            terminal = Some(event);
            break;
        }
        let event = terminal.expect("bounded alias evaluation");
        let actual = if event.kind == "value" {
            event.value.as_ref()
        } else {
            event.error.as_ref()
        };
        let expected = units(row.utf16);
        if event.kind != row.kind
            || actual.is_none_or(|value| {
                *value != expected
                    || PklString::from_utf16(value.clone())
                        .to_java_utf8()
                        .as_bytes()
                        != row.utf8
            })
        {
            failures.push(format!(
                "{}: expected {} {:?}, got {} {:?}",
                row.id,
                row.kind,
                String::from_utf16_lossy(&expected),
                event.kind,
                actual.map(|value| String::from_utf16_lossy(value))
            ));
        }
        probe.close();
        assert_eq!(probe.collect().slots, 0, "{} released roots", row.id);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

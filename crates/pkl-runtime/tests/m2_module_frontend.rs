//! Original parser rejection observations from pinned Pkl 0.32.0.
//! Full original CLI stdout/stderr/exit/generated-files comparisons are separate.
use pkl_runtime::diagnostic::{DiagnosticOrigin, FailureDisposition};
use pkl_runtime::evaluation::frontend::{
    DiagnosticProfile, FrontendPhase, ModuleEvalOptions, ModuleSourceInput,
    evaluate_module_frontend,
};
use pkl_runtime::{parser::parse_module, source::SourceMap};

#[test]
fn original_72_parser_messages_and_utf16_spans_match_pinned_observations() {
    // Fresh public Parser.parseModule observations: stage39-original72-phase-01.
    // Inputs are original pinned files, never reduced expressions or rewritten fixtures.
    let cases: &[(&str, &str, &str, usize, usize)] = &[
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingClassDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingClassDelimiter.pkl"
            )),
            "Missing `}` delimiter.",
            29,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingConstrainedTypeSeparator.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingConstrainedTypeSeparator.pkl"
            )),
            "Unexpected token `this`. Expected `,` or `)`.",
            18,
            4,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingContainerAmendDefDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingContainerAmendDefDelimiter.pkl"
            )),
            "Missing `}` delimiter.",
            13,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingContainerAmendExprDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingContainerAmendExprDelimiter.pkl"
            )),
            "Missing `}` delimiter.",
            21,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingEmptyMultiLineStringDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingEmptyMultiLineStringDelimiter.pkl"
            )),
            "Missing `\"\"\"` delimiter.",
            173,
            0,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingEmptyMultiLineStringDelimiterAtEof.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingEmptyMultiLineStringDelimiterAtEof.pkl"
            )),
            "Missing `\"\"\"` delimiter.",
            44,
            0,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingEmptyStringDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingEmptyStringDelimiter.pkl"
            )),
            "Missing `\"` delimiter.",
            8,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingEmptyStringDelimiterAtEof.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingEmptyStringDelimiterAtEof.pkl"
            )),
            "Missing `\"` delimiter.",
            183,
            0,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingFunction0ParameterListDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingFunction0ParameterListDelimiter.pkl"
            )),
            "Unexpected token `->`. Expected `)`.",
            6,
            2,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingFunction1ParameterListDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingFunction1ParameterListDelimiter.pkl"
            )),
            "Unexpected token `->`. Expected `)`.",
            9,
            2,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingFunctionAmendParameterListSeparator.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingFunctionAmendParameterListSeparator.pkl"
            )),
            "Unexpected token `arg3`. Expected `,` or `->`.",
            15,
            4,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingFunctionNParameterListDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingFunctionNParameterListDelimiter.pkl"
            )),
            "Unexpected token `->`. Expected `,` or `)`.",
            22,
            2,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingFunctionParameterListSeparator.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingFunctionParameterListSeparator.pkl"
            )),
            "Unexpected token `arg3`. Expected `,` or `)`.",
            16,
            4,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingFunctionType0ParameterListDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingFunctionType0ParameterListDelimiter.pkl"
            )),
            "Unexpected token `->`. Expected a type or `)`.",
            5,
            2,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingFunctionType1ParameterListDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingFunctionType1ParameterListDelimiter.pkl"
            )),
            "Unexpected token `->`. Expected `,` or `)`.",
            11,
            2,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingFunctionTypeNParameterListDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingFunctionTypeNParameterListDelimiter.pkl"
            )),
            "Unexpected token `->`. Expected `,` or `)`.",
            24,
            2,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingFunctionTypeParameterListSeparator.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingFunctionTypeParameterListSeparator.pkl"
            )),
            "Unexpected token `String`. Expected `,` or `)`.",
            16,
            6,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingIfExprDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingIfExprDelimiter.pkl"
            )),
            "Unexpected token `3`. Expected `)`.",
            16,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingListDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingListDelimiter.pkl"
            )),
            "Unexpected token `EOF`. Expected `,` or `)`.",
            21,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingListSeparator.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingListSeparator.pkl"
            )),
            "Unexpected token `3`. Expected `,` or `)`.",
            18,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingMapDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingMapDelimiter.pkl"
            )),
            "Unexpected token `EOF`. Expected `,` or `)`.",
            28,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingMapSeparator.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingMapSeparator.pkl"
            )),
            "Unexpected token `4`. Expected `,` or `)`.",
            19,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingMethodArgumentListDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingMethodArgumentListDelimiter.pkl"
            )),
            "Unexpected token `res2`. Expected `,` or `)`.",
            35,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingMethodArgumentListSeparator.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingMethodArgumentListSeparator.pkl"
            )),
            "Unexpected token `3`. Expected `,` or `)`.",
            16,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingMethodParameterListDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingMethodParameterListDelimiter.pkl"
            )),
            "Unexpected token `=`. Expected `,` or `)`.",
            24,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingMethodParameterListSeparator.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingMethodParameterListSeparator.pkl"
            )),
            "Unexpected token `arg2`. Expected `,` or `)`.",
            18,
            4,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingMultiLineStringDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingMultiLineStringDelimiter.pkl"
            )),
            "Missing `\"\"\"` delimiter.",
            25,
            0,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingObjectAmendDefDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingObjectAmendDefDelimiter.pkl"
            )),
            "Missing `}` delimiter.",
            13,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingObjectAmendDefDelimiter2.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingObjectAmendDefDelimiter2.pkl"
            )),
            "Missing `}` delimiter.",
            25,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingObjectAmendDefDelimiter3.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingObjectAmendDefDelimiter3.pkl"
            )),
            "Missing `}` delimiter.",
            66,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingObjectAmendExprDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingObjectAmendExprDelimiter.pkl"
            )),
            "Missing `}` delimiter.",
            21,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingObjectDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingObjectDelimiter.pkl"
            )),
            "Missing `}` delimiter.",
            19,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingParenthesizedExprDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingParenthesizedExprDelimiter.pkl"
            )),
            "Unexpected token `y`. Expected `)`.",
            14,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingParenthesizedTypeDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingParenthesizedTypeDelimiter.pkl"
            )),
            "Unexpected token `res2`. Expected `,` or `)`.",
            40,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingRawMultiLineStringDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingRawMultiLineStringDelimiter.pkl"
            )),
            "Missing `\"\"\"#` delimiter.",
            16,
            0,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingRawStringDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingRawStringDelimiter.pkl"
            )),
            "Missing `\"#` delimiter.",
            12,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingSetSeparator.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingSetSeparator.pkl"
            )),
            "Unexpected token `3`. Expected `,` or `)`.",
            16,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingStringDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingStringDelimiter.pkl"
            )),
            "Missing `\"` delimiter.",
            11,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingSubscriptDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingSubscriptDelimiter.pkl"
            )),
            "Unexpected token `EOF`. Expected `]`.",
            14,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingTypeConstraintListDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingTypeConstraintListDelimiter.pkl"
            )),
            "Unexpected token `=`. Expected `,` or `)`.",
            34,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingTypedMethodParameterListDelimiter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingTypedMethodParameterListDelimiter.pkl"
            )),
            "Unexpected token `=`. Expected `,` or `)`.",
            40,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingTypedMethodParameterListSeparator.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/missingTypedMethodParameterListSeparator.pkl"
            )),
            "Unexpected token `arg2`. Expected `,` or `)`.",
            26,
            4,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/unbalancedEntryBrackets1.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/unbalancedEntryBrackets1.pkl"
            )),
            "Unexpected token `{`. Expected `]]`.",
            32,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/unbalancedEntryBrackets2.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/unbalancedEntryBrackets2.pkl"
            )),
            "Unexpected token `]`. Expected `{` or `=`.",
            32,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/unbalancedEntryBrackets3.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/unbalancedEntryBrackets3.pkl"
            )),
            "Unexpected token `] ]`. Expected `]]`.",
            32,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/unbalancedEntryBrackets4.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/delimiters/unbalancedEntryBrackets4.pkl"
            )),
            "Unexpected token `]`. Expected `{` or `=`.",
            33,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser1.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser1.pkl"
            )),
            "Unexpected end of file.",
            8,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser10.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser10.pkl"
            )),
            "Unexpected token `[`. Expected a type or `{`.",
            11,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser11.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser11.pkl"
            )),
            "Unexpected token `]`. Expected `}`.",
            22,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser12.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser12.pkl"
            )),
            "Invalid token at position. Expected a class, typealias, method, or property.",
            23,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser14.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser14.pkl"
            )),
            "Unexpected end of file.",
            58,
            0,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser15.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser15.pkl"
            )),
            "Unexpected separator character.\n\nThe separator character (`_`) cannot follow `0x`, `0b`, `.`, `e`, or 'E' in a number literal.",
            11,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser16.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser16.pkl"
            )),
            "Unexpected separator character.\n\nThe separator character (`_`) cannot follow `0x`, `0b`, `.`, `e`, or 'E' in a number literal.",
            9,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser17.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser17.pkl"
            )),
            "Unexpected separator character.\n\nThe separator character (`_`) cannot follow `0x`, `0b`, `.`, `e`, or 'E' in a number literal.",
            9,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser18.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser18.pkl"
            )),
            "Unexpected separator character.\n\nThe separator character (`_`) cannot follow `0x`, `0b`, `.`, `e`, or 'E' in a number literal.",
            11,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser19.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser19.pkl"
            )),
            "Line must match or exceed indentation of the String's last line.",
            20,
            0,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser2.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser2.pkl"
            )),
            "Missing `\"` delimiter.",
            19,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser20.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser20.pkl"
            )),
            "Invalid line continuation escape sequence.\n\nLine continuations are only allowed in multi-line strings.",
            13,
            2,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser21.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser21.pkl"
            )),
            "Invalid line continuation escape sequence.\n\nWhitespace between the continuation escape and following newline is not allowed.",
            20,
            5,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser3.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser3.pkl"
            )),
            "Missing `\"\"\"` delimiter.",
            21,
            0,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser4.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser4.pkl"
            )),
            "Missing `\"\"\"` delimiter.",
            30,
            0,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser5.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser5.pkl"
            )),
            "Invalid token at position. Expected a class, typealias, method, or property.",
            20,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser6.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser6.pkl"
            )),
            "Missing `\"\"\"` delimiter.",
            22,
            0,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser7.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser7.pkl"
            )),
            "Unexpected token `11`. Expected a type.",
            17,
            2,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser8.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser8.pkl"
            )),
            "Line must match or exceed indentation of the String's last line.",
            14,
            17,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser9.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/errors/parser9.pkl"
            )),
            "Line must match or exceed indentation of the String's last line.",
            15,
            17,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/parser/amendsRequiresParens.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/parser/amendsRequiresParens.pkl"
            )),
            "Unexpected token: `{`.\n\nIf you meant to write an amends expression, wrap the parent in parentheses. Try: `(bar) { ... }`",
            13,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/parser/constantStringInterpolation.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/parser/constantStringInterpolation.pkl"
            )),
            "String constant cannot have interpolated values.",
            8,
            2,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/parser/invalidCharacter.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/parser/invalidCharacter.pkl"
            )),
            "Invalid identifier `^`.",
            8,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/parser/spacesBetweenDocComments.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/parser/spacesBetweenDocComments.pkl"
            )),
            "Dangling documentation comment.\n\nDocumentation comments must be attached to modules, classes, typealiases, methods, or properties.",
            23,
            28,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/parser/typeAnnotationInAmends.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/parser/typeAnnotationInAmends.pkl"
            )),
            "Properties with type annotations cannot have object bodies.\n\nTo define both a type annotation and an object body, try using assignment instead.\nFor example: `obj: Bird = new { ... }`.",
            22,
            1,
        ),
        (
            "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/parser/wrongDocComment.pkl",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/parser/wrongDocComment.pkl"
            )),
            "Imports cannot have doc comments, annotations or modifiers.",
            28,
            6,
        ),
    ];
    assert_eq!(cases.len(), 72);
    for &(name, source, message, offset, length) in cases {
        let error = parse_module(source).unwrap_err();
        assert_eq!(
            error.message.to_java_utf8().as_bytes(),
            message.as_bytes(),
            "{name}"
        );
        assert_eq!(
            error.provenance.disposition,
            FailureDisposition::LanguageError,
            "{name}"
        );
        assert_ne!(
            error.provenance.origin,
            DiagnosticOrigin::Unclassified,
            "{name}"
        );
        let mut sources = SourceMap::default();
        let id = sources.add(name, source);
        let source = sources.get(id).unwrap();
        let (actual_offset, actual_length) = source.utf16_span(error.span).unwrap();
        assert_eq!(
            (
                actual_offset,
                error.highlight_length_utf16.unwrap_or(actual_length)
            ),
            (offset, length),
            "{name}"
        );
    }
}

fn evaluate(
    source: &str,
    profile: DiagnosticProfile,
) -> pkl_runtime::evaluation::frontend::ModuleFrontendResult {
    evaluate_module_frontend(
        ModuleSourceInput {
            text: source,
            uri: "file:///frontend-input.pkl",
            inferred_name: "frontend-input",
        },
        &ModuleEvalOptions {
            diagnostic_profile: profile,
            ..Default::default()
        },
    )
}

#[test]
fn whole_module_rejection_keeps_inferred_identity_and_structured_profile() {
    let source = "module named\nfirst = 1\nlater = )\n";
    let identity = evaluate(source, DiagnosticProfile::ApiIdentity);
    let cli = evaluate(source, DiagnosticProfile::CliBundledSettings);
    assert_eq!(identity.disposition, FailureDisposition::LanguageError);
    assert_eq!(identity.phase, FrontendPhase::Parsing);
    assert_eq!(identity.exit_code, 1);
    assert!(identity.stdout.is_empty());
    assert_eq!(
        String::from_utf8(identity.stderr).unwrap(),
        "–– Pkl Error ––\nUnexpected token `)`.\n\n3 | later = )\n            ^\nat frontend-input (file:///frontend-input.pkl)\n"
    );
    assert_eq!(
        String::from_utf8(cli.stderr).unwrap(),
        "–– Pkl Error ––\nUnexpected token `)`.\n\n3 | later = )\n            ^\nat frontend-input (file:///frontend-input.pkl, line 3)\n"
    );
}

#[test]
fn keyword_expressions_do_not_commit_to_lambda_parameters() {
    // Fresh actual CLI observations: stage39-lambda-keyword-review-01.
    // Keyword atoms enter the grouped-expression grammar; comma/colon then
    // rejects at the delimiter, not at the keyword as a parameter identifier.
    for (source, delimiter, offset) in [
        ("result = (true, 1)\n", ",", 14),
        ("result = (false: Int)\n", ":", 15),
        ("result = (null, 1)\n", ",", 14),
        ("result = (this, 1)\n", ",", 14),
    ] {
        let error = parse_module(source).unwrap_err();
        assert_eq!(
            error.message,
            format!("Unexpected token `{delimiter}`. Expected `)`.")
        );
        assert_eq!(
            error.span,
            pkl_runtime::source::Span::new(offset, offset + 1)
        );
        assert_eq!(error.provenance.origin, DiagnosticOrigin::Parser);
        assert_eq!(
            error.provenance.disposition,
            FailureDisposition::LanguageError
        );
    }
}

#[test]
fn grouped_keywords_and_actual_lambda_parameters_remain_distinct() {
    use pkl_runtime::syntax::ExprKind;

    // Declaration-only acceptance observations: stage39-lambda-declaration-oracle-01.
    for atom in ["true", "false", "null", "this"] {
        let module = parse_module(&format!("result = ({atom})\n")).unwrap();
        assert!(matches!(
            module.members[0].expr.kind,
            ExprKind::Parenthesized(_)
        ));
    }
    for (header, names) in [
        ("(x, y)", vec!["x", "y"]),
        ("(x: Int)", vec!["x"]),
        ("(`true`, y)", vec!["true", "y"]),
        ("(_)", vec!["_"]),
        ("()", vec![]),
    ] {
        let source = format!("local f = {header} -> 7\nresult = 7\n");
        let module = parse_module(&source).unwrap();
        let ExprKind::Function { parameters, .. } = &module.members[0].expr.kind else {
            panic!("expected function for {header}");
        };
        assert_eq!(
            parameters
                .iter()
                .map(|parameter| parameter.name.clone())
                .collect::<Vec<_>>(),
            names
        );
    }
    // A genuine parameter-list delimiter commits even when the arrow is
    // missing. Do not fix keyword atoms by swallowing parameter diagnostics.
    // Exact EOF wording: stage39-lambda-commit-oracle-01, all five heads.
    for header in ["(x, y)", "(x: Int)", "(`true`, y)", "(_)", "()"] {
        let error = parse_module(&format!("local f = {header}\n")).unwrap_err();
        assert_eq!(error.message, "Unexpected token `EOF`. Expected `->`.");
    }
}

#[test]
fn preparation_success_never_claims_module_output_evaluation() {
    for source in [
        "result = 1",
        "result = throw(\"must not execute\")",
        "/// first\n// bridge\n/// next\nresult = 1",
    ] {
        let result = evaluate(source, DiagnosticProfile::CliBundledSettings);
        assert_eq!(result.phase, FrontendPhase::ModuleOutput);
        assert_eq!(result.disposition, FailureDisposition::Unsupported);
        assert_eq!(result.reason, Some("module-output-evaluation"));
        assert_eq!(result.exit_code, 2);
        assert!(result.stdout.is_empty());
    }
}

#[test]
fn ordinary_rejections_capability_gaps_and_limits_remain_distinct() {
    for source in [
        "result = [1]",
        "function if() = 1",
        "external result = 1",
        "result = 1 ^ 2",
    ] {
        assert_eq!(
            evaluate(source, DiagnosticProfile::ApiIdentity).disposition,
            FailureDisposition::LanguageError
        );
    }
    for source in [
        "import* \"foo*.pkl\"",
        "result = import(\"foo.pkl\")",
        "import \"\\u{D800}\"",
    ] {
        let result = evaluate(source, DiagnosticProfile::ApiIdentity);
        assert_eq!(result.disposition, FailureDisposition::Unsupported);
        assert!(result.reason.is_some());
        assert_eq!(result.exit_code, 2);
    }
    let source = format!("result: {}Int{}", "(".repeat(33), ")".repeat(33));
    let result = evaluate(&source, DiagnosticProfile::ApiIdentity);
    assert_eq!(result.disposition, FailureDisposition::LimitExceeded);
    assert_eq!(result.exit_code, 3);
}

#[test]
fn constant_string_interpolation_rejects_before_its_body_or_later_escape() {
    for (source, offset) in [
        ("import \"\\( )\"", 8),
        ("import \"\\(1)\\q\"", 8),
        ("extends \"\\( )\"", 9),
    ] {
        let error = parse_module(source).unwrap_err();
        assert_eq!(
            error.message,
            "String constant cannot have interpolated values."
        );
        assert_eq!(
            error.span,
            pkl_runtime::source::Span::new(offset, offset + 2)
        );
    }
}

#[test]
fn delayed_validation_and_object_parameters_do_not_hide_syntax() {
    for source in [
        "first = 9223372036854775808\nlater = )",
        "first = new { x -> value = x }\nlater = )",
    ] {
        let error = parse_module(source).unwrap_err();
        assert_eq!(error.message, "Unexpected token `)`.");
        assert_eq!(
            error.provenance.disposition,
            FailureDisposition::LanguageError
        );
    }
    let parsed = parse_module("first = new { x: Int, -> value = x }").unwrap();
    let pkl_runtime::syntax::ExprKind::Object { parameters, .. } = &parsed.members[0].expr.kind
    else {
        panic!("inferred object body");
    };
    assert_eq!(parameters.len(), 1);
    assert_eq!(parameters[0].name, "x");
    assert!(parameters[0].ty.is_some());
    assert!(parse_module("result = new Dynamic { x = 1 } { y = 2 }").is_ok());
}

#[test]
fn accepted_documentation_and_annotation_parameters_keep_runtime_semantics() {
    let source = "/// module docs\nmodule documented\n/// method docs\n// bridge\n/// continued\n@Deprecated { parameter: Missing -> }\nfunction value() = 7\nresult = value()\n";
    assert_eq!(
        pkl_runtime::evaluation::scalar_expression(source, "file:///docs.pkl", "docs", "result")
            .unwrap(),
        "7"
    );
}

#[test]
fn cli_boundary_rejections_keep_exact_source_display_and_first_error() {
    let cases = [
        (
            "import \"\\( )\"\nresult = 1\n",
            "String constant cannot have interpolated values.\n\n1 | import \"\\()\"\n            ^^",
            1,
        ),
        (
            "import \"\\(1)\\q\"\nresult = 1\n",
            "String constant cannot have interpolated values.\n\n1 | import \"\\(1)\\q\"\n            ^^",
            1,
        ),
        (
            "extends \"\\( )\"\n",
            "String constant cannot have interpolated values.\n\n1 | extends \"\\()\"\n             ^^",
            1,
        ),
        (
            "/// first\n\n/// second\nresult = 1\n",
            "Dangling documentation comment.\n\nDocumentation comments must be attached to modules, classes, typealiases, methods, or properties.\n\n3 | /// second\n    ^^^^^^^^^^",
            3,
        ),
        (
            "result = 1\n/// trailing\n",
            "Unexpected end of file.\n\n3 | \n    ^",
            3,
        ),
    ];
    for (source, body, line) in cases {
        let result = evaluate(source, DiagnosticProfile::CliBundledSettings);
        assert_eq!(result.disposition, FailureDisposition::LanguageError);
        assert_eq!(result.exit_code, 1);
        assert!(result.stdout.is_empty());
        let expected = format!(
            "–– Pkl Error ––\n{body}\nat frontend-input (file:///frontend-input.pkl, line {line})\n"
        );
        assert_eq!(result.stderr, expected.as_bytes());
    }
    for source in [
        "result = (new Dynamic { x = 1 } { y = 2 }).x\n",
        "/// first\n// bridge\n/// continuation\nresult = 1\n",
        "/// docs\n; result = 1\n",
        "/// first\n;\n/// next\nresult = 1\n",
    ] {
        let result = evaluate(source, DiagnosticProfile::CliBundledSettings);
        assert_eq!(result.phase, FrontendPhase::ModuleOutput);
        assert_eq!(result.disposition, FailureDisposition::Unsupported);
        assert_eq!(result.reason, Some("module-output-evaluation"));
        assert_eq!(result.exit_code, 2);
        assert!(result.stdout.is_empty());
    }
}

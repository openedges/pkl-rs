//! Exact 89-row Java 0.32.0 UTF-16 text/kind observations, capture-02.
//! Lab-only candidate verification; native/platform raw protocol remains separate.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::SourceProbe;

#[test]
fn complete_java_base64_matrix() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let rows = [
        (
            r########"encode-empty"########,
            r########""".base64"########,
            r########"value"########,
            r########""########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"encode-a"########,
            r########""a".base64"########,
            r########"value"########,
            r########"YQ=="########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"encode-ab"########,
            r########""ab".base64"########,
            r########"value"########,
            r########"YWI="########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"encode-abc"########,
            r########""abc".base64"########,
            r########"value"########,
            r########"YWJj"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"encode-accent"########,
            r########""é".base64"########,
            r########"value"########,
            r########"w6k="########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"encode-emoji"########,
            r########""😀".base64"########,
            r########"value"########,
            r########"8J+YgA=="########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"encode-nul"########,
            r########""\u{0000}".base64"########,
            r########"value"########,
            r########"AA=="########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"encode-high"########,
            r########""\u{d800}".base64"########,
            r########"value"########,
            r########"Pw=="########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"encode-low"########,
            r########""\u{dc00}".base64"########,
            r########"value"########,
            r########"Pw=="########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"encode-pair-literals"########,
            r########""\u{d800}\u{dc00}".base64"########,
            r########"value"########,
            r########"8JCAgA=="########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"encode-reverse-surrogates"########,
            r########""\u{dc00}\u{d800}".base64"########,
            r########"value"########,
            r########"Pz8="########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"encode-original-unicode"########,
            r########"module.originalUnicodeBase64"########,
            r########"value"########,
            r########"8J+YgPCfmIjwn5iN8J+YjvCfmKHwn6Si8J+kow=="########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"encode-original-unicode-roundtrip"########,
            r########"module.originalUnicodeRoundTrip"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"encode-original-ascii-roundtrip"########,
            r########"module.originalAsciiRoundTrip"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-empty"########,
            r########""".isBase64"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-padded1"########,
            r########""YQ==".isBase64"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-unpadded1"########,
            r########""YQ".isBase64"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-unpadded2"########,
            r########""YWI".isBase64"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-four"########,
            r########""YWJj".isBase64"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-paddedzero"########,
            r########""AA==".isBase64"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-single"########,
            r########""A".isBase64"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-single-pad"########,
            r########""A===".isBase64"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-one-equal"########,
            r########""AA=".isBase64"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-extra-padding"########,
            r########""AAAA====".isBase64"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-leading-pad"########,
            r########""=AAA".isBase64"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-trailing-byte"########,
            r########""AA==A".isBase64"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-space"########,
            r########""Y Q==".isBase64"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-newline"########,
            r########""YQ==\n".isBase64"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-crlf"########,
            r########""YQ==\r\n".isBase64"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-urlsafe-dash"########,
            r########""-___".isBase64"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-urlsafe-underscore"########,
            r########""____".isBase64"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-slash"########,
            r########""////".isBase64"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-plus"########,
            r########""++++".isBase64"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-accent"########,
            r########""é".isBase64"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-emoji"########,
            r########""😀".isBase64"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-high"########,
            r########""\u{d800}".isBase64"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"validation-nul"########,
            r########""\u{0000}".isBase64"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"decode-empty"########,
            r########""".base64Decoded"########,
            r########"value"########,
            r########""########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"decode-one-padded"########,
            r########""YQ==".base64Decoded"########,
            r########"value"########,
            r########"a"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"decode-one-unpadded"########,
            r########""YQ".base64Decoded"########,
            r########"value"########,
            r########"a"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"decode-two-unpadded"########,
            r########""YWI".base64Decoded"########,
            r########"value"########,
            r########"ab"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"decode-nul"########,
            r########""AA==".base64Decoded"########,
            r########"value"########,
            r########" "########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"decode-emoji"########,
            r########""8J+YgA==".base64Decoded"########,
            r########"value"########,
            r########"😀"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"decode-invalid-utf8-ff"########,
            r########""/w==".base64Decoded"########,
            r########"value"########,
            r########"�"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"decode-invalid-utf8-c0"########,
            r########""wA==".base64Decoded"########,
            r########"value"########,
            r########"�"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"decode-invalid-tilde"########,
            r########""~".base64Decoded"########,
            r########"error"########,
            r########"–– Pkl Error ––
Input byte[] should at least have 2 bytes for base64 bytes
String: "~"

1 | "~".base64Decoded
    ^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"decode-invalid-single"########,
            r########""A".base64Decoded"########,
            r########"error"########,
            r########"–– Pkl Error ––
Input byte[] should at least have 2 bytes for base64 bytes
String: "A"

1 | "A".base64Decoded
    ^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"decode-invalid-one-equal"########,
            r########""AA=".base64Decoded"########,
            r########"error"########,
            r########"–– Pkl Error ––
Input byte array has wrong 4-byte ending unit
String: "AA="

1 | "AA=".base64Decoded
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"decode-invalid-trailing"########,
            r########""AA==A".base64Decoded"########,
            r########"error"########,
            r########"–– Pkl Error ––
Input byte array has incorrect ending byte at 4
String: "AA==A"

1 | "AA==A".base64Decoded
    ^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"decode-invalid-space"########,
            r########""Y Q==".base64Decoded"########,
            r########"error"########,
            r########"–– Pkl Error ––
Illegal base64 character 20
String: "Y Q=="

1 | "Y Q==".base64Decoded
    ^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"decode-invalid-emoji"########,
            r########""😀".base64Decoded"########,
            r########"error"########,
            r########"–– Pkl Error ––
Input byte[] should at least have 2 bytes for base64 bytes
String: "😀"

1 | "😀".base64Decoded
    ^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"decode-invalid-accent"########,
            r########""é".base64Decoded"########,
            r########"error"########,
            r########"–– Pkl Error ––
Input byte[] should at least have 2 bytes for base64 bytes
String: "é"

1 | "é".base64Decoded
    ^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"decode-bytes-empty"########,
            r########""".base64DecodedBytes"########,
            r########"value"########,
            r########"Bytes()"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"decode-bytes-ascii"########,
            r########""YQ==".base64DecodedBytes"########,
            r########"value"########,
            r########"Bytes(97)"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"decode-bytes-sequence"########,
            r########""AQIDBA==".base64DecodedBytes"########,
            r########"value"########,
            r########"Bytes(1, 2, 3, 4)"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"decode-bytes-ff"########,
            r########""/w==".base64DecodedBytes"########,
            r########"value"########,
            r########"Bytes(255)"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"decode-bytes-slash"########,
            r########""////".base64DecodedBytes"########,
            r########"value"########,
            r########"Bytes(255, 255, 255)"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"decode-bytes-malformed-utf8"########,
            r########""wA==".base64DecodedBytes"########,
            r########"value"########,
            r########"Bytes(192)"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"decode-bytes-invalid-tilde"########,
            r########""~".base64DecodedBytes"########,
            r########"error"########,
            r########"–– Pkl Error ––
Input byte[] should at least have 2 bytes for base64 bytes
String: "~"

1 | "~".base64DecodedBytes
    ^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"decode-bytes-invalid-padding"########,
            r########""AA=".base64DecodedBytes"########,
            r########"error"########,
            r########"–– Pkl Error ––
Input byte array has wrong 4-byte ending unit
String: "AA="

1 | "AA=".base64DecodedBytes
    ^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"decode-bytes-length"########,
            r########""AQIDBA==".base64DecodedBytes.length"########,
            r########"value"########,
            r########"4"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"diagnostics-isbase64-call"########,
            r########""YQ==".isBase64()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `isBase64` in class `String`.

1 | "YQ==".isBase64()
    ^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
isBase64
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"diagnostics-base64-call"########,
            r########""abc".base64()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `base64` in class `String`.

1 | "abc".base64()
    ^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
base64
base64Decoded
base64DecodedBytes
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"diagnostics-base64-arg-throw"########,
            r########""abc".base64(throw("argument"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `base64` in class `String`.

1 | "abc".base64(throw("argument"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
base64.apply(...)
base64Decoded
base64DecodedBytes
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"diagnostics-decoded-call"########,
            r########""~".base64Decoded()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `base64Decoded` in class `String`.

1 | "~".base64Decoded()
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
base64Decoded
base64DecodedBytes
base64
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"diagnostics-decoded-arg-throw"########,
            r########""~".base64Decoded(throw("argument"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `base64Decoded` in class `String`.

1 | "~".base64Decoded(throw("argument"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
base64Decoded.apply(...)
base64DecodedBytes
base64
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"diagnostics-bytes-call"########,
            r########""YQ==".base64DecodedBytes()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `base64DecodedBytes` in class `String`.

1 | "YQ==".base64DecodedBytes()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
base64DecodedBytes
base64Decoded
base64
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"diagnostics-wrong-int"########,
            r########"1.base64"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `base64` in object of type `Int`.

1 | 1.base64
    ^^^^^^^^
at  (repl:text)

Available properties:
abs
b
ceil
d
floor
gb
gib
h
inv
isEven
isFinite
isInfinite
isNaN
isNonZero
isOdd
isPositive
kb
kib
mb
mib
min
ms
ns
pb
pib
s
sign
tb
tib
us
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"diagnostics-wrong-null"########,
            r########"null.base64"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `base64` in object of type `Null`.

1 | null.base64
    ^^^^^^^^^^^
at  (repl:text)

Available properties:
(none)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"diagnostics-wrong-list"########,
            r########"List(1).base64"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `base64` in object of type `List`.

1 | List(1).base64
    ^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
distinct
first
firstOrNull
isDistinct
isEmpty
isNotEmpty
last
lastIndex
lastOrNull
length
max
maxOrNull
min
minOrNull
rest
restOrNull
single
singleOrNull
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"diagnostics-nullsafe"########,
            r########"null?.base64"########,
            r########"value"########,
            r########"null"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"diagnostics-receiver-first"########,
            r########"throw("receiver").base64(throw("argument"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
receiver

1 | throw("receiver").base64(throw("argument"))
    ^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"constraints-valid"########,
            r########""YQ==" as String(isBase64)"########,
            r########"value"########,
            r########"YQ=="########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"constraints-invalid"########,
            r########""~" as String(isBase64)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `isBase64` violated.
Value: "~"

1 | "~" as String(isBase64)
                  ^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"constraints-encode"########,
            r########""a" as String(base64 == "YQ==")"########,
            r########"value"########,
            r########"a"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"constraints-decode"########,
            r########""YQ==" as String(base64Decoded == "a")"########,
            r########"value"########,
            r########"YQ=="########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"constraints-bytes"########,
            r########""YQ==" as String(base64DecodedBytes.length == 1)"########,
            r########"value"########,
            r########"YQ=="########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"constraints-decode-error"########,
            r########""~" as String(base64Decoded == "a")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Input byte[] should at least have 2 bytes for base64 bytes
String: "~"

1 | "~" as String(base64Decoded == "a")
                  ^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"constraints-this"########,
            r########""YQ==" as String(this.isBase64)"########,
            r########"value"########,
            r########"YQ=="########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"ref-metadata-valid-type"########,
            r########"module.refIsBase64Type"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"ref-metadata-encoded-type"########,
            r########"module.refBase64Type"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"ref-metadata-decoded-type"########,
            r########"module.refDecodedType"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"ref-metadata-bytes-type"########,
            r########"module.refDecodedBytesType"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"ref-metadata-isbase64-path"########,
            r########"module.refString.isBase64.getPath()[0].property"########,
            r########"value"########,
            r########"isBase64"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"ref-metadata-encode-path"########,
            r########"module.refString.base64.getPath()[0].property"########,
            r########"value"########,
            r########"base64"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"ref-metadata-decode-path"########,
            r########"module.refString.base64Decoded.getPath()[0].property"########,
            r########"value"########,
            r########"base64Decoded"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"ref-metadata-bytes-path"########,
            r########"module.refString.base64DecodedBytes.getPath()[0].property"########,
            r########"value"########,
            r########"base64DecodedBytes"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"ref-metadata-data"########,
            r########"module.refString.base64.getData()"########,
            r########"value"########,
            r########"null"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"ref-metadata-method"########,
            r########"module.refString.base64()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `base64` in class `pkl.ref#Reference`.

1 | module.refString.base64()
    ^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available methods in class `pkl.ref#Reference`:
getClass
getData
getDomain
getPath
getProperty
getPropertyOrNull
hasProperty
ifNonNull
toDynamic
toMap
toString
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
    ];
    let mut failures = Vec::new();
    for (id, expression, kind, expected) in rows {
        probe
            .evaluate_module_source(
                r########"file:///stage68/string-base64-exploration.pkl"########,
                Some(include_str!(
                    "../../../tests/fixtures/m2-string-base64/module.pkl"
                )),
                expression,
                0,
                None,
            )
            .unwrap();
        let mut terminal = None;
        for _ in 0..200_000 {
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
        let event = terminal.expect("Base64 matrix poll bound");
        let actual = if event.kind == "value" {
            event.value
        } else {
            event.error
        };
        if event.kind != kind || actual.as_ref() != Some(&expected) {
            failures.push(format!(
                "{id}: {expression}\nexpected {kind}: {}\nactual {}: {}",
                String::from_utf16_lossy(&expected),
                event.kind,
                actual
                    .as_ref()
                    .map(|v| String::from_utf16_lossy(v))
                    .unwrap_or_default()
            ));
        }
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

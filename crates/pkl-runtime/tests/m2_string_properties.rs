//! Complete independent Java 0.32.0 string observations, capture-02/repeat-0.
//! Authority: m2-string-properties/oracle-evidence.tar.gz.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::SourceProbe;
#[test]
fn complete_java_matrix() {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut failures = Vec::new();
    let rows = [
        (
            r########"empty"########,
            r########""".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"ascii-space"########,
            r########"" ".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"ascii-tabs"########,
            r########""\t\n\r".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"ascii-letter"########,
            r########""a".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"digit"########,
            r########""0".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"nul"########,
            r########""\u{0000}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"bell"########,
            r########""\u{0007}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"all-25"########,
            r########""\u{0009}\u{000a}\u{000b}\u{000c}\u{000d}\u{0020}\u{0085}\u{00a0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200a}\u{2028}\u{2029}\u{202f}\u{205f}\u{3000}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-0009"########,
            r########""\u{0009}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-000a"########,
            r########""\u{000a}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-000b"########,
            r########""\u{000b}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-000c"########,
            r########""\u{000c}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-000d"########,
            r########""\u{000d}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-0020"########,
            r########""\u{0020}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-0085"########,
            r########""\u{0085}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-00a0"########,
            r########""\u{00a0}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-1680"########,
            r########""\u{1680}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-2000"########,
            r########""\u{2000}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-2001"########,
            r########""\u{2001}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-2002"########,
            r########""\u{2002}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-2003"########,
            r########""\u{2003}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-2004"########,
            r########""\u{2004}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-2005"########,
            r########""\u{2005}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-2006"########,
            r########""\u{2006}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-2007"########,
            r########""\u{2007}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-2008"########,
            r########""\u{2008}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-2009"########,
            r########""\u{2009}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-200a"########,
            r########""\u{200a}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-2028"########,
            r########""\u{2028}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-2029"########,
            r########""\u{2029}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-202f"########,
            r########""\u{202f}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-205f"########,
            r########""\u{205f}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"white-3000"########,
            r########""\u{3000}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"near-0008"########,
            r########""\u{0008}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"near-000e"########,
            r########""\u{000e}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"near-001f"########,
            r########""\u{001f}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"near-007f"########,
            r########""\u{007f}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"near-0084"########,
            r########""\u{0084}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"near-0086"########,
            r########""\u{0086}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"near-009f"########,
            r########""\u{009f}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"near-00ad"########,
            r########""\u{00ad}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"near-180e"########,
            r########""\u{180e}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"near-200b"########,
            r########""\u{200b}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"near-202a"########,
            r########""\u{202a}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"near-2060"########,
            r########""\u{2060}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"near-3001"########,
            r########""\u{3001}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"near-feff"########,
            r########""\u{feff}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"leading-letter"########,
            r########""a\u{0020}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"trailing-letter"########,
            r########""\u{0020}a".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"middle-letter"########,
            r########""\u{00a0}a\u{0085}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"multi-white"########,
            r########""\u{00a0}\u{0085}\u{202f}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"space-and-nul"########,
            r########"" \u{0000}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"utf16-high-surrogate"########,
            r########""\u{d800}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"utf16-low-surrogate"########,
            r########""\u{dc00}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"utf16-pair"########,
            r########""\u{1f600}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"utf16-high-low-literals"########,
            r########""\u{d800}\u{dc00}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"utf16-low-high-literals"########,
            r########""\u{dc00}\u{d800}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"utf16-white-surrogate-white"########,
            r########""\u{0020}\u{d800}\u{00a0}".isBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"notblank-empty"########,
            r########""".isNotBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"notblank-nbsp"########,
            r########""\u{00a0}".isNotBlank"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"notblank-letter"########,
            r########""a".isNotBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"notblank-high"########,
            r########""\u{d800}".isNotBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"property-call"########,
            r########"" ".isBlank()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `isBlank` in class `String`.

1 | " ".isBlank()
    ^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
isBlank
isNotBlank
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"property-arg-throw"########,
            r########"" ".isBlank(throw("arg"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `isBlank` in class `String`.

1 | " ".isBlank(throw("arg"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
isBlank.apply(...)
isNotBlank
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"property-arg-value"########,
            r########"" ".isBlank(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `isBlank` in class `String`.

1 | " ".isBlank(1)
    ^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
isBlank.apply(...)
isNotBlank
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"nonblank-call"########,
            r########""a".isNotBlank()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `isNotBlank` in class `String`.

1 | "a".isNotBlank()
    ^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
isNotBlank
isBlank
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"missing-name"########,
            r########"" ".isBlanx"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `isBlanx` in object of type `String`.

1 | " ".isBlanx
    ^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
isBlank
isNotBlank
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"wrong-int"########,
            r########"(1).isBlank"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `isBlank` in object of type `Int`.

1 | (1).isBlank
    ^^^^^^^^^^^
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
            r########"wrong-bool"########,
            r########"true.isBlank"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `isBlank` in object of type `Boolean`.

1 | true.isBlank
    ^^^^^^^^^^^^
at  (repl:text)

Available properties:
(none)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"wrong-null"########,
            r########"null.isBlank"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `isBlank` in object of type `Null`.

1 | null.isBlank
    ^^^^^^^^^^^^
at  (repl:text)

Available properties:
(none)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"wrong-list"########,
            r########"List(" ").isBlank"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `isBlank` in object of type `List`.

1 | List(" ").isBlank
    ^^^^^^^^^^^^^^^^^
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
            r########"wrong-bytes"########,
            r########"Bytes(32).isBlank"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `isBlank` in object of type `Bytes`.

1 | Bytes(32).isBlank
    ^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
base64
hex
length
md5
sha1
sha256
sha256Int
size
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"receiver-throw"########,
            r########"throw("receiver").isBlank"########,
            r########"error"########,
            r########"–– Pkl Error ––
receiver

1 | throw("receiver").isBlank
    ^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"wrong-dynamic"########,
            r########"new Dynamic {}.isBlank"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `isBlank` in object of type `Dynamic`.

1 | new Dynamic {}.isBlank
    ^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Available properties:
default
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"original-line13"########,
            r########""\u{0009}\u{000A}\u{000B}\u{000C}\u{000D}\u{0020}\u{0085}\u{00A0}\u{1680}\u{2000}\u{2001}\u{2002}\u{2003}\u{2004}\u{2005}\u{2006}\u{2007}\u{2008}\u{2009}\u{200A}\u{2028}\u{2029}\u{202F}\u{205F}\u{3000}".isBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"module-line13"########,
            r########"module.originalLine13"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"module-emoji"########,
            r########"module.originalEmojiNonBlank"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"implicit-blank"########,
            r########"" " as String(isBlank)"########,
            r########"value"########,
            r########" "########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"implicit-notblank"########,
            r########""a" as String(isNotBlank)"########,
            r########"value"########,
            r########"a"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"implicit-blank-failure"########,
            r########""a" as String(isBlank)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `isBlank` violated.
Value: "a"

1 | "a" as String(isBlank)
                  ^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"implicit-notblank-failure"########,
            r########"" " as String(isNotBlank)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `isNotBlank` violated.
Value: " "

1 | " " as String(isNotBlank)
                  ^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"explicit-this-blank"########,
            r########"" " as String(this.isBlank)"########,
            r########"value"########,
            r########" "########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"explicit-this-notblank"########,
            r########""a" as String(this.isNotBlank)"########,
            r########"value"########,
            r########"a"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"explicit-this-failure"########,
            r########""a" as String(this.isBlank)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `this.isBlank` violated.
Value: "a"

1 | "a" as String(this.isBlank)
                  ^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"implicit-blank-class"########,
            r########"(" " as String(isBlank)).getClass().simpleName"########,
            r########"value"########,
            r########"String"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"typed-blank"########,
            r########"module.typedBlank"########,
            r########"value"########,
            r########" "########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"typed-notblank"########,
            r########"module.typedNotBlank"########,
            r########"value"########,
            r########"a"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"typed-failure"########,
            r########"module.typedFailure"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `isBlank` violated.
Value: "a"

7 | typedFailure: String(isBlank) = "a"
                         ^^^^^^^
at string-properties#typedFailure (file:///stage67/string-properties.pkl)

7 | typedFailure: String(isBlank) = "a"
                                    ^^^
at string-properties#typedFailure (file:///stage67/string-properties.pkl)

1 | module.typedFailure
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"typed-this-blank"########,
            r########"module.typedThisBlank"########,
            r########"value"########,
            r########" "########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"typed-this-failure"########,
            r########"module.typedThisFailure"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `this.isBlank` violated.
Value: "a"

9 | typedThisFailure: String(this.isBlank) = "a"
                             ^^^^^^^^^^^^
at string-properties#typedThisFailure (file:///stage67/string-properties.pkl)

9 | typedThisFailure: String(this.isBlank) = "a"
                                             ^^^
at string-properties#typedThisFailure (file:///stage67/string-properties.pkl)

1 | module.typedThisFailure
    ^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"typed-property-method"########,
            r########"module.typedBlank.isBlank(throw("argument"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `isBlank` in class `String`.

1 | module.typedBlank.isBlank(throw("argument"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
isBlank.apply(...)
isNotBlank
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"typed-failure-method"########,
            r########"module.typedFailure.isBlank(throw("argument"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `isBlank` violated.
Value: "a"

7 | typedFailure: String(isBlank) = "a"
                         ^^^^^^^
at string-properties#typedFailure (file:///stage67/string-properties.pkl)

7 | typedFailure: String(isBlank) = "a"
                                    ^^^
at string-properties#typedFailure (file:///stage67/string-properties.pkl)

1 | module.typedFailure.isBlank(throw("argument"))
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"nullsafe-blank"########,
            r########"null?.isBlank"########,
            r########"value"########,
            r########"null"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"nullsafe-notblank"########,
            r########"null?.isNotBlank"########,
            r########"value"########,
            r########"null"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"receiver-first"########,
            r########"throw("receiver").isBlank(throw("argument"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
receiver

1 | throw("receiver").isBlank(throw("argument"))
    ^^^^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"property-class"########,
            r########"" ".isBlank.getClass().simpleName"########,
            r########"value"########,
            r########"Boolean"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"notblank-property-class"########,
            r########""a".isNotBlank.getClass().simpleName"########,
            r########"value"########,
            r########"Boolean"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"ref-blank-type"########,
            r########"module.refBlankType"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"ref-notblank-type"########,
            r########"module.refNotBlankType"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"ref-blank-path"########,
            r########"module.refString.isBlank.getPath()[0].property"########,
            r########"value"########,
            r########"isBlank"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"ref-notblank-path"########,
            r########"module.refString.isNotBlank.getPath()[0].property"########,
            r########"value"########,
            r########"isNotBlank"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"ref-blank-data"########,
            r########"module.refString.isBlank.getData()"########,
            r########"value"########,
            r########"null"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"ref-blank-method"########,
            r########"module.refString.isBlank()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `isBlank` in class `pkl.ref#Reference`.

1 | module.refString.isBlank()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^
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
        (
            r########"ref-length-type"########,
            r########"module.refLengthType"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"ref-empty-type"########,
            r########"module.refEmptyType"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"ref-length-path"########,
            r########"module.refString.length.getPath()[0].property"########,
            r########"value"########,
            r########"length"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"ref-empty-path"########,
            r########"module.refString.isEmpty.getPath()[0].property"########,
            r########"value"########,
            r########"isEmpty"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"notempty-public-empty"########,
            r########""".isNotEmpty"########,
            r########"value"########,
            r########"false"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"notempty-public-space"########,
            r########"" ".isNotEmpty"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"notempty-public-ascii"########,
            r########""a".isNotEmpty"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"notempty-public-emoji"########,
            r########""😀".isNotEmpty"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"notempty-public-high"########,
            r########""\u{d800}".isNotEmpty"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"lastindex-public-empty"########,
            r########""".lastIndex"########,
            r########"value"########,
            r########"-1"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"lastindex-public-ascii"########,
            r########""a".lastIndex"########,
            r########"value"########,
            r########"0"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"lastindex-public-bmp"########,
            r########""abc".lastIndex"########,
            r########"value"########,
            r########"2"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"lastindex-public-emoji"########,
            r########""😀".lastIndex"########,
            r########"value"########,
            r########"0"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"lastindex-public-mixed"########,
            r########""a😀b".lastIndex"########,
            r########"value"########,
            r########"2"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"lastindex-public-high"########,
            r########""\u{d800}".lastIndex"########,
            r########"value"########,
            r########"0"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"lastindex-public-low"########,
            r########""\u{dc00}".lastIndex"########,
            r########"value"########,
            r########"0"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"lastindex-public-pair-literals"########,
            r########""\u{d800}\u{dc00}".lastIndex"########,
            r########"value"########,
            r########"0"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"lastindex-public-reverse-pair"########,
            r########""\u{dc00}\u{d800}".lastIndex"########,
            r########"value"########,
            r########"1"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"lastindex-public-original-emoji"########,
            r########"module.unicodeLastIndex"########,
            r########"value"########,
            r########"6"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"length-codepoints-emoji"########,
            r########""😀".length"########,
            r########"value"########,
            r########"1"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"length-codepoints-mixed"########,
            r########""a😀b".length"########,
            r########"value"########,
            r########"3"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"length-codepoints-pair-literals"########,
            r########""\u{d800}\u{dc00}".length"########,
            r########"value"########,
            r########"1"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"length-codepoints-reverse-pair"########,
            r########""\u{dc00}\u{d800}".length"########,
            r########"value"########,
            r########"2"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"property-method-notempty"########,
            r########""".isNotEmpty()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `isNotEmpty` in class `String`.

1 | "".isNotEmpty()
    ^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
isNotEmpty
isEmpty
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"property-method-lastindex"########,
            r########""".lastIndex()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `lastIndex` in class `String`.

1 | "".lastIndex()
    ^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
lastIndex
lastIndexOf(pattern)
lastIndexOfOrNull(pattern)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"property-method-length"########,
            r########""".length()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `length` in class `String`.

1 | "".length()
    ^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
length
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"property-method-lastindex-throw"########,
            r########""".lastIndex(throw("argument"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `lastIndex` in class `String`.

1 | "".lastIndex(throw("argument"))
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
lastIndex.apply(...)
lastIndexOf(pattern)
lastIndexOfOrNull(pattern)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"property-method-wrong-int"########,
            r########"1.lastIndex"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `lastIndex` in object of type `Int`.

1 | 1.lastIndex
    ^^^^^^^^^^^
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
            r########"property-method-nullsafe"########,
            r########"null?.lastIndex"########,
            r########"value"########,
            r########"null"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"implicit-properties-notempty-good"########,
            r########""x" as String(isNotEmpty)"########,
            r########"value"########,
            r########"x"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"implicit-properties-notempty-fail"########,
            r########""" as String(isNotEmpty)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `isNotEmpty` violated.
Value: ""

1 | "" as String(isNotEmpty)
                 ^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"implicit-properties-notempty-negated"########,
            r########""" as String(!isNotEmpty)"########,
            r########"value"########,
            r########""########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"implicit-properties-lastindex-good"########,
            r########""😀" as String(lastIndex == 0)"########,
            r########"value"########,
            r########"😀"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"implicit-properties-lastindex-this"########,
            r########""😀" as String(this.lastIndex == 0)"########,
            r########"value"########,
            r########"😀"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"implicit-properties-lastindex-fail"########,
            r########""😀" as String(lastIndex == 1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `lastIndex == 1` violated.
Value: "😀"

1 | "😀" as String(lastIndex == 1)
                   ^^^^^^^^^^^^^^
at  (repl:text)
"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"implicit-properties-length-emoji"########,
            r########""😀" as String(length == 1)"########,
            r########"value"########,
            r########"😀"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"implicit-properties-empty-good"########,
            r########""" as String(isEmpty)"########,
            r########"value"########,
            r########""########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"implicit-properties-empty-negated"########,
            r########""x" as String(!isEmpty)"########,
            r########"value"########,
            r########"x"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"ref-properties-notempty-type"########,
            r########"module.refNotEmptyType"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"ref-properties-lastindex-type"########,
            r########"module.refLastIndexType"########,
            r########"value"########,
            r########"true"########.encode_utf16().collect::<Vec<_>>(),
        ),
        (
            r########"ref-properties-notempty-path"########,
            r########"module.refString.isNotEmpty.getPath()[0].property"########,
            r########"value"########,
            r########"isNotEmpty"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"ref-properties-lastindex-path"########,
            r########"module.refString.lastIndex.getPath()[0].property"########,
            r########"value"########,
            r########"lastIndex"########
                .encode_utf16()
                .collect::<Vec<_>>(),
        ),
        (
            r########"ref-properties-lastindex-method"########,
            r########"module.refString.lastIndex()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `lastIndex` in class `pkl.ref#Reference`.

1 | module.refString.lastIndex()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
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
    assert_eq!(rows.len(), 146);
    for (id, expression, kind, expected) in rows {
        probe
            .evaluate_module_source(
                "file:///stage67/string-properties.pkl",
                Some(include_str!(
                    "../../../tests/fixtures/m2-string-properties/module.pkl"
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
        let event = terminal.expect("String property query poll bound");
        let actual = if event.kind == "value" {
            event.value
        } else {
            event.error
        };
        if event.kind != kind || actual.as_ref() != Some(&expected) {
            failures.push(format!(
                "{id}: {expression}\nexpected {kind}: {expected:?}\nactual {}: {}",
                event.kind,
                actual
                    .map(|v| String::from_utf16_lossy(&v))
                    .unwrap_or_default()
            ));
        }
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

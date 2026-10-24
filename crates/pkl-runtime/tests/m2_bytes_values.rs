//! Complete Java 0.32.0 string observations, including IntSeq construction.
//! Authority: tests/fixtures/m2-bytes-values/oracle-evidence.tar.gz, capture-05/repeat-0.
#[path = "support/module_keys.rs"]
mod module_keys;
use pkl_runtime::probe::{SourceEvent, SourceProbe};
fn query(probe: &mut SourceProbe, expression: &str) -> SourceEvent {
    probe
        .evaluate_module_source(
            "file:///stage60/bytes-values.pkl",
            Some(include_str!(
                "../../../tests/fixtures/m2-bytes-values/module.pkl"
            )),
            expression,
            0,
            None,
        )
        .unwrap();
    for _ in 0..200_000 {
        probe.collect();
        let event = probe.poll(0, 1).unwrap();
        probe.collect();
        if event.kind == "yield" {
            continue;
        }
        if event.kind == "module-request"
            && module_keys::complete_probe(probe, event.token.as_deref().unwrap(), 0).unwrap()
        {
            continue;
        }
        return event;
    }
    panic!("Bytes public-query poll bound")
}
fn compare_java(known_intseq: bool) {
    let mut probe = SourceProbe::new_module_evaluator("compact", false, false, false).unwrap();
    let mut mismatches = Vec::new();
    for (id, expression, kind, expected) in [
        (
            r########"empty-length"########,
            r########"emptyBytes.length"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"empty-size"########,
            r########"emptyBytes.size"########,
            r########"value"########,
            r########"0.b"########,
        ),
        (
            r########"empty-base64"########,
            r########"emptyBytes.base64"########,
            r########"value"########,
            r########""########,
        ),
        (
            r########"empty-hex"########,
            r########"emptyBytes.hex"########,
            r########"value"########,
            r########""########,
        ),
        (
            r########"empty-md5"########,
            r########"emptyBytes.md5"########,
            r########"value"########,
            r########"d41d8cd98f00b204e9800998ecf8427e"########,
        ),
        (
            r########"empty-sha1"########,
            r########"emptyBytes.sha1"########,
            r########"value"########,
            r########"da39a3ee5e6b4b0d3255bfef95601890afd80709"########,
        ),
        (
            r########"empty-sha256"########,
            r########"emptyBytes.sha256"########,
            r########"value"########,
            r########"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"########,
        ),
        (
            r########"empty-sha256int"########,
            r########"emptyBytes.sha256Int"########,
            r########"value"########,
            r########"1449310910991872227"########,
        ),
        (
            r########"single-length"########,
            r########"singleBytes.length"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"single-size"########,
            r########"singleBytes.size"########,
            r########"value"########,
            r########"1.b"########,
        ),
        (
            r########"single-base64"########,
            r########"singleBytes.base64"########,
            r########"value"########,
            r########"/w=="########,
        ),
        (
            r########"single-hex"########,
            r########"singleBytes.hex"########,
            r########"value"########,
            r########"ff"########,
        ),
        (
            r########"single-md5"########,
            r########"singleBytes.md5"########,
            r########"value"########,
            r########"00594fd4f42ba43fc1ca0427a0576295"########,
        ),
        (
            r########"single-sha1"########,
            r########"singleBytes.sha1"########,
            r########"value"########,
            r########"85e53271e14006f0265921d02d4d736cdc580b0b"########,
        ),
        (
            r########"single-sha256"########,
            r########"singleBytes.sha256"########,
            r########"value"########,
            r########"a8100ae6aa1940d0b663bb31cd466142ebbdbd5187131b92d93818987832eb89"########,
        ),
        (
            r########"single-sha256int"########,
            r########"singleBytes.sha256Int"########,
            r########"value"########,
            r########"-3440721893516504920"########,
        ),
        (
            r########"ascii-length"########,
            r########"asciiBytes.length"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"ascii-size"########,
            r########"asciiBytes.size"########,
            r########"value"########,
            r########"3.b"########,
        ),
        (
            r########"ascii-base64"########,
            r########"asciiBytes.base64"########,
            r########"value"########,
            r########"QUJD"########,
        ),
        (
            r########"ascii-hex"########,
            r########"asciiBytes.hex"########,
            r########"value"########,
            r########"414243"########,
        ),
        (
            r########"ascii-md5"########,
            r########"asciiBytes.md5"########,
            r########"value"########,
            r########"902fbdd2b1df0c4f70b4a5d23525e932"########,
        ),
        (
            r########"ascii-sha1"########,
            r########"asciiBytes.sha1"########,
            r########"value"########,
            r########"3c01bdbb26f358bab27f267924aa2c9a03fcfdb8"########,
        ),
        (
            r########"ascii-sha256"########,
            r########"asciiBytes.sha256"########,
            r########"value"########,
            r########"b5d4045c3f466fa91fe2cc6abe79232a1a57cdf104f7a26e716e0a1e2789df78"########,
        ),
        (
            r########"ascii-sha256int"########,
            r########"asciiBytes.sha256Int"########,
            r########"value"########,
            r########"-6237689720944143179"########,
        ),
        (
            r########"boundary-length"########,
            r########"boundaryBytes.length"########,
            r########"value"########,
            r########"4"########,
        ),
        (
            r########"boundary-size"########,
            r########"boundaryBytes.size"########,
            r########"value"########,
            r########"4.b"########,
        ),
        (
            r########"boundary-base64"########,
            r########"boundaryBytes.base64"########,
            r########"value"########,
            r########"AH+A/w=="########,
        ),
        (
            r########"boundary-hex"########,
            r########"boundaryBytes.hex"########,
            r########"value"########,
            r########"007f80ff"########,
        ),
        (
            r########"boundary-md5"########,
            r########"boundaryBytes.md5"########,
            r########"value"########,
            r########"55a963d22f883121a91d36df2645ed04"########,
        ),
        (
            r########"boundary-sha1"########,
            r########"boundaryBytes.sha1"########,
            r########"value"########,
            r########"77648de335db61221e1a03ea826693a174ff0cf0"########,
        ),
        (
            r########"boundary-sha256"########,
            r########"boundaryBytes.sha256"########,
            r########"value"########,
            r########"89273d2f70b93285bb7ddb4bcee86a5347ca7159352e3cbdd20c23e9d1e507d3"########,
        ),
        (
            r########"boundary-sha256int"########,
            r########"boundaryBytes.sha256Int"########,
            r########"value"########,
            r########"-8848806426349590647"########,
        ),
        (
            r########"all-length"########,
            r########"allBytes.length"########,
            r########"value"########,
            r########"256"########,
        ),
        (
            r########"all-size"########,
            r########"allBytes.size"########,
            r########"value"########,
            r########"256.b"########,
        ),
        (
            r########"all-base64"########,
            r########"allBytes.base64"########,
            r########"value"########,
            r########"AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8gISIjJCUmJygpKissLS4vMDEyMzQ1Njc4OTo7PD0+P0BBQkNERUZHSElKS0xNTk9QUVJTVFVWV1hZWltcXV5fYGFiY2RlZmdoaWprbG1ub3BxcnN0dXZ3eHl6e3x9fn+AgYKDhIWGh4iJiouMjY6PkJGSk5SVlpeYmZqbnJ2en6ChoqOkpaanqKmqq6ytrq+wsbKztLW2t7i5uru8vb6/wMHCw8TFxsfIycrLzM3Oz9DR0tPU1dbX2Nna29zd3t/g4eLj5OXm5+jp6uvs7e7v8PHy8/T19vf4+fr7/P3+/w=="########,
        ),
        (
            r########"all-hex"########,
            r########"allBytes.hex"########,
            r########"value"########,
            r########"000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f303132333435363738393a3b3c3d3e3f404142434445464748494a4b4c4d4e4f505152535455565758595a5b5c5d5e5f606162636465666768696a6b6c6d6e6f707172737475767778797a7b7c7d7e7f808182838485868788898a8b8c8d8e8f909192939495969798999a9b9c9d9e9fa0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7b8b9babbbcbdbebfc0c1c2c3c4c5c6c7c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedfe0e1e2e3e4e5e6e7e8e9eaebecedeeeff0f1f2f3f4f5f6f7f8f9fafbfcfdfeff"########,
        ),
        (
            r########"all-md5"########,
            r########"allBytes.md5"########,
            r########"value"########,
            r########"e2c865db4162bed963bfaa9ef6ac18f0"########,
        ),
        (
            r########"all-sha1"########,
            r########"allBytes.sha1"########,
            r########"value"########,
            r########"4916d6bdb7f78e6803698cab32d1586ea457dfc8"########,
        ),
        (
            r########"all-sha256"########,
            r########"allBytes.sha256"########,
            r########"value"########,
            r########"40aff2e9d2d8922e47afd4648e6967497158785fbd1da870e7110266bf944880"########,
        ),
        (
            r########"all-sha256int"########,
            r########"allBytes.sha256Int"########,
            r########"value"########,
            r########"3355983072724168512"########,
        ),
        (
            r########"all-index-0"########,
            r########"allBytes[0]"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"all-index-1"########,
            r########"allBytes[1]"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"all-index-2"########,
            r########"allBytes[2]"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"all-index-3"########,
            r########"allBytes[3]"########,
            r########"value"########,
            r########"3"########,
        ),
        (
            r########"all-index-4"########,
            r########"allBytes[4]"########,
            r########"value"########,
            r########"4"########,
        ),
        (
            r########"all-index-15"########,
            r########"allBytes[15]"########,
            r########"value"########,
            r########"15"########,
        ),
        (
            r########"all-index-16"########,
            r########"allBytes[16]"########,
            r########"value"########,
            r########"16"########,
        ),
        (
            r########"all-index-31"########,
            r########"allBytes[31]"########,
            r########"value"########,
            r########"31"########,
        ),
        (
            r########"all-index-32"########,
            r########"allBytes[32]"########,
            r########"value"########,
            r########"32"########,
        ),
        (
            r########"all-index-63"########,
            r########"allBytes[63]"########,
            r########"value"########,
            r########"63"########,
        ),
        (
            r########"all-index-64"########,
            r########"allBytes[64]"########,
            r########"value"########,
            r########"64"########,
        ),
        (
            r########"all-index-126"########,
            r########"allBytes[126]"########,
            r########"value"########,
            r########"126"########,
        ),
        (
            r########"all-index-127"########,
            r########"allBytes[127]"########,
            r########"value"########,
            r########"127"########,
        ),
        (
            r########"all-index-128"########,
            r########"allBytes[128]"########,
            r########"value"########,
            r########"-128"########,
        ),
        (
            r########"all-index-129"########,
            r########"allBytes[129]"########,
            r########"value"########,
            r########"-127"########,
        ),
        (
            r########"all-index-254"########,
            r########"allBytes[254]"########,
            r########"value"########,
            r########"-2"########,
        ),
        (
            r########"all-index-255"########,
            r########"allBytes[255]"########,
            r########"value"########,
            r########"-1"########,
        ),
        (
            r########"boundary-get-neg2"########,
            r########"boundaryBytes.getOrNull(-2)"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"boundary-get-neg1"########,
            r########"boundaryBytes.getOrNull(-1)"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"boundary-get-0"########,
            r########"boundaryBytes.getOrNull(0)"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"boundary-get-1"########,
            r########"boundaryBytes.getOrNull(1)"########,
            r########"value"########,
            r########"127"########,
        ),
        (
            r########"boundary-get-2"########,
            r########"boundaryBytes.getOrNull(2)"########,
            r########"value"########,
            r########"-128"########,
        ),
        (
            r########"boundary-get-3"########,
            r########"boundaryBytes.getOrNull(3)"########,
            r########"value"########,
            r########"-1"########,
        ),
        (
            r########"boundary-get-4"########,
            r########"boundaryBytes.getOrNull(4)"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"boundary-get-5"########,
            r########"boundaryBytes.getOrNull(5)"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"boundary-get-127"########,
            r########"boundaryBytes.getOrNull(127)"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"boundary-get-128"########,
            r########"boundaryBytes.getOrNull(128)"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"boundary-get-254"########,
            r########"boundaryBytes.getOrNull(254)"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"boundary-get-255"########,
            r########"boundaryBytes.getOrNull(255)"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"boundary-get-256"########,
            r########"boundaryBytes.getOrNull(256)"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"boundary-get-9223372036854775807"########,
            r########"boundaryBytes.getOrNull(9223372036854775807)"########,
            r########"value"########,
            r########"null"########,
        ),
        (
            r########"boundary-sub-neg1"########,
            r########"boundaryBytes[-1]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Element index `-1` is out of range `0`..`3`.
Value: Bytes(0, 127, 128, 255)

1 | boundaryBytes[-1]
    ^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"boundary-sub-0"########,
            r########"boundaryBytes[0]"########,
            r########"value"########,
            r########"0"########,
        ),
        (
            r########"boundary-sub-1"########,
            r########"boundaryBytes[1]"########,
            r########"value"########,
            r########"127"########,
        ),
        (
            r########"boundary-sub-2"########,
            r########"boundaryBytes[2]"########,
            r########"value"########,
            r########"-128"########,
        ),
        (
            r########"boundary-sub-3"########,
            r########"boundaryBytes[3]"########,
            r########"value"########,
            r########"-1"########,
        ),
        (
            r########"boundary-sub-4"########,
            r########"boundaryBytes[4]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Element index `4` is out of range `0`..`3`.
Value: Bytes(0, 127, 128, 255)

1 | boundaryBytes[4]
    ^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"boundary-sub-255"########,
            r########"boundaryBytes[255]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Element index `255` is out of range `0`..`3`.
Value: Bytes(0, 127, 128, 255)

1 | boundaryBytes[255]
    ^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"boundary-sub-256"########,
            r########"boundaryBytes[256]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Element index `256` is out of range `0`..`3`.
Value: Bytes(0, 127, 128, 255)

1 | boundaryBytes[256]
    ^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-empty"########,
            r########"Bytes()"########,
            r########"value"########,
            r########"Bytes()"########,
        ),
        (
            r########"ctor-zero"########,
            r########"Bytes(0)"########,
            r########"value"########,
            r########"Bytes(0)"########,
        ),
        (
            r########"ctor-max-signed"########,
            r########"Bytes(127)"########,
            r########"value"########,
            r########"Bytes(127)"########,
        ),
        (
            r########"ctor-first-high"########,
            r########"Bytes(128)"########,
            r########"value"########,
            r########"Bytes(128)"########,
        ),
        (
            r########"ctor-last"########,
            r########"Bytes(255)"########,
            r########"value"########,
            r########"Bytes(255)"########,
        ),
        (
            r########"ctor-two"########,
            r########"Bytes(0, 255)"########,
            r########"value"########,
            r########"Bytes(0, 255)"########,
        ),
        (
            r########"ctor-hex"########,
            r########"Bytes(0x0, 0xff)"########,
            r########"value"########,
            r########"Bytes(0, 255)"########,
        ),
        (
            r########"ctor-octal"########,
            r########"Bytes(0o0, 0o377)"########,
            r########"value"########,
            r########"Bytes(0, 255)"########,
        ),
        (
            r########"ctor-binary"########,
            r########"Bytes(0b0, 0b11111111)"########,
            r########"value"########,
            r########"Bytes(0, 255)"########,
        ),
        (
            r########"ctor-triple"########,
            r########"Bytes(1, 2, 3)"########,
            r########"value"########,
            r########"Bytes(1, 2, 3)"########,
        ),
        (
            r########"ctor-negative"########,
            r########"Bytes(-1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `isBetween(0, 255)` violated.
Value: -1

    isBetween(0, 255)
    │
    false

1109 | typealias UInt8 = Int(isBetween(0, 255))
                             ^^^^^^^^^^^^^^^^^
at  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | Bytes(-1)
           ^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-too-large"########,
            r########"Bytes(256)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `isBetween(0, 255)` violated.
Value: 256

    isBetween(0, 255)
    │
    false

1109 | typealias UInt8 = Int(isBetween(0, 255))
                             ^^^^^^^^^^^^^^^^^
at  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | Bytes(256)
          ^^^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-mid-negative"########,
            r########"Bytes(1, -1, 2)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `isBetween(0, 255)` violated.
Value: -1

    isBetween(0, 255)
    │
    false

1109 | typealias UInt8 = Int(isBetween(0, 255))
                             ^^^^^^^^^^^^^^^^^
at  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | Bytes(1, -1, 2)
              ^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-mid-large"########,
            r########"Bytes(1, 256, 2)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `isBetween(0, 255)` violated.
Value: 256

    isBetween(0, 255)
    │
    false

1109 | typealias UInt8 = Int(isBetween(0, 255))
                             ^^^^^^^^^^^^^^^^^
at  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | Bytes(1, 256, 2)
             ^^^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-first-large"########,
            r########"Bytes(1, 256, -1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `isBetween(0, 255)` violated.
Value: 256

    isBetween(0, 255)
    │
    false

1109 | typealias UInt8 = Int(isBetween(0, 255))
                             ^^^^^^^^^^^^^^^^^
at  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | Bytes(1, 256, -1)
             ^^^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-first-negative"########,
            r########"Bytes(1, -1, 256)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `isBetween(0, 255)` violated.
Value: -1

    isBetween(0, 255)
    │
    false

1109 | typealias UInt8 = Int(isBetween(0, 255))
                             ^^^^^^^^^^^^^^^^^
at  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | Bytes(1, -1, 256)
              ^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-float"########,
            r########"Bytes(1.0)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got type `Float`.
Value: 1.0

1109 | typealias UInt8 = Int(isBetween(0, 255))
                         ^^^
at  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | Bytes(1.0)
          ^^^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-string"########,
            r########"Bytes("1")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got type `String`.
Value: "1"

1109 | typealias UInt8 = Int(isBetween(0, 255))
                         ^^^
at  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | Bytes("1")
          ^^^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-null"########,
            r########"Bytes(null)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got `null`.

1109 | typealias UInt8 = Int(isBetween(0, 255))
                         ^^^
at  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | Bytes(null)
          ^^^^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-bool"########,
            r########"Bytes(true)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got type `Boolean`.
Value: true

1109 | typealias UInt8 = Int(isBetween(0, 255))
                         ^^^
at  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | Bytes(true)
          ^^^^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-second-string"########,
            r########"Bytes(1, "x")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got type `String`.
Value: "x"

1109 | typealias UInt8 = Int(isBetween(0, 255))
                         ^^^
at  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | Bytes(1, "x")
             ^^^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-second-float"########,
            r########"Bytes(1, 2.0)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got type `Float`.
Value: 2.0

1109 | typealias UInt8 = Int(isBetween(0, 255))
                         ^^^
at  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | Bytes(1, 2.0)
             ^^^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-throw-second"########,
            r########"Bytes(1, throw("second"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
second

1 | Bytes(1, throw("second"))
             ^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-range-before-throw"########,
            r########"Bytes(256, throw("second"))"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `isBetween(0, 255)` violated.
Value: 256

    isBetween(0, 255)
    │
    false

1109 | typealias UInt8 = Int(isBetween(0, 255))
                             ^^^^^^^^^^^^^^^^^
at  (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | Bytes(256, throw("second"))
          ^^^
at  (repl:text)
"########,
        ),
        (
            r########"ctor-throw-before-range"########,
            r########"Bytes(throw("first"),256)"########,
            r########"error"########,
            r########"–– Pkl Error ––
first

1 | Bytes(throw("first"),256)
          ^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"enc-empty-utf8"########,
            r########""".encodeToBytes("UTF-8").hex"########,
            r########"value"########,
            r########""########,
        ),
        (
            r########"round-empty-utf8"########,
            r########""".encodeToBytes("UTF-8").decodeToString("UTF-8")"########,
            r########"value"########,
            r########""########,
        ),
        (
            r########"enc-empty-utf16"########,
            r########""".encodeToBytes("UTF-16").hex"########,
            r########"value"########,
            r########""########,
        ),
        (
            r########"round-empty-utf16"########,
            r########""".encodeToBytes("UTF-16").decodeToString("UTF-16")"########,
            r########"value"########,
            r########""########,
        ),
        (
            r########"enc-empty-iso88591"########,
            r########""".encodeToBytes("ISO-8859-1").hex"########,
            r########"value"########,
            r########""########,
        ),
        (
            r########"round-empty-iso88591"########,
            r########""".encodeToBytes("ISO-8859-1").decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########""########,
        ),
        (
            r########"enc-ascii-utf8"########,
            r########""abc".encodeToBytes("UTF-8").hex"########,
            r########"value"########,
            r########"616263"########,
        ),
        (
            r########"round-ascii-utf8"########,
            r########""abc".encodeToBytes("UTF-8").decodeToString("UTF-8")"########,
            r########"value"########,
            r########"abc"########,
        ),
        (
            r########"enc-ascii-utf16"########,
            r########""abc".encodeToBytes("UTF-16").hex"########,
            r########"value"########,
            r########"feff006100620063"########,
        ),
        (
            r########"round-ascii-utf16"########,
            r########""abc".encodeToBytes("UTF-16").decodeToString("UTF-16")"########,
            r########"value"########,
            r########"abc"########,
        ),
        (
            r########"enc-ascii-iso88591"########,
            r########""abc".encodeToBytes("ISO-8859-1").hex"########,
            r########"value"########,
            r########"616263"########,
        ),
        (
            r########"round-ascii-iso88591"########,
            r########""abc".encodeToBytes("ISO-8859-1").decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"abc"########,
        ),
        (
            r########"enc-latin-utf8"########,
            r########""é".encodeToBytes("UTF-8").hex"########,
            r########"value"########,
            r########"c3a9"########,
        ),
        (
            r########"round-latin-utf8"########,
            r########""é".encodeToBytes("UTF-8").decodeToString("UTF-8")"########,
            r########"value"########,
            r########"é"########,
        ),
        (
            r########"enc-latin-utf16"########,
            r########""é".encodeToBytes("UTF-16").hex"########,
            r########"value"########,
            r########"feff00e9"########,
        ),
        (
            r########"round-latin-utf16"########,
            r########""é".encodeToBytes("UTF-16").decodeToString("UTF-16")"########,
            r########"value"########,
            r########"é"########,
        ),
        (
            r########"enc-latin-iso88591"########,
            r########""é".encodeToBytes("ISO-8859-1").hex"########,
            r########"value"########,
            r########"e9"########,
        ),
        (
            r########"round-latin-iso88591"########,
            r########""é".encodeToBytes("ISO-8859-1").decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"é"########,
        ),
        (
            r########"enc-euro-utf8"########,
            r########""€".encodeToBytes("UTF-8").hex"########,
            r########"value"########,
            r########"e282ac"########,
        ),
        (
            r########"round-euro-utf8"########,
            r########""€".encodeToBytes("UTF-8").decodeToString("UTF-8")"########,
            r########"value"########,
            r########"€"########,
        ),
        (
            r########"enc-euro-utf16"########,
            r########""€".encodeToBytes("UTF-16").hex"########,
            r########"value"########,
            r########"feff20ac"########,
        ),
        (
            r########"round-euro-utf16"########,
            r########""€".encodeToBytes("UTF-16").decodeToString("UTF-16")"########,
            r########"value"########,
            r########"€"########,
        ),
        (
            r########"enc-euro-iso88591"########,
            r########""€".encodeToBytes("ISO-8859-1").hex"########,
            r########"value"########,
            r########"3f"########,
        ),
        (
            r########"round-euro-iso88591"########,
            r########""€".encodeToBytes("ISO-8859-1").decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"?"########,
        ),
        (
            r########"enc-emoji-utf8"########,
            r########""😀".encodeToBytes("UTF-8").hex"########,
            r########"value"########,
            r########"f09f9880"########,
        ),
        (
            r########"round-emoji-utf8"########,
            r########""😀".encodeToBytes("UTF-8").decodeToString("UTF-8")"########,
            r########"value"########,
            r########"😀"########,
        ),
        (
            r########"enc-emoji-utf16"########,
            r########""😀".encodeToBytes("UTF-16").hex"########,
            r########"value"########,
            r########"feffd83dde00"########,
        ),
        (
            r########"round-emoji-utf16"########,
            r########""😀".encodeToBytes("UTF-16").decodeToString("UTF-16")"########,
            r########"value"########,
            r########"😀"########,
        ),
        (
            r########"enc-emoji-iso88591"########,
            r########""😀".encodeToBytes("ISO-8859-1").hex"########,
            r########"value"########,
            r########"3f"########,
        ),
        (
            r########"round-emoji-iso88591"########,
            r########""😀".encodeToBytes("ISO-8859-1").decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"?"########,
        ),
        (
            r########"enc-combining-utf8"########,
            r########""á".encodeToBytes("UTF-8").hex"########,
            r########"value"########,
            r########"61cc81"########,
        ),
        (
            r########"round-combining-utf8"########,
            r########""á".encodeToBytes("UTF-8").decodeToString("UTF-8")"########,
            r########"value"########,
            r########"á"########,
        ),
        (
            r########"enc-combining-utf16"########,
            r########""á".encodeToBytes("UTF-16").hex"########,
            r########"value"########,
            r########"feff00610301"########,
        ),
        (
            r########"round-combining-utf16"########,
            r########""á".encodeToBytes("UTF-16").decodeToString("UTF-16")"########,
            r########"value"########,
            r########"á"########,
        ),
        (
            r########"enc-combining-iso88591"########,
            r########""á".encodeToBytes("ISO-8859-1").hex"########,
            r########"value"########,
            r########"613f"########,
        ),
        (
            r########"round-combining-iso88591"########,
            r########""á".encodeToBytes("ISO-8859-1").decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"a?"########,
        ),
        (
            r########"enc-snow-utf8"########,
            r########""☃".encodeToBytes("UTF-8").hex"########,
            r########"value"########,
            r########"e29883"########,
        ),
        (
            r########"round-snow-utf8"########,
            r########""☃".encodeToBytes("UTF-8").decodeToString("UTF-8")"########,
            r########"value"########,
            r########"☃"########,
        ),
        (
            r########"enc-snow-utf16"########,
            r########""☃".encodeToBytes("UTF-16").hex"########,
            r########"value"########,
            r########"feff2603"########,
        ),
        (
            r########"round-snow-utf16"########,
            r########""☃".encodeToBytes("UTF-16").decodeToString("UTF-16")"########,
            r########"value"########,
            r########"☃"########,
        ),
        (
            r########"enc-snow-iso88591"########,
            r########""☃".encodeToBytes("ISO-8859-1").hex"########,
            r########"value"########,
            r########"3f"########,
        ),
        (
            r########"round-snow-iso88591"########,
            r########""☃".encodeToBytes("ISO-8859-1").decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"?"########,
        ),
        (
            r########"enc-bom-char-utf8"########,
            r########""﻿A".encodeToBytes("UTF-8").hex"########,
            r########"value"########,
            r########"efbbbf41"########,
        ),
        (
            r########"round-bom-char-utf8"########,
            r########""﻿A".encodeToBytes("UTF-8").decodeToString("UTF-8")"########,
            r########"value"########,
            r########"﻿A"########,
        ),
        (
            r########"enc-bom-char-utf16"########,
            r########""﻿A".encodeToBytes("UTF-16").hex"########,
            r########"value"########,
            r########"fefffeff0041"########,
        ),
        (
            r########"round-bom-char-utf16"########,
            r########""﻿A".encodeToBytes("UTF-16").decodeToString("UTF-16")"########,
            r########"value"########,
            r########"﻿A"########,
        ),
        (
            r########"enc-bom-char-iso88591"########,
            r########""﻿A".encodeToBytes("ISO-8859-1").hex"########,
            r########"value"########,
            r########"3f41"########,
        ),
        (
            r########"round-bom-char-iso88591"########,
            r########""﻿A".encodeToBytes("ISO-8859-1").decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"?A"########,
        ),
        (
            r########"enc-nul-utf8"########,
            r########""A\u{0}B".encodeToBytes("UTF-8").hex"########,
            r########"value"########,
            r########"410042"########,
        ),
        (
            r########"round-nul-utf8"########,
            r########""A\u{0}B".encodeToBytes("UTF-8").decodeToString("UTF-8")"########,
            r########"value"########,
            r########"A B"########,
        ),
        (
            r########"enc-nul-utf16"########,
            r########""A\u{0}B".encodeToBytes("UTF-16").hex"########,
            r########"value"########,
            r########"feff004100000042"########,
        ),
        (
            r########"round-nul-utf16"########,
            r########""A\u{0}B".encodeToBytes("UTF-16").decodeToString("UTF-16")"########,
            r########"value"########,
            r########"A B"########,
        ),
        (
            r########"enc-nul-iso88591"########,
            r########""A\u{0}B".encodeToBytes("ISO-8859-1").hex"########,
            r########"value"########,
            r########"410042"########,
        ),
        (
            r########"round-nul-iso88591"########,
            r########""A\u{0}B".encodeToBytes("ISO-8859-1").decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"A B"########,
        ),
        (
            r########"dec-empty-utf8"########,
            r########"Bytes().decodeToString("UTF-8")"########,
            r########"value"########,
            r########""########,
        ),
        (
            r########"dec-empty-utf16"########,
            r########"Bytes().decodeToString("UTF-16")"########,
            r########"value"########,
            r########""########,
        ),
        (
            r########"dec-empty-iso88591"########,
            r########"Bytes().decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########""########,
        ),
        (
            r########"dec-ascii-utf8"########,
            r########"Bytes(65, 66, 67).decodeToString("UTF-8")"########,
            r########"value"########,
            r########"ABC"########,
        ),
        (
            r########"dec-ascii-utf16"########,
            r########"Bytes(65, 66, 67).decodeToString("UTF-16")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-16".

1 | Bytes(65, 66, 67).decodeToString("UTF-16")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-ascii-iso88591"########,
            r########"Bytes(65, 66, 67).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"ABC"########,
        ),
        (
            r########"dec-utf8-bom-utf8"########,
            r########"Bytes(239, 187, 191, 65).decodeToString("UTF-8")"########,
            r########"value"########,
            r########"﻿A"########,
        ),
        (
            r########"dec-utf8-bom-utf16"########,
            r########"Bytes(239, 187, 191, 65).decodeToString("UTF-16")"########,
            r########"value"########,
            r########"뽁"########,
        ),
        (
            r########"dec-utf8-bom-iso88591"########,
            r########"Bytes(239, 187, 191, 65).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"ï»¿A"########,
        ),
        (
            r########"dec-utf8-two-utf8"########,
            r########"Bytes(194, 162).decodeToString("UTF-8")"########,
            r########"value"########,
            r########"¢"########,
        ),
        (
            r########"dec-utf8-two-utf16"########,
            r########"Bytes(194, 162).decodeToString("UTF-16")"########,
            r########"value"########,
            r########"슢"########,
        ),
        (
            r########"dec-utf8-two-iso88591"########,
            r########"Bytes(194, 162).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"Â¢"########,
        ),
        (
            r########"dec-utf8-three-utf8"########,
            r########"Bytes(226, 130, 172).decodeToString("UTF-8")"########,
            r########"value"########,
            r########"€"########,
        ),
        (
            r########"dec-utf8-three-utf16"########,
            r########"Bytes(226, 130, 172).decodeToString("UTF-16")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-16".

1 | Bytes(226, 130, 172).decodeToString("UTF-16")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf8-three-iso88591"########,
            r########"Bytes(226, 130, 172).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"â¬"########,
        ),
        (
            r########"dec-utf8-four-utf8"########,
            r########"Bytes(240, 159, 152, 128).decodeToString("UTF-8")"########,
            r########"value"########,
            r########"😀"########,
        ),
        (
            r########"dec-utf8-four-utf16"########,
            r########"Bytes(240, 159, 152, 128).decodeToString("UTF-16")"########,
            r########"value"########,
            r########"颀"########,
        ),
        (
            r########"dec-utf8-four-iso88591"########,
            r########"Bytes(240, 159, 152, 128).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"ð"########,
        ),
        (
            r########"dec-utf8-overlong-utf8"########,
            r########"Bytes(192, 175).decodeToString("UTF-8")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-8".

1 | Bytes(192, 175).decodeToString("UTF-8")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf8-overlong-utf16"########,
            r########"Bytes(192, 175).decodeToString("UTF-16")"########,
            r########"value"########,
            r########"삯"########,
        ),
        (
            r########"dec-utf8-overlong-iso88591"########,
            r########"Bytes(192, 175).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"À¯"########,
        ),
        (
            r########"dec-utf8-surrogate-utf8"########,
            r########"Bytes(237, 160, 128).decodeToString("UTF-8")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-8".

1 | Bytes(237, 160, 128).decodeToString("UTF-8")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf8-surrogate-utf16"########,
            r########"Bytes(237, 160, 128).decodeToString("UTF-16")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-16".

1 | Bytes(237, 160, 128).decodeToString("UTF-16")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf8-surrogate-iso88591"########,
            r########"Bytes(237, 160, 128).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"í "########,
        ),
        (
            r########"dec-utf8-out-of-range-utf8"########,
            r########"Bytes(244, 144, 128, 128).decodeToString("UTF-8")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-8".

1 | Bytes(244, 144, 128, 128).decodeToString("UTF-8")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf8-out-of-range-utf16"########,
            r########"Bytes(244, 144, 128, 128).decodeToString("UTF-16")"########,
            r########"value"########,
            r########"肀"########,
        ),
        (
            r########"dec-utf8-out-of-range-iso88591"########,
            r########"Bytes(244, 144, 128, 128).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"ô"########,
        ),
        (
            r########"dec-utf8-truncated-2-utf8"########,
            r########"Bytes(194).decodeToString("UTF-8")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-8".

1 | Bytes(194).decodeToString("UTF-8")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf8-truncated-2-utf16"########,
            r########"Bytes(194).decodeToString("UTF-16")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-16".

1 | Bytes(194).decodeToString("UTF-16")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf8-truncated-2-iso88591"########,
            r########"Bytes(194).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"Â"########,
        ),
        (
            r########"dec-utf8-truncated-3-utf8"########,
            r########"Bytes(226, 130).decodeToString("UTF-8")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-8".

1 | Bytes(226, 130).decodeToString("UTF-8")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf8-truncated-3-utf16"########,
            r########"Bytes(226, 130).decodeToString("UTF-16")"########,
            r########"value"########,
            r########""########,
        ),
        (
            r########"dec-utf8-truncated-3-iso88591"########,
            r########"Bytes(226, 130).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"â"########,
        ),
        (
            r########"dec-utf8-truncated-4-utf8"########,
            r########"Bytes(240, 159, 152).decodeToString("UTF-8")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-8".

1 | Bytes(240, 159, 152).decodeToString("UTF-8")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf8-truncated-4-utf16"########,
            r########"Bytes(240, 159, 152).decodeToString("UTF-16")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-16".

1 | Bytes(240, 159, 152).decodeToString("UTF-16")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf8-truncated-4-iso88591"########,
            r########"Bytes(240, 159, 152).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"ð"########,
        ),
        (
            r########"dec-utf8-continuation-utf8"########,
            r########"Bytes(128).decodeToString("UTF-8")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-8".

1 | Bytes(128).decodeToString("UTF-8")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf8-continuation-utf16"########,
            r########"Bytes(128).decodeToString("UTF-16")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-16".

1 | Bytes(128).decodeToString("UTF-16")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf8-continuation-iso88591"########,
            r########"Bytes(128).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########""########,
        ),
        (
            r########"dec-utf8-invalid-after-valid-utf8"########,
            r########"Bytes(65, 255, 66).decodeToString("UTF-8")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-8".

1 | Bytes(65, 255, 66).decodeToString("UTF-8")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf8-invalid-after-valid-utf16"########,
            r########"Bytes(65, 255, 66).decodeToString("UTF-16")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-16".

1 | Bytes(65, 255, 66).decodeToString("UTF-16")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf8-invalid-after-valid-iso88591"########,
            r########"Bytes(65, 255, 66).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"AÿB"########,
        ),
        (
            r########"dec-utf16-be-bom-utf8"########,
            r########"Bytes(254, 255, 0, 65).decodeToString("UTF-8")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-8".

1 | Bytes(254, 255, 0, 65).decodeToString("UTF-8")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf16-be-bom-utf16"########,
            r########"Bytes(254, 255, 0, 65).decodeToString("UTF-16")"########,
            r########"value"########,
            r########"A"########,
        ),
        (
            r########"dec-utf16-be-bom-iso88591"########,
            r########"Bytes(254, 255, 0, 65).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"þÿ A"########,
        ),
        (
            r########"dec-utf16-le-bom-utf8"########,
            r########"Bytes(255, 254, 65, 0).decodeToString("UTF-8")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-8".

1 | Bytes(255, 254, 65, 0).decodeToString("UTF-8")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf16-le-bom-utf16"########,
            r########"Bytes(255, 254, 65, 0).decodeToString("UTF-16")"########,
            r########"value"########,
            r########"A"########,
        ),
        (
            r########"dec-utf16-le-bom-iso88591"########,
            r########"Bytes(255, 254, 65, 0).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"ÿþA "########,
        ),
        (
            r########"dec-utf16-no-bom-utf8"########,
            r########"Bytes(0, 65).decodeToString("UTF-8")"########,
            r########"value"########,
            r########" A"########,
        ),
        (
            r########"dec-utf16-no-bom-utf16"########,
            r########"Bytes(0, 65).decodeToString("UTF-16")"########,
            r########"value"########,
            r########"A"########,
        ),
        (
            r########"dec-utf16-no-bom-iso88591"########,
            r########"Bytes(0, 65).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########" A"########,
        ),
        (
            r########"dec-utf16-odd-utf8"########,
            r########"Bytes(0, 65, 0).decodeToString("UTF-8")"########,
            r########"value"########,
            r########" A "########,
        ),
        (
            r########"dec-utf16-odd-utf16"########,
            r########"Bytes(0, 65, 0).decodeToString("UTF-16")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-16".

1 | Bytes(0, 65, 0).decodeToString("UTF-16")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf16-odd-iso88591"########,
            r########"Bytes(0, 65, 0).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########" A "########,
        ),
        (
            r########"dec-utf16-high-surrogate-utf8"########,
            r########"Bytes(216, 0).decodeToString("UTF-8")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-8".

1 | Bytes(216, 0).decodeToString("UTF-8")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf16-high-surrogate-utf16"########,
            r########"Bytes(216, 0).decodeToString("UTF-16")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-16".

1 | Bytes(216, 0).decodeToString("UTF-16")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf16-high-surrogate-iso88591"########,
            r########"Bytes(216, 0).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"Ø "########,
        ),
        (
            r########"dec-utf16-low-surrogate-utf8"########,
            r########"Bytes(220, 0).decodeToString("UTF-8")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-8".

1 | Bytes(220, 0).decodeToString("UTF-8")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf16-low-surrogate-utf16"########,
            r########"Bytes(220, 0).decodeToString("UTF-16")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-16".

1 | Bytes(220, 0).decodeToString("UTF-16")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf16-low-surrogate-iso88591"########,
            r########"Bytes(220, 0).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"Ü "########,
        ),
        (
            r########"dec-utf16-pair-utf8"########,
            r########"Bytes(216, 61, 222, 0).decodeToString("UTF-8")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-8".

1 | Bytes(216, 61, 222, 0).decodeToString("UTF-8")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf16-pair-utf16"########,
            r########"Bytes(216, 61, 222, 0).decodeToString("UTF-16")"########,
            r########"value"########,
            r########"😀"########,
        ),
        (
            r########"dec-utf16-pair-iso88591"########,
            r########"Bytes(216, 61, 222, 0).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"Ø=Þ "########,
        ),
        (
            r########"dec-utf16-reversed-bom-utf8"########,
            r########"Bytes(255, 254, 0, 65).decodeToString("UTF-8")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-8".

1 | Bytes(255, 254, 0, 65).decodeToString("UTF-8")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-utf16-reversed-bom-utf16"########,
            r########"Bytes(255, 254, 0, 65).decodeToString("UTF-16")"########,
            r########"value"########,
            r########"䄀"########,
        ),
        (
            r########"dec-utf16-reversed-bom-iso88591"########,
            r########"Bytes(255, 254, 0, 65).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########"ÿþ A"########,
        ),
        (
            r########"dec-latin-high-utf8"########,
            r########"Bytes(0, 127, 128, 255).decodeToString("UTF-8")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Invalid bytes for charset "UTF-8".

1 | Bytes(0, 127, 128, 255).decodeToString("UTF-8")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"dec-latin-high-utf16"########,
            r########"Bytes(0, 127, 128, 255).decodeToString("UTF-16")"########,
            r########"value"########,
            r########"胿"########,
        ),
        (
            r########"dec-latin-high-iso88591"########,
            r########"Bytes(0, 127, 128, 255).decodeToString("ISO-8859-1")"########,
            r########"value"########,
            r########" ÿ"########,
        ),
        (
            r########"list-empty"########,
            r########"List().toBytes()"########,
            r########"value"########,
            r########"Bytes()"########,
        ),
        (
            r########"list-boundary"########,
            r########"List(0,127,128,255).toBytes()"########,
            r########"value"########,
            r########"Bytes(0, 127, 128, 255)"########,
        ),
        (
            r########"list-triple"########,
            r########"List(1,2,3).toBytes()"########,
            r########"value"########,
            r########"Bytes(1, 2, 3)"########,
        ),
        (
            r########"list-roundtrip"########,
            r########"List(0,127,128,255).toBytes().toList()"########,
            r########"value"########,
            r########"List(0, 127, 128, 255)"########,
        ),
        (
            r########"list-all-to-list"########,
            r########"allBytes.toList()"########,
            r########"value"########,
            r########"List(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254, 255)"########,
        ),
        (
            r########"list-all-round-hash"########,
            r########"allBytes.toList().toBytes().sha256"########,
            r########"value"########,
            r########"40aff2e9d2d8922e47afd4648e6967497158785fbd1da870e7110266bf944880"########,
        ),
        (
            r########"list-neg"########,
            r########"List(-1).toBytes()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `isBetween(0, 255)` violated.
Value: -1

    isBetween(0, 255)
    │
    false

1109 | typealias UInt8 = Int(isBetween(0, 255))
                             ^^^^^^^^^^^^^^^^^
at pkl.base#List.toBytes (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | List(-1).toBytes()
    ^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-over"########,
            r########"List(256).toBytes()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `isBetween(0, 255)` violated.
Value: 256

    isBetween(0, 255)
    │
    false

1109 | typealias UInt8 = Int(isBetween(0, 255))
                             ^^^^^^^^^^^^^^^^^
at pkl.base#List.toBytes (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | List(256).toBytes()
    ^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-first-neg"########,
            r########"List(1, -1, 256).toBytes()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `isBetween(0, 255)` violated.
Value: -1

    isBetween(0, 255)
    │
    false

1109 | typealias UInt8 = Int(isBetween(0, 255))
                             ^^^^^^^^^^^^^^^^^
at pkl.base#List.toBytes (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | List(1, -1, 256).toBytes()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-first-over"########,
            r########"List(1, 256, -1).toBytes()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Type constraint `isBetween(0, 255)` violated.
Value: 256

    isBetween(0, 255)
    │
    false

1109 | typealias UInt8 = Int(isBetween(0, 255))
                             ^^^^^^^^^^^^^^^^^
at pkl.base#List.toBytes (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | List(1, 256, -1).toBytes()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-first-string"########,
            r########"List(1, "x", 256).toBytes()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got type `String`.
Value: "x"

1109 | typealias UInt8 = Int(isBetween(0, 255))
                         ^^^
at pkl.base#List.toBytes (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | List(1, "x", 256).toBytes()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-first-null"########,
            r########"List(1, null, 256).toBytes()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got `null`.

1109 | typealias UInt8 = Int(isBetween(0, 255))
                         ^^^
at pkl.base#List.toBytes (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | List(1, null, 256).toBytes()
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-float"########,
            r########"List(1, 2.0).toBytes()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got type `Float`.
Value: 2.0

1109 | typealias UInt8 = Int(isBetween(0, 255))
                         ^^^
at pkl.base#List.toBytes (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1109)

1 | List(1, 2.0).toBytes()
    ^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-eager-throw-second"########,
            r########"List(1, throw("second")).toBytes()"########,
            r########"error"########,
            r########"–– Pkl Error ––
second

1 | List(1, throw("second")).toBytes()
            ^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-eager-throw-after-range"########,
            r########"List(-1, throw("second")).toBytes()"########,
            r########"error"########,
            r########"–– Pkl Error ––
second

1 | List(-1, throw("second")).toBytes()
             ^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"list-intseq-all"########,
            r########"IntSeq(0, 255).toList().toBytes()"########,
            r########"value"########,
            r########"Bytes(0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99, 100, 101, 102, 103, 104, 105, 106, 107, 108, 109, 110, 111, 112, 113, 114, 115, 116, 117, 118, 119, 120, 121, 122, 123, 124, 125, 126, 127, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143, 144, 145, 146, 147, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 170, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181, 182, 183, 184, 185, 186, 187, 188, 189, 190, 191, 192, 193, 194, 195, 196, 197, 198, 199, 200, 201, 202, 203, 204, 205, 206, 207, 208, 209, 210, 211, 212, 213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245, 246, 247, 248, 249, 250, 251, 252, 253, 254, 255)"########,
        ),
        (
            r########"list-intseq-hash"########,
            r########"IntSeq(0, 255).toList().toBytes().sha256"########,
            r########"value"########,
            r########"40aff2e9d2d8922e47afd4648e6967497158785fbd1da870e7110266bf944880"########,
        ),
        (
            r########"concat-empty"########,
            r########"Bytes()+Bytes()"########,
            r########"value"########,
            r########"Bytes()"########,
        ),
        (
            r########"concat-empty-left"########,
            r########"Bytes()+asciiBytes"########,
            r########"value"########,
            r########"Bytes(65, 66, 67)"########,
        ),
        (
            r########"concat-empty-right"########,
            r########"asciiBytes+Bytes()"########,
            r########"value"########,
            r########"Bytes(65, 66, 67)"########,
        ),
        (
            r########"concat-boundary"########,
            r########"Bytes(0,127)+Bytes(128,255)"########,
            r########"value"########,
            r########"Bytes(0, 127, 128, 255)"########,
        ),
        (
            r########"concat-chain"########,
            r########"Bytes(1)+Bytes(2)+Bytes(3)"########,
            r########"value"########,
            r########"Bytes(1, 2, 3)"########,
        ),
        (
            r########"concat-double"########,
            r########"asciiBytes+asciiBytes"########,
            r########"value"########,
            r########"Bytes(65, 66, 67, 65, 66, 67)"########,
        ),
        (
            r########"concat-list-type"########,
            r########"asciiBytes+List(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Bytes` and `List`.
Left operand : Bytes(65, 66, 67)
Right operand: List(1)

1 | asciiBytes+List(1)
    ^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"concat-string-type"########,
            r########"asciiBytes+"x""########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Bytes` and `String`.
Left operand : Bytes(65, 66, 67)
Right operand: "x"

1 | asciiBytes+"x"
    ^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"concat-int-type"########,
            r########"asciiBytes+1"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `+` is not defined for operand types `Bytes` and `Int`.
Left operand : Bytes(65, 66, 67)
Right operand: 1

1 | asciiBytes+1
    ^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"concat-throw-right"########,
            r########"asciiBytes+throw("right")"########,
            r########"error"########,
            r########"–– Pkl Error ––
right

1 | asciiBytes+throw("right")
               ^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"value-same"########,
            r########"Bytes(1)==Bytes(1)"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"value-different"########,
            r########"Bytes(1)==Bytes(2)"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"value-not-equal"########,
            r########"Bytes(1)!=Bytes(2)"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"value-list"########,
            r########"Bytes(1)==List(1)"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"value-empty-list"########,
            r########"Bytes()==List()"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"value-all-intseq"########,
            r########"allBytes==IntSeq(0,255).toList().toBytes()"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"value-set-equal"########,
            r########"Set(Bytes(1),Bytes(1)).length"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"value-set-diff"########,
            r########"Set(Bytes(1),Bytes(2)).length"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"value-set-threshold-five"########,
            r########"Set(Bytes(1),Bytes(2),Bytes(3),Bytes(4),Bytes(1)).length"########,
            r########"value"########,
            r########"4"########,
        ),
        (
            r########"value-map-equal"########,
            r########"Map(Bytes(1),"a",Bytes(1),"b").length"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"value-map-diff"########,
            r########"Map(Bytes(1),"a",Bytes(2),"b").length"########,
            r########"value"########,
            r########"2"########,
        ),
        (
            r########"value-map-threshold-five"########,
            r########"Map(Bytes(1),1,Bytes(2),2,Bytes(3),3,Bytes(4),4,Bytes(1),5).length"########,
            r########"value"########,
            r########"4"########,
        ),
        (
            r########"value-set-empty"########,
            r########"Set(Bytes(),Bytes()).length"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"value-map-empty"########,
            r########"Map(Bytes(),1,Bytes(),2).length"########,
            r########"value"########,
            r########"1"########,
        ),
        (
            r########"value-class"########,
            r########"Bytes(1).getClass().simpleName"########,
            r########"value"########,
            r########"Bytes"########,
        ),
        (
            r########"value-string"########,
            r########"Bytes(1).toString()"########,
            r########"value"########,
            r########"Bytes(1)"########,
        ),
        (
            r########"value-if-non-null"########,
            r########"Bytes(1).ifNonNull((it)->it.hex)"########,
            r########"value"########,
            r########"01"########,
        ),
        (
            r########"value-is-bytes"########,
            r########"Bytes(1) is Bytes"########,
            r########"value"########,
            r########"true"########,
        ),
        (
            r########"value-is-list"########,
            r########"Bytes(1) is List"########,
            r########"value"########,
            r########"false"########,
        ),
        (
            r########"value-as-bytes"########,
            r########"Bytes(1) as Bytes"########,
            r########"value"########,
            r########"Bytes(1)"########,
        ),
        (
            r########"error-charset-lower"########,
            r########"Bytes(1).decodeToString("utf-8")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `"UTF-8"|"UTF-16"|"ISO-8859-1"`, but got `"utf-8"`.

1691 | typealias Charset = "UTF-8" | "UTF-16" | "ISO-8859-1"
                           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Bytes.decodeToString (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1691)

1 | Bytes(1).decodeToString("utf-8")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"error-charset-no-hyphen"########,
            r########"Bytes(1).decodeToString("UTF8")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `"UTF-8"|"UTF-16"|"ISO-8859-1"`, but got `"UTF8"`.

1691 | typealias Charset = "UTF-8" | "UTF-16" | "ISO-8859-1"
                           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Bytes.decodeToString (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1691)

1 | Bytes(1).decodeToString("UTF8")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"error-charset-latin-alias"########,
            r########"Bytes(1).decodeToString("latin1")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `"UTF-8"|"UTF-16"|"ISO-8859-1"`, but got `"latin1"`.

1691 | typealias Charset = "UTF-8" | "UTF-16" | "ISO-8859-1"
                           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Bytes.decodeToString (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1691)

1 | Bytes(1).decodeToString("latin1")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"error-charset-unsupported"########,
            r########"Bytes(1).decodeToString("UTF-32")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `"UTF-8"|"UTF-16"|"ISO-8859-1"`, but got `"UTF-32"`.

1691 | typealias Charset = "UTF-8" | "UTF-16" | "ISO-8859-1"
                           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Bytes.decodeToString (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1691)

1 | Bytes(1).decodeToString("UTF-32")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"error-charset-ascii"########,
            r########"Bytes(1).decodeToString("ASCII")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `"UTF-8"|"UTF-16"|"ISO-8859-1"`, but got `"ASCII"`.

1691 | typealias Charset = "UTF-8" | "UTF-16" | "ISO-8859-1"
                           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Bytes.decodeToString (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1691)

1 | Bytes(1).decodeToString("ASCII")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"error-charset-int"########,
            r########"Bytes(1).decodeToString(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `"UTF-8"|"UTF-16"|"ISO-8859-1"`, but got type `Int`.
Value: 1

1691 | typealias Charset = "UTF-8" | "UTF-16" | "ISO-8859-1"
                           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Bytes.decodeToString (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1691)

1 | Bytes(1).decodeToString(1)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"error-charset-null"########,
            r########"Bytes(1).decodeToString(null)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `"UTF-8"|"UTF-16"|"ISO-8859-1"`, but got `null`.

1691 | typealias Charset = "UTF-8" | "UTF-16" | "ISO-8859-1"
                           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Bytes.decodeToString (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1691)

1 | Bytes(1).decodeToString(null)
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"error-charset-missing"########,
            r########"Bytes(1).decodeToString()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 1 function arguments but got 0.

3752 | external function decodeToString(charset: Charset): String
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Bytes.decodeToString (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3752)

1 | Bytes(1).decodeToString()
    ^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"error-charset-extra"########,
            r########"Bytes(1).decodeToString("UTF-8", "UTF-16")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 1 function arguments but got 2.

3752 | external function decodeToString(charset: Charset): String
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Bytes.decodeToString (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3752)

1 | Bytes(1).decodeToString("UTF-8", "UTF-16")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"error-encode-alias"########,
            r########""a".encodeToBytes("utf-8")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `"UTF-8"|"UTF-16"|"ISO-8859-1"`, but got `"utf-8"`.

1691 | typealias Charset = "UTF-8" | "UTF-16" | "ISO-8859-1"
                           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#String.encodeToBytes (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1691)

1 | "a".encodeToBytes("utf-8")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"error-encode-unsupported"########,
            r########""a".encodeToBytes("UTF-32")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `"UTF-8"|"UTF-16"|"ISO-8859-1"`, but got `"UTF-32"`.

1691 | typealias Charset = "UTF-8" | "UTF-16" | "ISO-8859-1"
                           ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#String.encodeToBytes (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L1691)

1 | "a".encodeToBytes("UTF-32")
    ^^^^^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"error-get-string"########,
            r########"Bytes(1).getOrNull("0")"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected value of type `Int`, but got type `String`.
Value: "0"

3747 | external function getOrNull(index: Int): UInt8?
                                          ^^^
at pkl.base#Bytes.getOrNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3747)

1 | Bytes(1).getOrNull("0")
    ^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"error-get-missing"########,
            r########"Bytes(1).getOrNull()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 1 function arguments but got 0.

3747 | external function getOrNull(index: Int): UInt8?
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Bytes.getOrNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3747)

1 | Bytes(1).getOrNull()
    ^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"error-get-extra"########,
            r########"Bytes(1).getOrNull(0,1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 1 function arguments but got 2.

3747 | external function getOrNull(index: Int): UInt8?
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Bytes.getOrNull (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3747)

1 | Bytes(1).getOrNull(0,1)
    ^^^^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"error-tolist-extra"########,
            r########"Bytes(1).toList(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

3755 | external function toList(): List<UInt8>
       ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Bytes.toList (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L3755)

1 | Bytes(1).toList(1)
    ^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"error-missing-member"########,
            r########"Bytes(1).missing"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find property `missing` in object of type `Bytes`.

1 | Bytes(1).missing
    ^^^^^^^^^^^^^^^^
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
"########,
        ),
        (
            r########"error-property-as-method"########,
            r########"Bytes(1).sha256()"########,
            r########"error"########,
            r########"–– Pkl Error ––
Cannot find method `sha256` in class `Bytes`.

1 | Bytes(1).sha256()
    ^^^^^^^^^^^^^^^^^
at  (repl:text)

Did you mean any of the following?
sha256
sha256Int
sha1
"########,
        ),
        (
            r########"error-tostring-extra"########,
            r########"Bytes(1).toString(1)"########,
            r########"error"########,
            r########"–– Pkl Error ––
Expected 0 function arguments but got 1.

54 | external function toString(): String
     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
at pkl.base#Any.toString (https://github.com/apple/pkl/blob/0.32.0/stdlib/base.pkl#L54)

1 | Bytes(1).toString(1)
    ^^^^^^^^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"error-sub-string"########,
            r########"Bytes(1)["0"]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `[]` is not defined for operand types `Bytes` and `String`.
Left operand : Bytes(1)
Right operand: "0"

1 | Bytes(1)["0"]
    ^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"error-sub-float"########,
            r########"Bytes(1)[1.0]"########,
            r########"error"########,
            r########"–– Pkl Error ––
Operator `[]` is not defined for operand types `Bytes` and `Float`.
Left operand : Bytes(1)
Right operand: 1.0

1 | Bytes(1)[1.0]
    ^^^^^^^^^^^^^
at  (repl:text)
"########,
        ),
        (
            r########"conversion-signed-subscript"########,
            r########"Bytes(255)[0]"########,
            r########"value"########,
            r########"-1"########,
        ),
        (
            r########"conversion-signed-get"########,
            r########"Bytes(255).getOrNull(0)"########,
            r########"value"########,
            r########"-1"########,
        ),
        (
            r########"conversion-unsigned-list"########,
            r########"Bytes(255).toList()[0]"########,
            r########"value"########,
            r########"255"########,
        ),
        (
            r########"conversion-unsigned-list-roundtrip"########,
            r########"List(255).toBytes().toList()[0]"########,
            r########"value"########,
            r########"255"########,
        ),
        (
            r########"conversion-boundary-to-list"########,
            r########"boundaryBytes.toList()"########,
            r########"value"########,
            r########"List(0, 127, 128, 255)"########,
        ),
        (
            r########"conversion-all-to-list-last"########,
            r########"allBytes.toList()[255]"########,
            r########"value"########,
            r########"255"########,
        ),
    ] {
        let is_intseq = matches!(
            id,
            "list-intseq-all" | "list-intseq-hash" | "value-all-intseq"
        );
        if is_intseq != known_intseq {
            continue;
        }
        let event = query(&mut probe, expression);
        let actual_kind = event.kind;
        let actual = if actual_kind == "error" {
            event.error
        } else {
            event.value
        };
        let expected_units: Vec<_> = expected.encode_utf16().collect();
        if actual_kind != kind || actual.as_deref() != Some(expected_units.as_slice()) {
            mismatches.push(format!(
                "{id}: expected {kind} {expected:?}; actual {actual_kind} {:?}",
                actual.map(|text| String::from_utf16_lossy(&text))
            ));
        }
    }
    probe.close();
    assert_eq!(probe.collect().slots, 0);
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n\n"));
}

#[test]
fn bytes_values_operations_and_complete_diagnostics_match_java() {
    compare_java(false);
}

#[test]
fn intseq_bytes_construction_matches_java() {
    compare_java(true);
}

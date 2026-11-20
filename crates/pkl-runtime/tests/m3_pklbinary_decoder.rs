//! Pkl 0.32.0 PklBinaryDecoder parity tests matching `PklBinaryDecoderTest.kt`
//! and live pinned Java 0.32.0 oracle observations, plus full 19-tag lossless
//! round-trip tests with the shared `pkl:pklbinary` encoder.

use pkl_runtime::{
    data_size::{DataSize, DataSizeUnit},
    duration::{Duration, DurationUnit},
    evaluation::frontend::{ModuleSourceInput, evaluate_module_output},
    pkl_binary_decode::{
        DecodeErrorKind, DecodedClassInfo, DecodedIntSeq, DecodedObjectMember, DecodedValue,
        ObjectMemberKind, PklBinaryCode, PklBinaryDecoder, decode, decode_lossless,
    },
    value::PklString,
};

const FIXTURE: &[u8] = include_bytes!(
    "../../../external/pkl/pkl-core/src/test/resources/org/pkl/core/pklBinaryDecoderTest.msgpack"
);

const STR_FOO: &[u8] = &[0xa3, b'f', b'o', b'o'];
const DOUBLE_ZERO: &[u8] = &[0xcb, 0, 0, 0, 0, 0, 0, 0, 0];

fn concat(parts: &[&[u8]]) -> Vec<u8> {
    parts
        .iter()
        .flat_map(|slice| slice.iter().copied())
        .collect()
}

fn assert_cause_contains(bytes: &[u8], expected: &str) {
    let err = PklBinaryDecoder::decode(bytes).expect_err("expected decode failure");
    assert!(
        err.cause.contains(expected),
        "expected cause to contain {expected:?}, got cause={:?} (message={:?})",
        err.cause,
        err.message
    );
}

#[test]
fn decodes_upstream_pkl_binary_decoder_fixture() {
    let decoded = decode(FIXTURE).expect("decode pklBinaryDecoderTest.msgpack");
    let DecodedValue::Object(root) = decoded else {
        panic!("expected root DecodedValue::Object, got {decoded:?}");
    };

    let expected_uri = "file:///$snippetsDir/input/api/encoding1.msgpack.yaml.pkl";
    assert_eq!(root.class_name, "encoding1#Foo");
    assert_eq!(root.module_uri, expected_uri);
    assert_eq!(
        root.class_info,
        DecodedClassInfo {
            module_name: "encoding1".into(),
            simple_name: "Foo".into(),
            module_uri: expected_uri.into(),
            is_module: false,
            is_dynamic: false,
        }
    );

    let names: Vec<&str> = root
        .members
        .iter()
        .map(|m| {
            assert_eq!(m.kind, ObjectMemberKind::Property);
            let DecodedValue::String(name) = &m.key else {
                panic!("expected property string key");
            };
            name.as_str().unwrap()
        })
        .collect();
    assert_eq!(
        names,
        vec![
            "dynamic",
            "string",
            "map",
            "mapping",
            "list",
            "listing",
            "set",
            "duration",
            "dataSize",
            "pair",
            "intSeq",
            "regex",
            "func",
            "bytes",
            "moduleClass",
            "baseModuleClass",
            "encodingModuleClass",
            "stdlibClass",
            "someClass",
            "stdlibTypealias",
            "someTypealias",
            "something",
        ]
    );

    // Dynamic object preserves exact wire member kinds and also supports Java property view.
    let DecodedValue::Object(dynamic) = root.get_property("dynamic").unwrap() else {
        panic!("expected dynamic object");
    };
    assert!(dynamic.class_info.is_dynamic);
    assert_eq!(dynamic.class_info.module_name, "pkl.base");
    assert_eq!(dynamic.class_info.simple_name, "Dynamic");
    assert_eq!(dynamic.class_info.module_uri, "pkl:base");
    assert_eq!(
        dynamic.members,
        vec![
            DecodedObjectMember {
                kind: ObjectMemberKind::Property,
                key: DecodedValue::String("hello".into()),
                value: DecodedValue::String("world".into()),
            },
            DecodedObjectMember {
                kind: ObjectMemberKind::Entry,
                key: DecodedValue::String("hello".into()),
                value: DecodedValue::String("world".into()),
            },
            DecodedObjectMember {
                kind: ObjectMemberKind::Element,
                key: DecodedValue::Int(0),
                value: DecodedValue::String("hello world".into()),
            },
        ]
    );
    let dyn_props: Vec<(PklString, &DecodedValue)> = dynamic.properties();
    assert_eq!(dyn_props.len(), 2);
    assert_eq!(dyn_props[0].0, "hello");
    assert_eq!(dyn_props[0].1, &DecodedValue::String("world".into()));
    assert_eq!(dyn_props[1].0, "0");
    assert_eq!(dyn_props[1].1, &DecodedValue::String("hello world".into()));

    assert_eq!(
        root.get_property("string"),
        Some(&DecodedValue::String("foo".into()))
    );
    assert_eq!(
        root.get_property("map"),
        Some(&DecodedValue::Map(vec![(
            DecodedValue::String("foo".into()),
            DecodedValue::String("bar".into()),
        )]))
    );
    assert_eq!(
        root.get_property("mapping"),
        Some(&DecodedValue::Mapping(vec![(
            DecodedValue::String("foo".into()),
            DecodedValue::String("bar".into()),
        )]))
    );
    assert_eq!(
        root.get_property("list"),
        Some(&DecodedValue::List(vec![
            DecodedValue::String("foo".into()),
            DecodedValue::String("bar".into()),
        ]))
    );
    assert_eq!(
        root.get_property("listing"),
        Some(&DecodedValue::Listing(vec![
            DecodedValue::String("foo".into()),
            DecodedValue::Int(0),
        ]))
    );
    assert_eq!(
        root.get_property("set"),
        Some(&DecodedValue::Set(vec![
            DecodedValue::String("foo".into()),
            DecodedValue::String("bar".into()),
        ]))
    );

    let Some(DecodedValue::Duration(dur)) = root.get_property("duration") else {
        panic!("expected Duration");
    };
    assert_eq!(dur.value(), 123.0);
    assert_eq!(dur.unit(), DurationUnit::Hours);

    let Some(DecodedValue::DataSize(ds)) = root.get_property("dataSize") else {
        panic!("expected DataSize");
    };
    assert_eq!(ds.value(), 123.0);
    assert_eq!(ds.unit(), DataSizeUnit::Gibibytes);

    assert_eq!(
        root.get_property("pair"),
        Some(&DecodedValue::Pair(
            Box::new(DecodedValue::String("foo".into())),
            Box::new(DecodedValue::String("bar".into())),
        ))
    );
    assert_eq!(root.get_property("intSeq"), Some(&DecodedValue::Null));
    assert_eq!(
        root.get_property("regex"),
        Some(&DecodedValue::Regex("foo.*".into()))
    );
    assert_eq!(root.get_property("func"), Some(&DecodedValue::Null));
    assert_eq!(
        root.get_property("bytes"),
        Some(&DecodedValue::Bytes(vec![0x01, 0x02, 0x03]))
    );
    for nulled in [
        "moduleClass",
        "baseModuleClass",
        "encodingModuleClass",
        "stdlibClass",
        "someClass",
        "stdlibTypealias",
        "someTypealias",
    ] {
        assert_eq!(root.get_property(nulled), Some(&DecodedValue::Null));
    }

    let DecodedValue::Object(pcf) = root.get_property("something").unwrap() else {
        panic!("expected PcfRenderer object");
    };
    assert_eq!(
        pcf.class_info,
        DecodedClassInfo {
            module_name: "pkl.base".into(),
            simple_name: "PcfRenderer".into(),
            module_uri: "pkl:base".into(),
            is_module: false,
            is_dynamic: false,
        }
    );
    assert_eq!(
        pcf.get_property("converters"),
        Some(&DecodedValue::Mapping(vec![]))
    );
    assert_eq!(
        pcf.get_property("extension"),
        Some(&DecodedValue::String("pcf".into()))
    );
    assert_eq!(
        pcf.get_property("indent"),
        Some(&DecodedValue::String("  ".into()))
    );
    assert_eq!(
        pcf.get_property("omitNullProperties"),
        Some(&DecodedValue::Bool(false))
    );
    assert_eq!(
        pcf.get_property("useCustomStringDelimiters"),
        Some(&DecodedValue::Bool(false))
    );
}

#[test]
fn decoding_of_unsupported_pkl_types_fails_in_strict_mode() {
    assert_cause_contains(
        &[0x94, PklBinaryCode::IntSeq.code(), 0x00, 0x04, 0x02],
        "Cannot decode IntSeq value",
    );
    assert_cause_contains(
        &concat(&[&[0x93, PklBinaryCode::Class.code()], STR_FOO, STR_FOO]),
        "Cannot decode Class value",
    );
    assert_cause_contains(
        &concat(&[&[0x93, PklBinaryCode::TypeAlias.code()], STR_FOO, STR_FOO]),
        "Cannot decode TypeAlias value",
    );
    assert_cause_contains(
        &[0x91, PklBinaryCode::Function.code()],
        "Cannot decode Function value",
    );
    assert_cause_contains(
        &[0x94, PklBinaryCode::Reference.code(), 0x00, 0x00, 0x90],
        "Unrecognized object code REFERENCE",
    );
}

#[test]
fn decode_unexpected_eof_and_offsets_match_java() {
    let err = decode(&[]).unwrap_err();
    assert_eq!(err.kind, DecodeErrorKind::UnexpectedEof);
    assert_eq!(
        err.message,
        "Exception while decoding binary data at offset 0, path []"
    );
    assert_eq!(err.cause, "Unexpected EOF");

    // missing List element
    let err = decode(&[0x92, PklBinaryCode::List.code(), 0x91]).unwrap_err();
    assert_eq!(
        err.message,
        "Exception while decoding binary data at offset 3, path ['list]"
    );
    assert_eq!(err.cause, "Unexpected EOF");

    // missing struct slot
    let err = decode(&[0x92, PklBinaryCode::List.code()]).unwrap_err();
    assert_eq!(
        err.message,
        "Exception while decoding binary data at offset 2, path ['list]"
    );
    assert_eq!(err.cause, "Unexpected EOF");
}

#[test]
fn decode_unexpected_msgpack_values_match_java() {
    for (bytes, expected_cause) in [
        (&[0xc4u8, 0x00][..], "Unexpected msgpack bin value"),
        (&[0x80u8][..], "Unexpected msgpack map value"),
        (&[0xd4u8, 0x00, 0x00][..], "Unexpected msgpack ext value"),
        (&[0xc1u8][..], "Cannot convert NEVER_USED to ValueType"),
    ] {
        let err = decode(bytes).unwrap_err();
        assert_eq!(
            err.message,
            "Exception while decoding binary data at offset 0, path []"
        );
        assert_eq!(err.cause, expected_cause);
    }
}

#[test]
fn decode_invalid_non_primitives_match_java() {
    let err = decode(&[0x90]).unwrap_err();
    assert_eq!(err.offset, 1);
    assert_eq!(err.cause, "Unexpected empty object array value");

    let err = decode(&[0x93, 0xff]).unwrap_err();
    assert_eq!(err.offset, 2);
    assert_eq!(err.cause, "Unrecognized code 0xff");

    let err = decode(&[0x93, 0x00]).unwrap_err();
    assert_eq!(err.offset, 2);
    assert_eq!(err.cause, "Unrecognized code 0x0");

    let err = decode(&[0x93, PklBinaryCode::Element.code(), 0x00, 0x00]).unwrap_err();
    assert_eq!(err.offset, 2);
    assert_eq!(err.cause, "Unrecognized object code ELEMENT");
}

#[test]
fn decode_invalid_object_and_nested_path_match_java() {
    // too few slots
    let err = decode(&[0x91, PklBinaryCode::Object.code()]).unwrap_err();
    assert_eq!(err.offset, 2);
    assert_eq!(
        err.cause,
        "Expected OBJECT structure to have at least 4 slots, found 1"
    );

    // blank class
    assert_cause_contains(
        &concat(&[&[0x94, PklBinaryCode::Object.code(), 0xa0], STR_FOO]),
        "Unexpected blank object class name",
    );

    // blank module uri
    assert_cause_contains(
        &concat(&[&[0x94, PklBinaryCode::Object.code()], STR_FOO, &[0xa0]]),
        "Unexpected blank object module URI",
    );

    // unexpected member struct length
    assert_cause_contains(
        &concat(&[
            &[0x94, PklBinaryCode::Object.code()],
            STR_FOO,
            STR_FOO,
            &[0x91, 0x90],
        ]),
        "Expected 3 fields in object member, found 0",
    );

    // unrecognized code in a member
    assert_cause_contains(
        &concat(&[
            &[0x94, PklBinaryCode::Object.code()],
            STR_FOO,
            STR_FOO,
            &[0x91, 0x93, 0xff],
        ]),
        "Unrecognized code 0xff",
    );

    // unrecognized member code (object code)
    assert_cause_contains(
        &concat(&[
            &[0x94, PklBinaryCode::Object.code()],
            STR_FOO,
            STR_FOO,
            &[0x91, 0x93, PklBinaryCode::Object.code()],
        ]),
        "Unrecognized member code OBJECT",
    );

    // Nested path unwinding matches Java DecodeIterator.next() finally pop behavior.
    let nested = [
        0x94, 0x01, 0xa1, b'A', 0xa1, b'u', 0x91, 0x93, 0x10, 0xa1, b'p', 0x92, 0x04, 0x91, 0x91,
        0xff,
    ];
    let err = decode(&nested).unwrap_err();
    assert_eq!(
        err.message,
        "Exception while decoding binary data at offset 16, path [p, 0, 'object]"
    );
    assert_eq!(err.cause, "Unrecognized code 0xff");
}

#[test]
fn decode_invalid_units_and_types_match_java() {
    assert_cause_contains(
        &concat(&[
            &[0x93, PklBinaryCode::Duration.code()],
            DOUBLE_ZERO,
            STR_FOO,
        ]),
        "Invalid Duration unit `foo`",
    );
    assert_cause_contains(
        &concat(&[
            &[0x93, PklBinaryCode::DataSize.code()],
            DOUBLE_ZERO,
            STR_FOO,
        ]),
        "Invalid DataSize unit `foo`",
    );
    assert_cause_contains(
        &concat(&[&[0x93, PklBinaryCode::Class.code(), 0xa0], STR_FOO]),
        "Unexpected blank class name",
    );
    assert_cause_contains(
        &concat(&[&[0x93, PklBinaryCode::Class.code()], STR_FOO, &[0xa0]]),
        "Unexpected blank class module URI",
    );
    assert_cause_contains(
        &concat(&[&[0x93, PklBinaryCode::TypeAlias.code(), 0xa0], STR_FOO]),
        "Unexpected blank typealias name",
    );
    assert_cause_contains(
        &concat(&[&[0x93, PklBinaryCode::TypeAlias.code()], STR_FOO, &[0xa0]]),
        "Unexpected blank typealias module URI",
    );
}

#[test]
fn java_edge_cases_whitespace_floats_overflow_trailing_slots_uri_regex_utf8() {
    // NBSP (U+00A0) is NOT blank in Java String.isBlank(); U+3000 IS blank.
    let nbsp = [0x94, 0x01, 0xa2, 0xc2, 0xa0, 0xa1, b'u', 0x90];
    let DecodedValue::Object(obj) = decode(&nbsp).expect("NBSP is not Java blank") else {
        panic!("expected object");
    };
    assert!(obj.class_info.is_module);
    assert_eq!(obj.class_name, "\u{00a0}");

    let u3000 = [0x94, 0x01, 0xa3, 0xe3, 0x80, 0x80, 0xa1, b'u', 0x90];
    let err = decode(&u3000).unwrap_err();
    assert_eq!(err.offset, 6);
    assert_eq!(err.cause, "Unexpected blank object class name");

    // float32 accepted by unpackDouble()
    let float32_dur = [
        0x93,
        PklBinaryCode::Duration.code(),
        0xca,
        0x3f,
        0x80,
        0x00,
        0x00,
        0xa1,
        b's',
    ];
    assert_eq!(
        decode(&float32_dur).unwrap(),
        DecodedValue::Duration(Duration::new(1.0, DurationUnit::Seconds))
    );

    // integer rejected by unpackDouble() at offset 3
    let int_dur = [0x93, PklBinaryCode::Duration.code(), 0x01, 0xa1, b's'];
    let err = decode(&int_dur).unwrap_err();
    assert_eq!(
        err.message,
        "Exception while decoding binary data at offset 3, path ['duration]"
    );
    assert_eq!(err.cause, "Expected Float, but got Integer (01)");

    // uint64 > i64::MAX overflows
    let overflow = [0xcf, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff];
    let err = decode(&overflow).unwrap_err();
    assert_eq!(err.kind, DecodeErrorKind::IntegerOverflow);
    assert_eq!(err.offset, 9);
    assert_eq!(err.cause, "18446744073709551615");

    // Trailing slots skipped cleanly via skipValue
    let extra = [
        0x93,
        PklBinaryCode::List.code(),
        0x91,
        0x2a,
        0x81,
        0xa1,
        b'k',
        0x92,
        0xc0,
        0xc3,
    ];
    assert_eq!(
        decode(&extra).unwrap(),
        DecodedValue::List(vec![DecodedValue::Int(42)])
    );

    // Invalid URI & invalid Regex rejected
    let bad_uri = [0x94, 0x01, 0xa1, b'A', 0xa3, b'a', b' ', b'b', 0x90];
    let err = decode(&bad_uri).unwrap_err();
    assert_eq!(err.kind, DecodeErrorKind::InvalidUri);

    let bad_regex = [0x92, PklBinaryCode::Regex.code(), 0xa1, b'['];
    let err = decode(&bad_regex).unwrap_err();
    assert_eq!(err.kind, DecodeErrorKind::InvalidRegex);

    // Invalid UTF-8 in string replaced matching JDK 21 UTF_8$Decoder (CodingErrorAction.REPLACE)
    assert_eq!(
        decode(&[0xa1, 0xff]).unwrap(),
        DecodedValue::String("\u{fffd}".into())
    );
    // 3-byte UTF-8 encoded surrogate ED A0 80 -> single U+FFFD (unlike Rust from_utf8_lossy)
    assert_eq!(
        decode(&[0xa3, 0xed, 0xa0, 0x80]).unwrap(),
        DecodedValue::String("\u{fffd}".into())
    );
    // Truncated 2-byte surrogate prefix ED A0 -> single U+FFFD
    assert_eq!(
        decode(&[0xa2, 0xed, 0xa0]).unwrap(),
        DecodedValue::String("\u{fffd}".into())
    );
    // Surrounded surrogate and truncated prefix before ASCII
    assert_eq!(
        decode(&[0xa5, b'x', 0xed, 0xa0, 0x80, b'y']).unwrap(),
        DecodedValue::String("x\u{fffd}y".into())
    );
    assert_eq!(
        decode(&[0xa3, 0xed, 0xa0, b' ']).unwrap(),
        DecodedValue::String("\u{fffd} ".into())
    );
    assert_eq!(
        decode(&[0xa3, 0xf0, 0x90, 0x80]).unwrap(),
        DecodedValue::String("\u{fffd}".into())
    );
}

#[test]
fn round_trips_all_19_pkl_binary_codes_with_shared_encoder() {
    let source = r#"
import "pkl:pklbinary"
import "pkl:ref"

class Sample {
  name: String = "alpha"
}

class MyDomain extends ref.Domain {
  function renderReference(r) = "rendered"
}

typealias Alias = String

class Root {
  nullVal: Null = null
  boolVal: Boolean = true
  intVal: Int = -42
  floatVal: Float = -0.0
  strVal: String = "héllo 🚀"
  dynVal: Dynamic = new Dynamic {
    prop = 1
    ["entryKey"] = 2
    "elemVal"
  }
  typedVal: Sample = new Sample {}
  mapVal: Map = Map("k", 1, 2, "v")
  mappingVal: Mapping = new Mapping { ["m"] = 3 }
  listVal: List = List(1, 2)
  listingVal: Listing = new Listing { 3; 4 }
  setVal: Set = Set("a", "b")
  durVal: Duration = 5.min
  sizeVal: DataSize = 16.kib
  pairVal: Pair = Pair("left", 99)
  intSeqVal: IntSeq = IntSeq(1, 10).step(2)
  regexVal: Regex = Regex("[a-z]+")
  classVal: Class = Sample
  aliasVal: TypeAlias = Alias
  funcVal: Function = (x) -> x + 1
  bytesVal: Bytes = Bytes(0, 127, 255)
  refVal: Any = ref.Reference(new MyDomain {}, Sample, "payload").name
}

output {
  renderer = new pklbinary.Renderer {}
  value = new Root {}
}
"#;
    let uri = "file:///m3-roundtrip/main.pkl";
    let result = evaluate_module_output(
        ModuleSourceInput {
            text: source,
            uri,
            inferred_name: "main",
        },
        &Default::default(),
    );
    assert_eq!(
        result.exit_code,
        0,
        "stderr: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());

    // Strict mode fails on IntSeq (the first non-exportable member in declaration order)
    let strict_err = decode(&result.stdout).unwrap_err();
    assert_eq!(strict_err.kind, DecodeErrorKind::UnsupportedType);
    assert_eq!(strict_err.cause, "Cannot decode IntSeq value");

    // Lossless mode decodes all 19 codes cleanly!
    let DecodedValue::Object(root) = decode_lossless(&result.stdout).expect("lossless decode")
    else {
        panic!("expected root Object");
    };
    assert_eq!(root.class_name, "main#Root");
    assert_eq!(root.module_uri, uri);
    assert_eq!(root.get_property("nullVal"), Some(&DecodedValue::Null));
    assert_eq!(
        root.get_property("boolVal"),
        Some(&DecodedValue::Bool(true))
    );
    assert_eq!(root.get_property("intVal"), Some(&DecodedValue::Int(-42)));
    let Some(DecodedValue::Float(neg_zero)) = root.get_property("floatVal") else {
        panic!("expected floatVal");
    };
    assert_eq!(neg_zero.to_bits(), (-0.0f64).to_bits());
    assert_eq!(
        root.get_property("strVal"),
        Some(&DecodedValue::String("héllo 🚀".into()))
    );

    let Some(DecodedValue::Object(dyn_val)) = root.get_property("dynVal") else {
        panic!("expected dynVal");
    };
    assert_eq!(dyn_val.members.len(), 3);
    assert_eq!(dyn_val.members[0].kind, ObjectMemberKind::Property);
    assert_eq!(dyn_val.members[1].kind, ObjectMemberKind::Entry);
    assert_eq!(dyn_val.members[2].kind, ObjectMemberKind::Element);
    assert_eq!(dyn_val.members[2].key, DecodedValue::Int(0));

    assert_eq!(
        root.get_property("durVal"),
        Some(&DecodedValue::Duration(Duration::new(
            5.0,
            DurationUnit::Minutes
        )))
    );
    assert_eq!(
        root.get_property("sizeVal"),
        Some(&DecodedValue::DataSize(DataSize::new(
            16.0,
            DataSizeUnit::Kibibytes
        )))
    );
    assert_eq!(
        root.get_property("pairVal"),
        Some(&DecodedValue::Pair(
            Box::new(DecodedValue::String("left".into())),
            Box::new(DecodedValue::Int(99)),
        ))
    );
    assert_eq!(
        root.get_property("intSeqVal"),
        Some(&DecodedValue::IntSeq(DecodedIntSeq {
            start: 1,
            end: 10,
            step: 2,
        }))
    );
    assert_eq!(
        root.get_property("regexVal"),
        Some(&DecodedValue::Regex("[a-z]+".into()))
    );
    assert_eq!(
        root.get_property("classVal"),
        Some(&DecodedValue::Class {
            qualified_name: "main#Sample".into(),
            module_uri: uri.into(),
        })
    );
    assert_eq!(
        root.get_property("aliasVal"),
        Some(&DecodedValue::TypeAlias {
            qualified_name: "main#Alias".into(),
            module_uri: uri.into(),
        })
    );
    assert_eq!(root.get_property("funcVal"), Some(&DecodedValue::Function));
    assert_eq!(
        root.get_property("bytesVal"),
        Some(&DecodedValue::Bytes(vec![0, 127, 255]))
    );

    let Some(DecodedValue::Reference(reference)) = root.get_property("refVal") else {
        panic!("expected Reference (0x20)");
    };
    assert!(matches!(reference.domain.as_ref(), DecodedValue::Object(_)));
    assert_eq!(
        reference.data.as_ref(),
        &DecodedValue::String("payload".into())
    );
    assert_eq!(reference.path.len(), 1);
    let DecodedValue::Object(access) = &reference.path[0] else {
        panic!("expected Access object in reference path");
    };
    assert_eq!(
        access.get_property("property"),
        Some(&DecodedValue::String("name".into()))
    );
    assert_eq!(access.get_property("key"), Some(&DecodedValue::Null));
}

#[test]
fn iterative_decoder_handles_deep_nesting_without_stack_overflow() {
    let depth = 500;
    let mut bytes = Vec::with_capacity(depth * 3 + 1);
    for _ in 0..depth {
        bytes.extend_from_slice(&[0x92, PklBinaryCode::List.code(), 0x91]);
    }
    bytes.push(0x2a);
    let mut val = decode(&bytes).expect("deeply nested list decodes");
    for _ in 0..depth {
        let DecodedValue::List(mut items) = val else {
            panic!("expected List");
        };
        assert_eq!(items.len(), 1);
        val = items.pop().unwrap();
    }
    assert_eq!(val, DecodedValue::Int(42));
}

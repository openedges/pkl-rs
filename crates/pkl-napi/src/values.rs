//! Lossless JS <-> Rust value conversion for `pkl-binary` `DecodedValue`,
//! round-trip validation, and conversion telemetry.

use crate::error::{
    NativeError, js_pkl_string, parse_i64_bigint, to_js_outcome, utf16_to_pkl, utf16_to_string,
};
use napi::{
    Env, JsString, Result,
    bindgen_prelude::{Array, BigInt, Either8, Null, Object, Uint8Array, Utf16String},
};
use napi_derive::napi;
use pkl_runtime::{
    data_size::{DataSize, DataSizeUnit},
    duration::{Duration, DurationUnit},
    pkl_binary_decode::{
        DecodedClassInfo, DecodedIntSeq, DecodedObject, DecodedObjectMember, DecodedReference,
        DecodedValue, ObjectMemberKind, decode_lossless,
    },
    value::PklString,
};
use std::time::Instant;

/// Safe N-API output union covering all lossless JS primitives, arrays, bytes, and tagged objects.
pub type JsAnyOut<'env> =
    Either8<Null, bool, BigInt, f64, JsString<'env>, Uint8Array, Array<'env>, Object<'env>>;

/// Safe N-API input union ordered so typed arrays and arrays match before generic objects.
pub type JsAnyIn<'env> =
    Either8<Null, bool, BigInt, f64, Utf16String, Uint8Array, Array<'env>, Object<'env>>;

/// Converts a decoded `DecodedObject` into a tagged JS descriptor object for JS hydration into `PklObject`.
pub fn decoded_object_to_js<'env>(env: &'env Env, obj: &DecodedObject) -> Result<Object<'env>> {
    let mut out = Object::new(env)?;
    out.set("__pklTag", "object")?;
    out.set("className", js_pkl_string(env, &obj.class_name)?)?;
    out.set("moduleUri", js_pkl_string(env, &obj.module_uri)?)?;

    let mut prop_pairs: Vec<Array<'env>> = Vec::new();
    let mut entry_pairs: Vec<Array<'env>> = Vec::new();
    let mut elements: Vec<JsAnyOut<'env>> = Vec::new();

    for member in &obj.members {
        match member.kind {
            ObjectMemberKind::Property => {
                let key = js_pkl_string(env, &member.key.to_java_key_string())?;
                let val = decoded_value_to_js(env, &member.value)?;
                let pair = Array::from_vec(env, vec![Either8::E(key), val])?;
                prop_pairs.push(pair);
            }
            ObjectMemberKind::Entry => {
                let k = decoded_value_to_js(env, &member.key)?;
                let v = decoded_value_to_js(env, &member.value)?;
                let pair = Array::from_vec(env, vec![k, v])?;
                entry_pairs.push(pair);
            }
            ObjectMemberKind::Element => {
                elements.push(decoded_value_to_js(env, &member.value)?);
            }
        }
    }

    out.set("properties", Array::from_vec(env, prop_pairs)?)?;
    out.set("entries", Array::from_vec(env, entry_pairs)?)?;
    out.set("elements", Array::from_vec(env, elements)?)?;
    Ok(out)
}

/// Converts a `DecodedValue` into a lossless JS representation (hydrated into typed classes/Map/Set by JS).
pub fn decoded_value_to_js<'env>(env: &'env Env, value: &DecodedValue) -> Result<JsAnyOut<'env>> {
    match value {
        DecodedValue::Null => Ok(Either8::A(Null)),
        DecodedValue::Bool(b) => Ok(Either8::B(*b)),
        DecodedValue::Int(i) => Ok(Either8::C(BigInt::from(*i))),
        DecodedValue::Float(f) => Ok(Either8::D(*f)),
        DecodedValue::String(s) => Ok(Either8::E(js_pkl_string(env, s)?)),
        DecodedValue::Bytes(bytes) => Ok(Either8::F(Uint8Array::new(bytes.clone()))),
        DecodedValue::List(items) | DecodedValue::Listing(items) => {
            let mut vec = Vec::with_capacity(items.len());
            for item in items {
                vec.push(decoded_value_to_js(env, item)?);
            }
            Ok(Either8::G(Array::from_vec(env, vec)?))
        }
        DecodedValue::Set(items) => {
            let mut vec = Vec::with_capacity(items.len());
            for item in items {
                vec.push(decoded_value_to_js(env, item)?);
            }
            let mut obj = Object::new(env)?;
            obj.set("__pklTag", "set")?;
            obj.set("items", Array::from_vec(env, vec)?)?;
            Ok(Either8::H(obj))
        }
        DecodedValue::Map(entries) | DecodedValue::Mapping(entries) => {
            let mut pairs = Vec::with_capacity(entries.len());
            for (k, v) in entries {
                let jk = decoded_value_to_js(env, k)?;
                let jv = decoded_value_to_js(env, v)?;
                pairs.push(Array::from_vec(env, vec![jk, jv])?);
            }
            let mut obj = Object::new(env)?;
            obj.set("__pklTag", "map")?;
            obj.set("entries", Array::from_vec(env, pairs)?)?;
            Ok(Either8::H(obj))
        }
        DecodedValue::Duration(d) => {
            let mut obj = Object::new(env)?;
            obj.set("__pklTag", "duration")?;
            obj.set("value", d.value())?;
            obj.set("unit", d.unit().symbol())?;
            Ok(Either8::H(obj))
        }
        DecodedValue::DataSize(d) => {
            let mut obj = Object::new(env)?;
            obj.set("__pklTag", "dataSize")?;
            obj.set("value", d.value())?;
            obj.set("unit", d.unit().symbol())?;
            Ok(Either8::H(obj))
        }
        DecodedValue::Regex(pattern) => {
            let mut obj = Object::new(env)?;
            obj.set("__pklTag", "regex")?;
            obj.set("pattern", js_pkl_string(env, pattern)?)?;
            Ok(Either8::H(obj))
        }
        DecodedValue::Pair(first, second) => {
            let mut obj = Object::new(env)?;
            obj.set("__pklTag", "pair")?;
            obj.set("first", decoded_value_to_js(env, first)?)?;
            obj.set("second", decoded_value_to_js(env, second)?)?;
            Ok(Either8::H(obj))
        }
        DecodedValue::Object(decoded_obj) => {
            Ok(Either8::H(decoded_object_to_js(env, decoded_obj)?))
        }
        DecodedValue::IntSeq(seq) => {
            let mut obj = Object::new(env)?;
            obj.set("__pklTag", "intSeq")?;
            obj.set("start", BigInt::from(seq.start))?;
            obj.set("end", BigInt::from(seq.end))?;
            obj.set("step", BigInt::from(seq.step))?;
            Ok(Either8::H(obj))
        }
        DecodedValue::Class {
            qualified_name,
            module_uri,
        } => {
            let mut obj = Object::new(env)?;
            obj.set("__pklTag", "class")?;
            obj.set("name", js_pkl_string(env, qualified_name)?)?;
            obj.set("moduleUri", js_pkl_string(env, module_uri)?)?;
            Ok(Either8::H(obj))
        }
        DecodedValue::TypeAlias {
            qualified_name,
            module_uri,
        } => {
            let mut obj = Object::new(env)?;
            obj.set("__pklTag", "typeAlias")?;
            obj.set("name", js_pkl_string(env, qualified_name)?)?;
            obj.set("moduleUri", js_pkl_string(env, module_uri)?)?;
            Ok(Either8::H(obj))
        }
        DecodedValue::Function => {
            let mut obj = Object::new(env)?;
            obj.set("__pklTag", "function")?;
            Ok(Either8::H(obj))
        }
        DecodedValue::Reference(reference) => {
            let mut path_vec = Vec::with_capacity(reference.path.len());
            for item in &reference.path {
                path_vec.push(decoded_value_to_js(env, item)?);
            }
            let mut obj = Object::new(env)?;
            obj.set("__pklTag", "reference")?;
            obj.set("domain", decoded_value_to_js(env, &reference.domain)?)?;
            obj.set("data", decoded_value_to_js(env, &reference.data)?)?;
            obj.set("path", Array::from_vec(env, path_vec)?)?;
            Ok(Either8::H(obj))
        }
    }
}

/// Converts a dehydrated JS value into a Rust `DecodedValue` with strict 64-bit `BigInt`,
/// owned `Uint8Array`, and lossless UTF-16 validation.
pub fn js_to_decoded_value(value: JsAnyIn<'_>) -> std::result::Result<DecodedValue, NativeError> {
    match value {
        Either8::A(_) => Ok(DecodedValue::Null),
        Either8::B(b) => Ok(DecodedValue::Bool(b)),
        Either8::C(bi) => Ok(DecodedValue::Int(parse_i64_bigint(&bi)?)),
        Either8::D(f) => Ok(DecodedValue::Float(f)),
        Either8::E(s) => Ok(DecodedValue::String(utf16_to_pkl(&s))),
        Either8::F(bytes) => Ok(DecodedValue::Bytes(bytes.to_vec())),
        Either8::G(arr) => {
            let len = arr.len();
            let mut out = Vec::with_capacity(len as usize);
            for i in 0..len {
                let item = arr
                    .get::<JsAnyIn<'_>>(i)
                    .map_err(|e| NativeError::invalid_argument(format!("Invalid array item: {e}")))?
                    .ok_or_else(|| NativeError::invalid_argument("Missing array element"))?;
                out.push(js_to_decoded_value(item)?);
            }
            Ok(DecodedValue::List(out))
        }
        Either8::H(obj) => {
            let tag = obj
                .get::<String>("__pklTag")
                .map_err(|e| NativeError::invalid_argument(format!("Invalid __pklTag: {e}")))?;
            match tag.as_deref() {
                Some("set") => {
                    let items_arr = get_required_array(&obj, "items")?;
                    let mut out = Vec::with_capacity(items_arr.len() as usize);
                    for i in 0..items_arr.len() {
                        let item = items_arr
                            .get::<JsAnyIn<'_>>(i)
                            .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                            .ok_or_else(|| NativeError::invalid_argument("Missing set item"))?;
                        out.push(js_to_decoded_value(item)?);
                    }
                    Ok(DecodedValue::Set(out))
                }
                Some("map") => {
                    let entries_arr = get_required_array(&obj, "entries")?;
                    let mut out = Vec::with_capacity(entries_arr.len() as usize);
                    for i in 0..entries_arr.len() {
                        let pair = entries_arr
                            .get::<Array<'_>>(i)
                            .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                            .ok_or_else(|| NativeError::invalid_argument("Missing map entry"))?;
                        let k = pair
                            .get::<JsAnyIn<'_>>(0)
                            .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                            .ok_or_else(|| NativeError::invalid_argument("Missing map key"))?;
                        let v = pair
                            .get::<JsAnyIn<'_>>(1)
                            .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                            .ok_or_else(|| NativeError::invalid_argument("Missing map value"))?;
                        out.push((js_to_decoded_value(k)?, js_to_decoded_value(v)?));
                    }
                    Ok(DecodedValue::Map(out))
                }
                Some("duration") => {
                    let val = obj
                        .get::<f64>("value")
                        .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                        .ok_or_else(|| NativeError::invalid_argument("Missing duration.value"))?;
                    let unit_raw = obj
                        .get::<Utf16String>("unit")
                        .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                        .ok_or_else(|| NativeError::invalid_argument("Missing duration.unit"))?;
                    let unit_str = utf16_to_string(&unit_raw)?;
                    let unit = DurationUnit::parse(&unit_str).ok_or_else(|| {
                        NativeError::invalid_argument(format!(
                            "Invalid duration unit `{unit_str}`."
                        ))
                    })?;
                    Ok(DecodedValue::Duration(Duration::new(val, unit)))
                }
                Some("dataSize") => {
                    let val = obj
                        .get::<f64>("value")
                        .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                        .ok_or_else(|| NativeError::invalid_argument("Missing dataSize.value"))?;
                    let unit_raw = obj
                        .get::<Utf16String>("unit")
                        .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                        .ok_or_else(|| NativeError::invalid_argument("Missing dataSize.unit"))?;
                    let unit_str = utf16_to_string(&unit_raw)?;
                    let unit = DataSizeUnit::parse(&unit_str).ok_or_else(|| {
                        NativeError::invalid_argument(format!(
                            "Invalid data size unit `{unit_str}`."
                        ))
                    })?;
                    Ok(DecodedValue::DataSize(DataSize::new(val, unit)))
                }
                Some("regex") => {
                    let pat = obj
                        .get::<Utf16String>("pattern")
                        .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                        .ok_or_else(|| NativeError::invalid_argument("Missing regex.pattern"))?;
                    Ok(DecodedValue::Regex(utf16_to_pkl(&pat)))
                }
                Some("pair") => {
                    let first = obj
                        .get::<JsAnyIn<'_>>("first")
                        .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                        .ok_or_else(|| NativeError::invalid_argument("Missing pair.first"))?;
                    let second = obj
                        .get::<JsAnyIn<'_>>("second")
                        .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                        .ok_or_else(|| NativeError::invalid_argument("Missing pair.second"))?;
                    Ok(DecodedValue::Pair(
                        Box::new(js_to_decoded_value(first)?),
                        Box::new(js_to_decoded_value(second)?),
                    ))
                }
                Some("object") => {
                    let class_name_raw = obj
                        .get::<Utf16String>("className")
                        .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                        .ok_or_else(|| NativeError::invalid_argument("Missing object.className"))?;
                    let module_uri_raw = obj
                        .get::<Utf16String>("moduleUri")
                        .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                        .ok_or_else(|| NativeError::invalid_argument("Missing object.moduleUri"))?;
                    let class_name = utf16_to_pkl(&class_name_raw);
                    let module_uri = utf16_to_pkl(&module_uri_raw);
                    let class_info = DecodedClassInfo::from_wire(&class_name, &module_uri);
                    let mut members = Vec::new();

                    let props = get_required_array(&obj, "properties")?;
                    for i in 0..props.len() {
                        let pair = props
                            .get::<Array<'_>>(i)
                            .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                            .ok_or_else(|| {
                                NativeError::invalid_argument("Missing property pair")
                            })?;
                        let k = pair
                            .get::<Utf16String>(0)
                            .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                            .ok_or_else(|| NativeError::invalid_argument("Missing property key"))?;
                        let v = pair
                            .get::<JsAnyIn<'_>>(1)
                            .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                            .ok_or_else(|| {
                                NativeError::invalid_argument("Missing property value")
                            })?;
                        members.push(DecodedObjectMember {
                            kind: ObjectMemberKind::Property,
                            key: DecodedValue::String(utf16_to_pkl(&k)),
                            value: js_to_decoded_value(v)?,
                        });
                    }
                    let entries = get_required_array(&obj, "entries")?;
                    for i in 0..entries.len() {
                        let pair = entries
                            .get::<Array<'_>>(i)
                            .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                            .ok_or_else(|| NativeError::invalid_argument("Missing entry pair"))?;
                        let k = pair
                            .get::<JsAnyIn<'_>>(0)
                            .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                            .ok_or_else(|| NativeError::invalid_argument("Missing entry key"))?;
                        let v = pair
                            .get::<JsAnyIn<'_>>(1)
                            .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                            .ok_or_else(|| NativeError::invalid_argument("Missing entry value"))?;
                        members.push(DecodedObjectMember {
                            kind: ObjectMemberKind::Entry,
                            key: js_to_decoded_value(k)?,
                            value: js_to_decoded_value(v)?,
                        });
                    }
                    let elements = get_required_array(&obj, "elements")?;
                    for i in 0..elements.len() {
                        let v = elements
                            .get::<JsAnyIn<'_>>(i)
                            .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                            .ok_or_else(|| NativeError::invalid_argument("Missing element"))?;
                        members.push(DecodedObjectMember {
                            kind: ObjectMemberKind::Element,
                            key: DecodedValue::Int(i as i64),
                            value: js_to_decoded_value(v)?,
                        });
                    }

                    Ok(DecodedValue::Object(DecodedObject {
                        class_name,
                        module_uri,
                        class_info,
                        members,
                    }))
                }
                Some("intSeq") => {
                    let start = parse_i64_bigint(
                        &obj.get::<BigInt>("start")
                            .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                            .ok_or_else(|| NativeError::invalid_argument("Missing intSeq.start"))?,
                    )?;
                    let end = parse_i64_bigint(
                        &obj.get::<BigInt>("end")
                            .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                            .ok_or_else(|| NativeError::invalid_argument("Missing intSeq.end"))?,
                    )?;
                    let step = parse_i64_bigint(
                        &obj.get::<BigInt>("step")
                            .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                            .ok_or_else(|| NativeError::invalid_argument("Missing intSeq.step"))?,
                    )?;
                    Ok(DecodedValue::IntSeq(DecodedIntSeq { start, end, step }))
                }
                Some("class") => {
                    let name = obj
                        .get::<Utf16String>("name")
                        .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                        .ok_or_else(|| NativeError::invalid_argument("Missing class.name"))?;
                    let uri = obj
                        .get::<Utf16String>("moduleUri")
                        .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                        .ok_or_else(|| NativeError::invalid_argument("Missing class.moduleUri"))?;
                    Ok(DecodedValue::Class {
                        qualified_name: utf16_to_pkl(&name),
                        module_uri: utf16_to_pkl(&uri),
                    })
                }
                Some("typeAlias") => {
                    let name = obj
                        .get::<Utf16String>("name")
                        .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                        .ok_or_else(|| NativeError::invalid_argument("Missing typeAlias.name"))?;
                    let uri = obj
                        .get::<Utf16String>("moduleUri")
                        .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                        .ok_or_else(|| {
                            NativeError::invalid_argument("Missing typeAlias.moduleUri")
                        })?;
                    Ok(DecodedValue::TypeAlias {
                        qualified_name: utf16_to_pkl(&name),
                        module_uri: utf16_to_pkl(&uri),
                    })
                }
                Some("function") => Ok(DecodedValue::Function),
                Some("reference") => {
                    let domain = obj
                        .get::<JsAnyIn<'_>>("domain")
                        .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                        .ok_or_else(|| NativeError::invalid_argument("Missing reference.domain"))?;
                    let data = obj
                        .get::<JsAnyIn<'_>>("data")
                        .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                        .ok_or_else(|| NativeError::invalid_argument("Missing reference.data"))?;
                    let path_arr = get_required_array(&obj, "path")?;
                    let mut path = Vec::with_capacity(path_arr.len() as usize);
                    for i in 0..path_arr.len() {
                        let item = path_arr
                            .get::<JsAnyIn<'_>>(i)
                            .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                            .ok_or_else(|| {
                                NativeError::invalid_argument("Missing reference path item")
                            })?;
                        path.push(js_to_decoded_value(item)?);
                    }
                    Ok(DecodedValue::Reference(DecodedReference {
                        domain: Box::new(js_to_decoded_value(domain)?),
                        data: Box::new(js_to_decoded_value(data)?),
                        path,
                    }))
                }
                Some(other) => Err(NativeError::invalid_argument(format!(
                    "Unknown Pkl value tag `{other}`."
                ))),
                None => {
                    // Plain JS object -> Dynamic PklObject with string properties
                    let keys = Object::keys(&obj)
                        .map_err(|e| NativeError::invalid_argument(e.to_string()))?;
                    let mut members = Vec::with_capacity(keys.len());
                    for k in keys {
                        if let Some(val) = obj
                            .get::<JsAnyIn<'_>>(&k)
                            .map_err(|e| NativeError::invalid_argument(e.to_string()))?
                        {
                            members.push(DecodedObjectMember {
                                kind: ObjectMemberKind::Property,
                                key: DecodedValue::String(PklString::from(k.as_str())),
                                value: js_to_decoded_value(val)?,
                            });
                        }
                    }
                    let class_name = PklString::from("Dynamic");
                    let module_uri = PklString::from("pkl:base");
                    let class_info = DecodedClassInfo::from_wire(&class_name, &module_uri);
                    Ok(DecodedValue::Object(DecodedObject {
                        class_name,
                        module_uri,
                        class_info,
                        members,
                    }))
                }
            }
        }
    }
}

fn get_required_array<'env>(
    obj: &Object<'env>,
    field: &str,
) -> std::result::Result<Array<'env>, NativeError> {
    obj.get::<Array<'env>>(field)
        .map_err(|e| NativeError::invalid_argument(format!("Invalid field `{field}`: {e}")))?
        .ok_or_else(|| NativeError::invalid_argument(format!("Missing field `{field}`.")))
}

#[napi]
pub fn product_decode_pkl_binary<'env>(env: &'env Env, bytes: Uint8Array) -> Result<Object<'env>> {
    let raw = bytes.to_vec();
    match decode_lossless(&raw) {
        Ok(decoded) => {
            let js_val = decoded_value_to_js(env, &decoded)?;
            to_js_outcome(env, Ok(js_val))
        }
        Err(err) => to_js_outcome::<Null>(
            env,
            Err(NativeError::decode(format!(
                "Failed to decode pkl-binary: {err}"
            ))),
        ),
    }
}

#[napi]
pub fn product_roundtrip_value<'env>(env: &'env Env, input: JsAnyIn<'env>) -> Result<Object<'env>> {
    match js_to_decoded_value(input) {
        Ok(decoded) => {
            let js_val = decoded_value_to_js(env, &decoded)?;
            to_js_outcome(env, Ok(js_val))
        }
        Err(err) => to_js_outcome::<Null>(env, Err(err)),
    }
}

#[napi]
pub fn product_roundtrip_integer<'env>(env: &'env Env, input: BigInt) -> Result<Object<'env>> {
    match parse_i64_bigint(&input) {
        Ok(i) => to_js_outcome(env, Ok(BigInt::from(i))),
        Err(err) => to_js_outcome::<Null>(env, Err(err)),
    }
}

#[napi]
pub fn product_measure_conversion<'env>(
    env: &'env Env,
    input: JsAnyIn<'env>,
    iterations: f64,
) -> Result<Object<'env>> {
    if !iterations.is_finite()
        || iterations.fract() != 0.0
        || !(1.0..=1_000_000.0).contains(&iterations)
    {
        return to_js_outcome::<Null>(
            env,
            Err(NativeError::invalid_argument(
                "iterations must be an integer between 1 and 1000000.",
            )),
        );
    }
    let count = iterations as u32;
    let decoded = match js_to_decoded_value(input) {
        Ok(v) => v,
        Err(err) => return to_js_outcome::<Null>(env, Err(err)),
    };
    let start = Instant::now();
    let mut last_out: Option<JsAnyOut<'env>> = None;
    for _ in 0..count {
        let js_val = decoded_value_to_js(env, &decoded)?;
        last_out = Some(js_val);
    }
    let elapsed_secs = start.elapsed().as_secs_f64();
    let elapsed_ms = elapsed_secs * 1000.0;
    let per_iter_us = (elapsed_secs * 1_000_000.0) / f64::from(count);
    let mut report = Object::new(env)?;
    report.set("iterations", count)?;
    report.set("elapsedMs", elapsed_ms)?;
    report.set("perIterationUs", per_iter_us)?;
    if let Some(val) = last_out {
        report.set("lastValue", val)?;
    }
    to_js_outcome(env, Ok(report))
}

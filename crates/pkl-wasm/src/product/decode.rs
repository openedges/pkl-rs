//! Lossless `pkl-binary` decoding and Rust <-> JS tagged value marshaling.

use super::util::{
    StructuredError, bytes_from_js, get_prop, js_from_bytes, js_string_from_pkl, parse_i64_js,
    pkl_string_from_js, rust_string_from_js, set_prop,
};
use js_sys::{Array, BigInt, Object, Uint8Array};
use pkl_runtime::{
    data_size::{DataSize, DataSizeUnit},
    duration::{Duration, DurationUnit},
    pkl_binary_decode::{
        DecodedClassInfo, DecodedIntSeq, DecodedObject, DecodedObjectMember, DecodedReference,
        DecodedValue, ObjectMemberKind, decode as decode_strict, decode_lossless,
    },
};
use wasm_bindgen::{JsCast, JsValue};

/// Decodes raw `pkl-binary` MessagePack bytes into a JS descriptor tree consumed by
/// `packages/pkl-wasm`'s `hydrateValue` to construct real `PklObject`, `PklDuration`,
/// `Map`, `Set`, `BigInt`, owned `Uint8Array`, etc.
pub fn decode_pkl_binary_to_js(bytes: &[u8], strict: bool) -> Result<JsValue, StructuredError> {
    let decoded = if strict {
        decode_strict(bytes)
    } else {
        decode_lossless(bytes)
    }
    .map_err(|err| StructuredError::decode(err.to_string()))?;
    decoded_value_to_js(&decoded)
        .map_err(|_| StructuredError::decode("failed to construct JS value from decoded payload"))
}

/// Converts a `DecodedValue` into a lossless JS descriptor tree.
pub fn decoded_value_to_js(value: &DecodedValue) -> Result<JsValue, JsValue> {
    match value {
        DecodedValue::Null => Ok(JsValue::NULL),
        DecodedValue::Bool(b) => Ok(JsValue::from_bool(*b)),
        DecodedValue::Int(n) => Ok(BigInt::from(*n).into()),
        DecodedValue::Float(f) => Ok(JsValue::from_f64(*f)),
        DecodedValue::String(s) => Ok(js_string_from_pkl(s).into()),
        DecodedValue::Bytes(bytes) => Ok(js_from_bytes(bytes).into()),
        DecodedValue::Duration(d) => {
            let obj = Object::new();
            set_prop(&obj, "__pklTag", JsValue::from_str("duration"))?;
            set_prop(&obj, "value", JsValue::from_f64(d.value()))?;
            set_prop(&obj, "unit", JsValue::from_str(d.unit().symbol()))?;
            Ok(obj.into())
        }
        DecodedValue::DataSize(d) => {
            let obj = Object::new();
            set_prop(&obj, "__pklTag", JsValue::from_str("dataSize"))?;
            set_prop(&obj, "value", JsValue::from_f64(d.value()))?;
            set_prop(&obj, "unit", JsValue::from_str(d.unit().symbol()))?;
            Ok(obj.into())
        }
        DecodedValue::Pair(first, second) => {
            let obj = Object::new();
            set_prop(&obj, "__pklTag", JsValue::from_str("pair"))?;
            set_prop(&obj, "first", decoded_value_to_js(first)?)?;
            set_prop(&obj, "second", decoded_value_to_js(second)?)?;
            Ok(obj.into())
        }
        DecodedValue::Regex(pattern) => {
            let obj = Object::new();
            set_prop(&obj, "__pklTag", JsValue::from_str("regex"))?;
            set_prop(&obj, "pattern", js_string_from_pkl(pattern).into())?;
            Ok(obj.into())
        }
        DecodedValue::List(items) | DecodedValue::Listing(items) => {
            let arr = Array::new_with_length(items.len() as u32);
            for (idx, item) in items.iter().enumerate() {
                arr.set(idx as u32, decoded_value_to_js(item)?);
            }
            Ok(arr.into())
        }
        DecodedValue::Set(items) => {
            let obj = Object::new();
            set_prop(&obj, "__pklTag", JsValue::from_str("set"))?;
            let arr = Array::new_with_length(items.len() as u32);
            for (idx, item) in items.iter().enumerate() {
                arr.set(idx as u32, decoded_value_to_js(item)?);
            }
            set_prop(&obj, "items", arr.into())?;
            Ok(obj.into())
        }
        DecodedValue::Map(entries) | DecodedValue::Mapping(entries) => {
            let obj = Object::new();
            set_prop(&obj, "__pklTag", JsValue::from_str("map"))?;
            let arr = Array::new_with_length(entries.len() as u32);
            for (idx, (k, v)) in entries.iter().enumerate() {
                let pair = Array::new_with_length(2);
                pair.set(0, decoded_value_to_js(k)?);
                pair.set(1, decoded_value_to_js(v)?);
                arr.set(idx as u32, pair.into());
            }
            set_prop(&obj, "entries", arr.into())?;
            Ok(obj.into())
        }
        DecodedValue::Object(obj) => decoded_object_to_js(obj),
        DecodedValue::IntSeq(seq) => {
            let obj = Object::new();
            set_prop(&obj, "__pklTag", JsValue::from_str("intSeq"))?;
            set_prop(&obj, "start", BigInt::from(seq.start).into())?;
            set_prop(&obj, "end", BigInt::from(seq.end).into())?;
            set_prop(&obj, "step", BigInt::from(seq.step).into())?;
            Ok(obj.into())
        }
        DecodedValue::Class {
            qualified_name,
            module_uri,
        } => {
            let obj = Object::new();
            set_prop(&obj, "__pklTag", JsValue::from_str("class"))?;
            set_prop(&obj, "name", js_string_from_pkl(qualified_name).into())?;
            set_prop(&obj, "moduleUri", js_string_from_pkl(module_uri).into())?;
            Ok(obj.into())
        }
        DecodedValue::TypeAlias {
            qualified_name,
            module_uri,
        } => {
            let obj = Object::new();
            set_prop(&obj, "__pklTag", JsValue::from_str("typeAlias"))?;
            set_prop(&obj, "name", js_string_from_pkl(qualified_name).into())?;
            set_prop(&obj, "moduleUri", js_string_from_pkl(module_uri).into())?;
            Ok(obj.into())
        }
        DecodedValue::Function => {
            let obj = Object::new();
            set_prop(&obj, "__pklTag", JsValue::from_str("function"))?;
            Ok(obj.into())
        }
        DecodedValue::Reference(reference) => {
            let obj = Object::new();
            set_prop(&obj, "__pklTag", JsValue::from_str("reference"))?;
            set_prop(&obj, "domain", decoded_value_to_js(&reference.domain)?)?;
            set_prop(&obj, "data", decoded_value_to_js(&reference.data)?)?;
            let path = Array::new_with_length(reference.path.len() as u32);
            for (idx, item) in reference.path.iter().enumerate() {
                path.set(idx as u32, decoded_value_to_js(item)?);
            }
            set_prop(&obj, "path", path.into())?;
            Ok(obj.into())
        }
    }
}

pub fn decoded_object_to_js(decoded: &DecodedObject) -> Result<JsValue, JsValue> {
    let obj = Object::new();
    set_prop(&obj, "__pklTag", JsValue::from_str("object"))?;
    set_prop(
        &obj,
        "className",
        js_string_from_pkl(&decoded.class_name).into(),
    )?;
    set_prop(
        &obj,
        "moduleUri",
        js_string_from_pkl(&decoded.module_uri).into(),
    )?;

    let properties = Array::new();
    let entries = Array::new();
    let elements = Array::new();
    for member in &decoded.members {
        match member.kind {
            ObjectMemberKind::Property => {
                let pair = Array::new_with_length(2);
                pair.set(
                    0,
                    js_string_from_pkl(&member.key.to_java_key_string()).into(),
                );
                pair.set(1, decoded_value_to_js(&member.value)?);
                properties.push(&pair);
            }
            ObjectMemberKind::Entry => {
                let pair = Array::new_with_length(2);
                pair.set(0, decoded_value_to_js(&member.key)?);
                pair.set(1, decoded_value_to_js(&member.value)?);
                entries.push(&pair);
            }
            ObjectMemberKind::Element => {
                elements.push(&decoded_value_to_js(&member.value)?);
            }
        }
    }
    set_prop(&obj, "properties", properties.into())?;
    set_prop(&obj, "entries", entries.into())?;
    set_prop(&obj, "elements", elements.into())?;
    Ok(obj.into())
}

/// Converts a JS descriptor tree into a Rust `DecodedValue`, enforcing exact `i64` BigInt bounds,
/// owned `Uint8Array` bytes, and lossless UTF-16 strings.
pub fn js_to_decoded_value(value: &JsValue) -> Result<DecodedValue, StructuredError> {
    if value.is_null() || value.is_undefined() {
        return Ok(DecodedValue::Null);
    }
    if let Some(b) = value.as_bool() {
        return Ok(DecodedValue::Bool(b));
    }
    if value.is_bigint() {
        return Ok(DecodedValue::Int(parse_i64_js(value)?));
    }
    if let Some(f) = value.as_f64() {
        return Ok(DecodedValue::Float(f));
    }
    if value.is_string() {
        return Ok(DecodedValue::String(pkl_string_from_js(value)?));
    }
    if value.is_instance_of::<Uint8Array>() {
        return Ok(DecodedValue::Bytes(bytes_from_js(value)?));
    }
    if Array::is_array(value) {
        let arr = Array::from(value);
        let mut items = Vec::with_capacity(arr.length() as usize);
        for item in arr.iter() {
            items.push(js_to_decoded_value(&item)?);
        }
        return Ok(DecodedValue::List(items));
    }
    if value.is_object() {
        let tag_val = get_prop(value, "__pklTag")?;
        if tag_val.is_string() {
            let tag = rust_string_from_js(&tag_val)?;
            return match tag.as_str() {
                "duration" => {
                    let num = get_prop(value, "value")?.as_f64().ok_or_else(|| {
                        StructuredError::invalid_argument("Duration.value must be number")
                    })?;
                    let unit_str = rust_string_from_js(&get_prop(value, "unit")?)?;
                    let unit = DurationUnit::parse(&unit_str).ok_or_else(|| {
                        StructuredError::invalid_argument(format!(
                            "unknown Duration unit `{unit_str}`"
                        ))
                    })?;
                    Ok(DecodedValue::Duration(Duration::new(num, unit)))
                }
                "dataSize" => {
                    let num = get_prop(value, "value")?.as_f64().ok_or_else(|| {
                        StructuredError::invalid_argument("DataSize.value must be number")
                    })?;
                    let unit_str = rust_string_from_js(&get_prop(value, "unit")?)?;
                    let unit = DataSizeUnit::parse(&unit_str).ok_or_else(|| {
                        StructuredError::invalid_argument(format!(
                            "unknown DataSize unit `{unit_str}`"
                        ))
                    })?;
                    Ok(DecodedValue::DataSize(DataSize::new(num, unit)))
                }
                "pair" => {
                    let first = js_to_decoded_value(&get_prop(value, "first")?)?;
                    let second = js_to_decoded_value(&get_prop(value, "second")?)?;
                    Ok(DecodedValue::Pair(Box::new(first), Box::new(second)))
                }
                "regex" => {
                    let pattern = pkl_string_from_js(&get_prop(value, "pattern")?)?;
                    Ok(DecodedValue::Regex(pattern))
                }
                "set" => {
                    let items_val = get_prop(value, "items")?;
                    if !Array::is_array(&items_val) {
                        return Err(StructuredError::invalid_argument("Set.items must be array"));
                    }
                    let arr = Array::from(&items_val);
                    let mut items = Vec::with_capacity(arr.length() as usize);
                    for item in arr.iter() {
                        items.push(js_to_decoded_value(&item)?);
                    }
                    Ok(DecodedValue::Set(items))
                }
                "map" => {
                    let entries_val = get_prop(value, "entries")?;
                    if !Array::is_array(&entries_val) {
                        return Err(StructuredError::invalid_argument(
                            "Map.entries must be array",
                        ));
                    }
                    let arr = Array::from(&entries_val);
                    let mut entries = Vec::with_capacity(arr.length() as usize);
                    for pair_val in arr.iter() {
                        if !Array::is_array(&pair_val) {
                            return Err(StructuredError::invalid_argument(
                                "Map entry must be [key, value]",
                            ));
                        }
                        let pair = Array::from(&pair_val);
                        if pair.length() != 2 {
                            return Err(StructuredError::invalid_argument(
                                "Map entry must have length 2",
                            ));
                        }
                        entries.push((
                            js_to_decoded_value(&pair.get(0))?,
                            js_to_decoded_value(&pair.get(1))?,
                        ));
                    }
                    Ok(DecodedValue::Map(entries))
                }
                "object" => {
                    let class_name = pkl_string_from_js(&get_prop(value, "className")?)?;
                    let module_uri = pkl_string_from_js(&get_prop(value, "moduleUri")?)?;
                    let class_info = DecodedClassInfo::from_wire(&class_name, &module_uri);
                    let mut members = Vec::new();
                    let props_val = get_prop(value, "properties")?;
                    if Array::is_array(&props_val) {
                        for pair_val in Array::from(&props_val).iter() {
                            let pair = Array::from(&pair_val);
                            let k = pkl_string_from_js(&pair.get(0))?;
                            let v = js_to_decoded_value(&pair.get(1))?;
                            members.push(DecodedObjectMember {
                                kind: ObjectMemberKind::Property,
                                key: DecodedValue::String(k),
                                value: v,
                            });
                        }
                    }
                    let entries_val = get_prop(value, "entries")?;
                    if Array::is_array(&entries_val) {
                        for pair_val in Array::from(&entries_val).iter() {
                            let pair = Array::from(&pair_val);
                            members.push(DecodedObjectMember {
                                kind: ObjectMemberKind::Entry,
                                key: js_to_decoded_value(&pair.get(0))?,
                                value: js_to_decoded_value(&pair.get(1))?,
                            });
                        }
                    }
                    let elems_val = get_prop(value, "elements")?;
                    if Array::is_array(&elems_val) {
                        for (idx, elem_val) in Array::from(&elems_val).iter().enumerate() {
                            members.push(DecodedObjectMember {
                                kind: ObjectMemberKind::Element,
                                key: DecodedValue::Int(idx as i64),
                                value: js_to_decoded_value(&elem_val)?,
                            });
                        }
                    }
                    Ok(DecodedValue::Object(DecodedObject {
                        class_name,
                        module_uri,
                        class_info,
                        members,
                    }))
                }
                "intSeq" => Ok(DecodedValue::IntSeq(DecodedIntSeq {
                    start: parse_i64_js(&get_prop(value, "start")?)?,
                    end: parse_i64_js(&get_prop(value, "end")?)?,
                    step: parse_i64_js(&get_prop(value, "step")?)?,
                })),
                "class" => Ok(DecodedValue::Class {
                    qualified_name: pkl_string_from_js(&get_prop(value, "name")?)?,
                    module_uri: pkl_string_from_js(&get_prop(value, "moduleUri")?)?,
                }),
                "typeAlias" => Ok(DecodedValue::TypeAlias {
                    qualified_name: pkl_string_from_js(&get_prop(value, "name")?)?,
                    module_uri: pkl_string_from_js(&get_prop(value, "moduleUri")?)?,
                }),
                "function" => Ok(DecodedValue::Function),
                "reference" => {
                    let domain = js_to_decoded_value(&get_prop(value, "domain")?)?;
                    let data = js_to_decoded_value(&get_prop(value, "data")?)?;
                    let path_val = get_prop(value, "path")?;
                    let mut path = Vec::new();
                    if Array::is_array(&path_val) {
                        for item in Array::from(&path_val).iter() {
                            path.push(js_to_decoded_value(&item)?);
                        }
                    }
                    Ok(DecodedValue::Reference(DecodedReference {
                        domain: Box::new(domain),
                        data: Box::new(data),
                        path,
                    }))
                }
                other => Err(StructuredError::invalid_argument(format!(
                    "unsupported Pkl value tag `{other}`"
                ))),
            };
        }
    }
    Err(StructuredError::invalid_argument(
        "unsupported value type for Pkl conversion",
    ))
}

/// Round-trips a JS descriptor tree through Rust `DecodedValue` and back to JS losslessly.
pub fn roundtrip_value_js(value: &JsValue) -> Result<JsValue, StructuredError> {
    let rust_val = js_to_decoded_value(value)?;
    decoded_value_to_js(&rust_val)
        .map_err(|_| StructuredError::invalid_argument("failed to convert round-tripped value"))
}

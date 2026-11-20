//! Rust `PklBinaryDecoder` differential probe matching `tools/java/M3PklBinaryDecodeProbe.java`.
//!
//! Emits identical canonical lossless JSON on stdout (exit 0) for valid `pkl-binary`
//! streams, or `error: <outer>\ncause: <inner>\n` on stderr (exit 1) for decode errors.

#![forbid(unsafe_code)]

use pkl_runtime::pkl_binary_decode::{DecodedClassInfo, DecodedValue, PklBinaryDecoder};
use pkl_runtime::value::PklString;
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;

fn append_json_string_utf16(out: &mut String, units: &[u16]) {
    out.push('"');
    for &unit in units {
        match unit {
            0x0022 => out.push_str("\\\""),
            0x005C => out.push_str("\\\\"),
            0x0008 => out.push_str("\\b"),
            0x000C => out.push_str("\\f"),
            0x000A => out.push_str("\\n"),
            0x000D => out.push_str("\\r"),
            0x0009 => out.push_str("\\t"),
            u if u < 0x0020 || (0xD800..=0xDFFF).contains(&u) => {
                let _ = write!(out, "\\u{u:04x}");
            }
            u => {
                if let Some(ch) = char::from_u32(u32::from(u)) {
                    out.push(ch);
                } else {
                    let _ = write!(out, "\\u{u:04x}");
                }
            }
        }
    }
    out.push('"');
}

fn append_json_pkl_string(out: &mut String, value: &PklString) {
    let units: Vec<u16> = value.encode_utf16().collect();
    append_json_string_utf16(out, &units);
}

fn append_json_str(out: &mut String, value: &str) {
    let units: Vec<u16> = value.encode_utf16().collect();
    append_json_string_utf16(out, &units);
}

fn append_hex_bytes(out: &mut String, bytes: &[u8]) {
    for &b in bytes {
        let _ = write!(out, "{b:02x}");
    }
}

fn append_utf16_hex(out: &mut String, value: &PklString) {
    for unit in value.encode_utf16() {
        let _ = write!(out, "{unit:04x}");
    }
}

fn append_class_info(out: &mut String, info: &DecodedClassInfo) {
    out.push_str("{\"moduleName\":");
    append_json_pkl_string(out, &info.module_name);
    out.push_str(",\"simpleName\":");
    append_json_pkl_string(out, &info.simple_name);
    out.push_str(",\"moduleUri\":");
    append_json_pkl_string(out, &info.module_uri);
    out.push('}');
}

fn append_value(out: &mut String, value: &DecodedValue) -> Result<(), String> {
    match value {
        DecodedValue::Null => out.push_str("{\"type\":\"null\"}"),
        DecodedValue::Bool(b) => {
            out.push_str("{\"type\":\"bool\",\"value\":");
            out.push_str(if *b { "true" } else { "false" });
            out.push('}');
        }
        DecodedValue::Int(n) => {
            let _ = write!(out, "{{\"type\":\"int\",\"value\":{n}}}");
        }
        DecodedValue::Float(f) => {
            let _ = write!(
                out,
                "{{\"type\":\"float\",\"bits\":\"{:016x}\"}}",
                f.to_bits()
            );
        }
        DecodedValue::String(s) => {
            out.push_str("{\"type\":\"string\",\"utf16_hex\":\"");
            append_utf16_hex(out, s);
            out.push_str("\",\"text\":");
            append_json_pkl_string(out, s);
            out.push('}');
        }
        DecodedValue::Bytes(bytes) => {
            out.push_str("{\"type\":\"bytes\",\"hex\":\"");
            append_hex_bytes(out, bytes);
            out.push_str("\"}");
        }
        DecodedValue::Duration(dur) => {
            let _ = write!(
                out,
                "{{\"type\":\"duration\",\"bits\":\"{:016x}\",\"unit\":",
                dur.value().to_bits()
            );
            append_json_str(out, dur.unit().symbol());
            out.push('}');
        }
        DecodedValue::DataSize(ds) => {
            let _ = write!(
                out,
                "{{\"type\":\"datasize\",\"bits\":\"{:016x}\",\"unit\":",
                ds.value().to_bits()
            );
            append_json_str(out, ds.unit().symbol());
            out.push('}');
        }
        DecodedValue::Regex(pattern) => {
            out.push_str("{\"type\":\"regex\",\"pattern\":");
            append_json_pkl_string(out, pattern);
            out.push('}');
        }
        DecodedValue::Pair(first, second) => {
            out.push_str("{\"type\":\"pair\",\"first\":");
            append_value(out, first)?;
            out.push_str(",\"second\":");
            append_value(out, second)?;
            out.push('}');
        }
        DecodedValue::List(items) | DecodedValue::Listing(items) => {
            out.push_str("{\"type\":\"list\",\"elements\":[");
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                append_value(out, item)?;
            }
            out.push_str("]}");
        }
        DecodedValue::Set(items) => {
            out.push_str("{\"type\":\"set\",\"elements\":[");
            for (idx, item) in items.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                append_value(out, item)?;
            }
            out.push_str("]}");
        }
        DecodedValue::Map(entries) | DecodedValue::Mapping(entries) => {
            out.push_str("{\"type\":\"map\",\"entries\":[");
            for (idx, (k, v)) in entries.iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                out.push('[');
                append_value(out, k)?;
                out.push(',');
                append_value(out, v)?;
                out.push(']');
            }
            out.push_str("]}");
        }
        DecodedValue::Object(obj) => {
            if obj.class_info.is_module {
                out.push_str("{\"type\":\"module\",\"uri\":");
                append_json_pkl_string(out, &obj.module_uri);
                out.push_str(",\"name\":");
                append_json_pkl_string(out, &obj.class_name);
                out.push_str(",\"class\":");
                append_class_info(out, &obj.class_info);
                out.push_str(",\"properties\":[");
            } else {
                out.push_str("{\"type\":\"object\",\"class\":");
                append_class_info(out, &obj.class_info);
                out.push_str(",\"properties\":[");
            }
            for (idx, (key, val)) in obj.properties().into_iter().enumerate() {
                if idx > 0 {
                    out.push(',');
                }
                out.push('[');
                append_json_pkl_string(out, &key);
                out.push(',');
                append_value(out, val)?;
                out.push(']');
            }
            out.push_str("]}");
        }
        other => {
            return Err(format!(
                "Unexpected decoded value in strict mode: {other:?}"
            ));
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 1 {
        eprintln!("usage: m3_pklbinary_decode_probe <msgpack-file>");
        std::process::exit(2);
    }
    let bytes = match fs::read(&args[0]) {
        Ok(b) => b,
        Err(err) => {
            eprintln!("failed to read {}: {err}", args[0]);
            std::process::exit(2);
        }
    };
    match PklBinaryDecoder::decode(&bytes) {
        Ok(value) => {
            let mut out = String::new();
            if let Err(err) = append_value(&mut out, &value) {
                eprintln!("{err}");
                std::process::exit(2);
            }
            out.push('\n');
            if let Err(err) = std::io::stdout().write_all(out.as_bytes()) {
                eprintln!("stdout write failed: {err}");
                std::process::exit(2);
            }
        }
        Err(err) => {
            let payload = format!("error: {}\ncause: {}\n", err.message, err.cause);
            let _ = std::io::stderr().write_all(payload.as_bytes());
            std::process::exit(1);
        }
    }
}

//! ResourceReader values and evaluator-local successful/absent read cache.
use super::*;
use crate::resources::{ReadResult, Resource};

pub(crate) const RESOURCE: usize = usize::MAX - 300;
const FACTORY: usize = RESOURCE - 1;

#[derive(Clone, Copy, Debug)]
pub enum Property {
    Uri,
    Text,
    Bytes,
}

#[cfg_attr(target_arch = "wasm32", inline(never))]
pub(super) fn builtin_members(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<usize, Vec<Member>> {
    let source = crate::stdlib_sources::base();
    let class_start = source.text.find("class Resource {").unwrap();
    let mut definitions = BTreeMap::new();
    for factory in [false, true] {
        let mut members = Vec::new();
        for (name, ty, hidden, fixed) in [
            ("uri", TypeCheck::String, false, false),
            ("text", TypeCheck::String, false, false),
            ("base64", TypeCheck::String, false, false),
            ("bytes", TypeCheck::Bytes, true, false),
            ("md5", TypeCheck::String, true, true),
            ("sha1", TypeCheck::String, true, true),
            ("sha256", TypeCheck::String, true, true),
            ("sha256Int", TypeCheck::Int, true, true),
        ] {
            if factory && !matches!(name, "uri" | "text" | "bytes") {
                continue;
            }
            let start = class_start
                + source
                    .text
                    .entire()
                    .range(class_start..)
                    .find(&format!("{name}:"))
                    .unwrap();
            let span = Span::new(start, start + name.len());
            let type_start = span.end + 2;
            let type_len = match name {
                "uri" | "sha256Int" => 3,
                _ => {
                    if matches!(ty, TypeCheck::Bytes) {
                        5
                    } else {
                        6
                    }
                }
            };
            let type_span = if name == "uri" {
                // Resource.uri is declared as the source-backed Uri alias; its
                // expanded String check retains the alias target's source span.
                let target =
                    source.text.find("typealias Uri = String").unwrap() + "typealias Uri = ".len();
                Span::new(target, target + "String".len())
            } else {
                Span::new(type_start, type_start + type_len)
            };
            let mut code = if factory {
                vec![Instruction::ResourceProperty(match name {
                    "uri" => Property::Uri,
                    "text" => Property::Text,
                    _ => Property::Bytes,
                })]
            } else if matches!(name, "uri" | "text") {
                vec![Instruction::Undefined(name.into())]
            } else if name == "base64" {
                vec![
                    Instruction::LoadThis,
                    Instruction::GetMember("bytes".into()),
                    Instruction::GetMember("base64".into()),
                ]
            } else if name == "bytes" {
                vec![
                    Instruction::LoadThis,
                    Instruction::GetMember("base64".into()),
                    Instruction::GetMember("base64DecodedBytes".into()),
                ]
            } else {
                // The pinned Resource digest defaults delegate to the effective
                // receiver's bytes, including reader and base64-backed values.
                vec![
                    Instruction::LoadThis,
                    Instruction::GetMember("bytes".into()),
                    Instruction::GetMember(name.into()),
                ]
            };
            code.push(Instruction::CheckType(Box::new(ty.clone())));
            code.push(Instruction::Return);
            let block = blocks.len();
            metadata.push(BlockMetadata {
                source: Some(source.clone()),
                member: format!("#Resource.{name}").into(),
                declaration: Some(span),
                spans: code
                    .iter()
                    .map(|instruction| {
                        Some(if matches!(instruction, Instruction::CheckType(_)) {
                            type_span
                        } else {
                            span
                        })
                    })
                    .collect(),
                ..Default::default()
            });
            blocks.push(code);
            members.push(Member {
                flags: if hidden { 0x08 } else { 0 },
                entry: false,
                name: name.into(),
                code: block,
                local: false,
                method: false,
                ty: Some(ty),
                type_span: Some(type_span),
                constant: false,
                fixed,
            });
        }
        definitions.insert(if factory { FACTORY } else { RESOURCE }, members);
    }
    definitions
}

impl Vm {
    /// Publish only a successful typed reader result. A cached absence retains
    /// no exception: every ordinary read creates its own authored URI/frame.
    pub fn complete_read(
        &mut self,
        id: u64,
        result: Result<ReadResult, RuntimeError>,
    ) -> Result<(), RuntimeError> {
        let request = match &self.state {
            State::Waiting(PendingRequest::Read(request)) if request.id == id => request.clone(),
            _ => return self.resume(id, Err(invalid("invalid resource completion"))),
        };
        let (resource_uri_opt, value) = match result {
            Ok(ReadResult::Missing) => (None, None),
            Ok(ReadResult::Text(text)) => (None, Some(Value::String(text))),
            Ok(ReadResult::Resource(resource)) => {
                let res_uri = resource.uri.clone();
                (Some(res_uri), Some(self.resource(resource)))
            }
            Err(error) => return self.resume(id, Err(error)),
        };
        let uri = if let Some(res_uri) = resource_uri_opt
            && request.resolved_uri.is_none()
            && (request.uri.starts_with("@")
                || request.uri.starts_with("package:")
                || request.uri.starts_with("projectpackage:")
                || !request.uri.contains(":"))
        {
            res_uri
        } else {
            match crate::host::resources::resolved_uri(&request) {
                Ok(uri) => uri,
                Err(error) => return self.resume(id, Err(error)),
            }
        };
        self.resources
            .insert(crate::host::resources::cache_key(&uri), value.clone());
        self.resume(
            id,
            match value {
                Some(value) => Ok(value),
                None if request.nullable => Ok(Value::Null),
                None => Err(crate::host::resources::missing(&request.uri)),
            },
        )
    }

    fn resource(&mut self, resource: Resource) -> Value {
        let parent = self
            .builtin_class(builtins::ty(RESOURCE))
            .heap_id()
            .unwrap();
        let class = parent;
        let parent = self.heap.get_class(class).expect("builtin class").prototype;
        let object = Object {
            parent: Some(parent),
            instance_class: Some(class),
            resource: Some(Arc::new(resource)),
            members: self.program.builtin_members[&FACTORY].clone().into(),
            ..Default::default()
        };
        Value::Object(self.heap.allocate(object))
    }

    pub(super) fn resource_property(&mut self, property: Property) -> Result<(), RuntimeError> {
        let owner = self
            .frames
            .last()
            .and_then(|frame| frame.force.as_ref())
            .map(|force| force.owner)
            .ok_or_else(|| invalid("Resource property without declaration owner"))?;
        let resource = self
            .heap
            .get(owner)?
            .resource
            .as_ref()
            .ok_or_else(|| invalid("Resource payload missing"))?;
        let value = match property {
            Property::Uri => Value::String(resource.uri.clone()),
            Property::Text => Value::String(crate::resources::decode_utf8(&resource.bytes)),
            Property::Bytes => Value::Bytes(resource.bytes.clone().into()),
        };
        self.stack.push(value);
        Ok(())
    }
}

pub(super) fn base64(bytes: &[u8]) -> PklString {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::new();
    for chunk in bytes.chunks(3) {
        let value = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for (position, shift) in [18, 12, 6, 0].into_iter().enumerate() {
            text.push(if position > chunk.len() {
                '='
            } else {
                ALPHABET[((value >> shift) & 63) as usize] as char
            });
        }
    }
    text.into()
}

/// java.util.Base64's basic decoder, including its unpadded final unit and
/// ISO-8859-1 String boundary. Errors retain the caller's original UTF-16 value.
pub(super) fn decode_base64(text: &PklString) -> Result<Arc<[u8]>, String> {
    let bytes = char::decode_utf16(text.encode_utf16())
        .map(|point| {
            point
                .ok()
                .filter(|c| u32::from(*c) <= 255)
                .map_or(b'?', |c| c as u8)
        })
        .collect::<Vec<_>>();
    if bytes.len() == 1 {
        return Err("Input byte[] should at least have 2 bytes for base64 bytes".into());
    }
    let mut output = Vec::with_capacity(bytes.len() / 4 * 3 + 2);
    let mut index = 0;
    let mut bits = 0u32;
    let mut shift = 18;
    while index < bytes.len() {
        let byte = bytes[index];
        index += 1;
        let digit = match byte {
            b'A'..=b'Z' => u32::from(byte - b'A'),
            b'a'..=b'z' => u32::from(byte - b'a' + 26),
            b'0'..=b'9' => u32::from(byte - b'0' + 52),
            b'+' => 62,
            b'/' => 63,
            b'=' => {
                if shift == 18
                    || shift == 6
                        && (index == bytes.len() || {
                            let next = bytes[index];
                            index += 1;
                            next != b'='
                        })
                {
                    return Err("Input byte array has wrong 4-byte ending unit".into());
                }
                break;
            }
            _ => {
                let signed = i32::from(byte as i8);
                let hex = if signed < 0 {
                    format!("-{:x}", -signed)
                } else {
                    format!("{signed:x}")
                };
                return Err(format!("Illegal base64 character {hex}"));
            }
        };
        bits |= digit << shift;
        if shift == 0 {
            output.extend([(bits >> 16) as u8, (bits >> 8) as u8, bits as u8]);
            shift = 18;
            bits = 0;
        } else {
            shift -= 6;
        }
    }
    match shift {
        6 => output.push((bits >> 16) as u8),
        0 => output.extend([(bits >> 16) as u8, (bits >> 8) as u8]),
        12 => return Err("Last unit does not have enough valid bits".into()),
        _ => (),
    }
    if index < bytes.len() {
        return Err(format!(
            "Input byte array has incorrect ending byte at {index}"
        ));
    }
    Ok(output.into())
}

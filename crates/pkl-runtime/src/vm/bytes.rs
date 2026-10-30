//! Bytes literal argument checks, including the pinned UInt8 diagnostic source.
use super::strings::diagnostic_integer;
use super::*;

impl Vm {
    pub(super) fn check_byte(&self) -> Result<(), RuntimeError> {
        let value = self.stack.last().ok_or_else(|| invalid("Bytes argument"))?;
        if matches!(value, Value::Int(value) if (0..=255).contains(&value.get())) {
            return Ok(());
        }
        let source = crate::stdlib_sources::base();
        let declaration = source
            .text
            .find("typealias UInt8 = Int(isBetween(0, 255))")
            .unwrap();
        let int_start = declaration + "typealias UInt8 = ".len();
        let (mut error, span) = if matches!(value, Value::Int(_)) {
            let span = Span::new(int_start + 4, int_start + 4 + "isBetween(0, 255)".len());
            let mut error = RuntimeError::new(ErrorKind::Type, "Type constraint violated.");
            error.type_diagnostic = Some(Box::new(crate::diagnostic::TypeDiagnostic::Constraint {
                span,
                value: crate::diagnostic_values::render_string(
                    value,
                    &self.heap,
                    &self.program.module_name,
                    80,
                )?,
                body_span: span,
                // IntMaskSlotTypeNode reports its builtin range predicate even
                // when general source power assertions are disabled.
                observations: Some(vec![crate::diagnostic::PowerAssertionValue {
                    span,
                    value: "false".into(),
                    executions: 1,
                    hidden: false,
                }]),
            }));
            (error, span)
        } else {
            (
                type_error(
                    &TypeCheck::Int,
                    value,
                    &self.program.module_name,
                    &self.heap,
                )?,
                Span::new(int_start, int_start + 3),
            )
        };
        let frame = self
            .frames
            .last()
            .ok_or_else(|| invalid("Bytes argument frame"))?;
        let metadata = self
            .program
            .metadata
            .get(frame.block)
            .ok_or_else(|| invalid("Bytes argument metadata"))?;
        let module_name = metadata
            .source
            .as_ref()
            .map_or(&self.program.module_name, |source| &source.module_name);
        error.span = Some(span);
        error.frames.push(RuntimeFrame {
            anonymous: false,
            span,
            member: frame
                .diagnostic_member
                .clone()
                .unwrap_or_else(|| metadata.member.clone()),
            source: Some(Arc::new(crate::value::RuntimeSource {
                module_name: module_name.clone(),
                ..source.as_ref().clone()
            })),
        });
        // The instruction's authored argument span is appended by ordinary
        // unwinding, matching BytesLiteralNode's inserted argument frame.
        Err(error)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Operation {
    ToList,
    GetOrNull,
    Decode,
    ToString,
    GetClass,
}

pub(super) fn charset_type() -> (TypeCheck, Span) {
    let source = crate::stdlib_sources::base();
    let start = source.text.find("typealias Charset = ").unwrap() + "typealias Charset = ".len();
    let end = start + source.text.entire().range(start..).find("\n").unwrap();
    let names = ["UTF-8", "UTF-16", "ISO-8859-1"];
    let spans = names.map(|name| {
        let literal = format!("\"{name}\"");
        let offset = start
            + source
                .text
                .entire()
                .range(start..end)
                .find(&literal)
                .unwrap();
        Span::new(offset, offset + literal.len())
    });
    (
        TypeCheck::Union {
            string_literals: true,
            types: names
                .map(|name| TypeCheck::StringLiteral(name.into()))
                .to_vec(),
            spans: spans.to_vec(),
            labels: spans.to_vec(),
            default: None,
        },
        Span::new(start, end),
    )
}

pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), usize> {
    let source = crate::stdlib_sources::base();
    let class = source
        .text
        .find("external class Bytes extends Any")
        .unwrap();
    let mut result = BTreeMap::new();
    for (name, signature, operation) in [
        (
            "toList",
            "external function toList(): List<UInt8>",
            Operation::ToList,
        ),
        (
            "toString",
            "external function toString(): String",
            Operation::ToString,
        ),
        (
            "getClass",
            "external function getClass(): Class",
            Operation::GetClass,
        ),
        (
            "getOrNull",
            "external function getOrNull(index: Int): UInt8?",
            Operation::GetOrNull,
        ),
        (
            "decodeToString",
            "external function decodeToString(charset: Charset): String",
            Operation::Decode,
        ),
    ] {
        let inherited = matches!(operation, Operation::ToString | Operation::GetClass);
        let start = if inherited {
            source.text.find(signature).unwrap()
        } else {
            class + source.text.entire().range(class..).find(signature).unwrap()
        };
        let mut code = Vec::new();
        let mut spans = Vec::new();
        if matches!(operation, Operation::GetOrNull | Operation::Decode) {
            code.push(Instruction::LoadLocal(0));
            spans.push(None);
            let (ty, span) = if matches!(operation, Operation::Decode) {
                charset_type()
            } else {
                let offset = start + signature.find("Int").unwrap();
                (TypeCheck::Int, Span::new(offset, offset + 3))
            };
            code.push(Instruction::CheckType(ty));
            spans.push(Some(span));
        }
        code.extend([
            Instruction::Primitive(primitives::Operation::Bytes(operation)),
            Instruction::Return,
        ]);
        spans.extend([None, None]);
        let block = blocks.len();
        blocks.push(code);
        metadata.push(BlockMetadata {
            source: Some(source.clone()),
            member: format!("#{}.{name}", if inherited { "Any" } else { "Bytes" }).into(),
            declaration: Some(Span::new(start, start + signature.len())),
            spans,
            ..Default::default()
        });
        result.insert(("Bytes".into(), name.into()), block);
    }
    result
}

fn hex(bytes: &[u8]) -> PklString {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 15) as usize] as char);
    }
    output.into()
}

impl Vm {
    pub(super) fn bytes_property(
        &mut self,
        bytes: &crate::value::Bytes,
        name: &str,
    ) -> Result<bool, RuntimeError> {
        use sha2::Digest;
        let result = match name {
            "length" => Value::Int((bytes.len() as i64).into()),
            "size" => Value::DataSize(bytes.size()),
            "hex" => Value::String(bytes.cache.hex.get_or_init(|| hex(bytes)).clone()),
            "base64" => Value::String(
                bytes
                    .cache
                    .base64
                    .get_or_init(|| resources::base64(bytes))
                    .clone(),
            ),
            "md5" => Value::String(hex(&md5::Md5::digest(bytes))),
            "sha1" => Value::String(hex(&sha1::Sha1::digest(bytes))),
            "sha256" => Value::String(hex(&sha2::Sha256::digest(bytes))),
            "sha256Int" => {
                let digest = sha2::Sha256::digest(bytes);
                Value::Int(i64::from_le_bytes(digest[..8].try_into().unwrap()).into())
            }
            _ => return Ok(false),
        };
        self.stack.push(result);
        Ok(true)
    }

    pub(super) fn bytes_operation(
        &mut self,
        receiver: Value,
        operation: Operation,
    ) -> Result<(), RuntimeError> {
        let Value::Bytes(bytes) = receiver else {
            return Err(invalid("Bytes receiver"));
        };
        let result = match operation {
            Operation::GetOrNull => {
                let Value::Int(index) = self.pop()? else {
                    return Err(invalid("checked Bytes index"));
                };
                // VmBytes.get widens a signed Java byte; iteration/toList is unsigned.
                usize::try_from(index.get())
                    .ok()
                    .and_then(|index| bytes.get(index))
                    .map_or(Value::Null, |byte| Value::Int((*byte as i8 as i64).into()))
            }
            Operation::Decode => {
                let Value::String(charset) = self.pop()? else {
                    return Err(invalid("checked Charset"));
                };
                let text = decode(
                    &bytes,
                    charset.as_str().ok_or_else(|| invalid("checked Charset"))?,
                )
                .ok_or_else(|| {
                    RuntimeError::new(
                        ErrorKind::Arithmetic,
                        format!("Invalid bytes for charset \"{charset}\"."),
                    )
                })?;
                Value::String(text)
            }
            Operation::ToList => Value::Object(self.bytes_to_list(&bytes)?),
            Operation::ToString => Value::String(bytes.to_string().into()),
            Operation::GetClass => self.reference_get_class(&Value::Bytes(bytes))?,
        };
        self.stack.push(result);
        Ok(())
    }

    fn bytes_to_list(&mut self, bytes: &crate::value::Bytes) -> Result<ObjectId, RuntimeError> {
        if let Some((_, id, _)) = self.heap.bytes_lists.get(&bytes.reference()) {
            return Ok(*id);
        }
        let id = if bytes.is_empty() {
            self.empty_strict_collection(crate::heap::CollectionKind::List)
        } else {
            let builder = self.heap.allocate(Object {
                collection_kind: crate::heap::CollectionKind::List,
                ..Default::default()
            });
            self.frames
                .last_mut()
                .expect("Bytes.toList frame")
                .builders
                .push(builder);
            for byte in bytes.iter() {
                self.stack.push(Value::Int(i64::from(*byte).into()));
                self.append_strict_element()?;
            }
            self.frames
                .last_mut()
                .expect("Bytes.toList frame")
                .builders
                .pop();
            builder
        };
        // Constant carriers live with the immutable code, independently of VM frames.
        let code_constant = self.program.blocks.iter().flatten().any(|instruction| {
            matches!(instruction, Instruction::Constant(Value::Bytes(value)) if value.reference() == bytes.reference())
        });
        self.heap
            .bytes_lists
            .insert(bytes.reference(), (bytes.lifetime(), id, code_constant));
        Ok(id)
    }

    pub(super) fn subscript_bytes(
        &self,
        bytes: &crate::value::Bytes,
        key: Value,
    ) -> Result<Value, RuntimeError> {
        let Value::Int(index) = key else {
            return Err(RuntimeError::new(ErrorKind::Type, PklString::join(&[
                format!("Operator `[]` is not defined for operand types `Bytes` and `{}`.\nLeft operand : ", crate::diagnostic_values::type_name(&key, &self.heap, &self.program.module_name)?).into(),
                crate::diagnostic_values::render_string(&Value::Bytes(bytes.clone()), &self.heap, &self.program.module_name, 80)?,
                "\nRight operand: ".into(),
                crate::diagnostic_values::render_string(&key, &self.heap, &self.program.module_name, 80)?,
            ], "")));
        };
        if let Some(byte) = usize::try_from(index.get())
            .ok()
            .and_then(|index| bytes.get(index))
        {
            return Ok(Value::Int((*byte as i8 as i64).into()));
        }
        Err(RuntimeError::new(
            ErrorKind::Type,
            PklString::join(
                &[
                    format!(
                        "Element index `{}` is out of range `0`..`{}`.\nValue: ",
                        diagnostic_integer(*index),
                        diagnostic_integer(bytes.len() as i64 - 1)
                    )
                    .into(),
                    crate::diagnostic_values::render_string(
                        &Value::Bytes(bytes.clone()),
                        &self.heap,
                        &self.program.module_name,
                        80,
                    )?,
                ],
                "",
            ),
        ))
    }

    pub(super) fn bytes_member_error(
        &self,
        name: impl Into<PklString>,
        method: bool,
    ) -> RuntimeError {
        let name = name.into();
        let properties = [
            "length",
            "size",
            "base64",
            "hex",
            "md5",
            "sha1",
            "sha256",
            "sha256Int",
        ]
        .map(str::to_owned);
        let methods = [
            ("toList", "toList()", 0),
            ("getOrNull", "getOrNull(index)", 1),
            ("decodeToString", "decodeToString(charset)", 1),
            ("getClass", "getClass()", 0),
            ("toString", "toString()", 0),
            ("ifNonNull", "ifNonNull(transform)", 1),
        ]
        .map(|(name, signature, arity)| (name.to_owned(), signature.to_owned(), arity));
        let arity = if method {
            self.frames
                .last()
                .and_then(|frame| {
                    self.program.metadata.get(frame.block).and_then(|metadata| {
                        metadata.method_arities.get(&frame.pc.saturating_sub(1))
                    })
                })
                .copied()
                .unwrap_or(0) as isize
        } else {
            -1
        };
        let mut error = RuntimeError::new(
            ErrorKind::MissingMember,
            if method {
                PklString::from("Cannot find method `")
                    .concat(&name)
                    .concat(&"` in class `Bytes`.".into())
            } else {
                PklString::from("Cannot find property `")
                    .concat(&name)
                    .concat(&"` in object of type `Bytes`.".into())
            },
        );
        error.hint = crate::diagnostic::member_suggestions(&name, arity, &properties, &methods)
            .or_else(|| {
                let mut names = if method {
                    methods
                        .iter()
                        .map(|(name, _, _)| name.as_str())
                        .collect::<Vec<_>>()
                } else {
                    properties.iter().map(String::as_str).collect()
                };
                names.sort();
                Some(
                    format!(
                        "{}\n{}",
                        if method {
                            "Available methods in class `Bytes`:"
                        } else {
                            "Available properties:"
                        },
                        names.join("\n")
                    )
                    .into(),
                )
            });
        error
    }
}

fn decode(bytes: &[u8], charset: &str) -> Option<PklString> {
    match charset {
        "UTF-8" => std::str::from_utf8(bytes).ok().map(PklString::from),
        "ISO-8859-1" => Some(PklString::from_utf16(
            bytes
                .iter()
                .map(|byte| u16::from(*byte))
                .collect::<Vec<_>>(),
        )),
        "UTF-16" => {
            if !bytes.len().is_multiple_of(2) {
                return None;
            }
            let (bytes, little) = if bytes.starts_with(&[0xff, 0xfe]) {
                (&bytes[2..], true)
            } else if bytes.starts_with(&[0xfe, 0xff]) {
                (&bytes[2..], false)
            } else {
                (bytes, false)
            };
            let units = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|pair| {
                    if little {
                        u16::from_le_bytes([pair[0], pair[1]])
                    } else {
                        u16::from_be_bytes([pair[0], pair[1]])
                    }
                })
                .collect::<Vec<_>>();
            if char::decode_utf16(units.iter().copied()).any(|result| result.is_err()) {
                return None;
            }
            Some(PklString::from_utf16(units))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_cached_strings_and_concatenation_preserve_java_reference_identity() {
        let bytes: crate::value::Bytes = [0, 128, 255].into();
        let mut vm = Vm::new(Arc::new(Program::new(vec![])));
        for name in ["hex", "base64"] {
            vm.bytes_property(&bytes, name).unwrap();
            let first = vm.pop().unwrap();
            vm.bytes_property(&bytes, name).unwrap();
            let second = vm.pop().unwrap();
            assert_eq!(first.reference(), second.reference());
        }
        for name in ["md5", "sha1", "sha256"] {
            vm.bytes_property(&bytes, name).unwrap();
            let first = vm.pop().unwrap();
            vm.bytes_property(&bytes, name).unwrap();
            let second = vm.pop().unwrap();
            assert_eq!(first, second);
            assert_ne!(first.reference(), second.reference());
        }
        let empty = crate::value::Bytes::empty();
        assert_eq!(bytes.concatenate(&empty).reference(), bytes.reference());
        assert_eq!(empty.concatenate(&bytes).reference(), bytes.reference());
        assert_ne!(bytes.concatenate(&bytes).reference(), bytes.reference());
    }

    #[test]
    fn heap_release_drops_bytes_backlinks_even_while_scalar_carriers_live() {
        let bytes: crate::value::Bytes = [255].into();
        let mut heap = Heap::new();
        let id = heap.allocate(Object::default());
        heap.bytes_lists
            .insert(bytes.reference(), (bytes.lifetime(), id, true));
        assert_eq!(heap.collect(&[]).live, 1);
        heap.release();
        assert!(heap.bytes_lists.is_empty());
        assert_eq!(heap.stats().slots, 0);
        assert!(heap.get(id).is_err());
    }
}

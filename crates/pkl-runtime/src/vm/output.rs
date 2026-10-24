//! Source-backed module output classes and ordinary lazy property definitions.
use super::*;

pub(crate) const MODULE: usize = usize::MAX - 200;
pub(crate) const FILE_OUTPUT: usize = MODULE - 1;
pub(crate) const MODULE_OUTPUT: usize = MODULE - 2;
pub(crate) const BASE_RENDERER: usize = MODULE - 3;
pub(crate) const VALUE_RENDERER: usize = MODULE - 4;
pub(crate) const BYTES_RENDERER: usize = MODULE - 5;
pub(crate) const PCF_RENDERER: usize = MODULE - 6;
pub(crate) const CONVERT_PROPERTY: usize = MODULE - 7;
pub(crate) const JSON_RENDERER: usize = MODULE - 8;
pub(crate) const RENDER_DIRECTIVE: usize = MODULE - 9;
const NAMES: &[&str] = &[
    "Module",
    "FileOutput",
    "ModuleOutput",
    "BaseValueRenderer",
    "ValueRenderer",
    "BytesRenderer",
    "PcfRenderer",
    "ConvertProperty",
    "JsonRenderer",
    "RenderDirective",
];
pub(crate) fn class_id(name: &str) -> Option<usize> {
    NAMES
        .iter()
        .position(|candidate| *candidate == name)
        .map(|i| MODULE - i)
}
pub(crate) fn class_name(id: usize) -> Option<&'static str> {
    MODULE.checked_sub(id).and_then(|i| NAMES.get(i).copied())
}
pub(super) fn class_parent(id: usize) -> Option<usize> {
    match id {
        CONVERT_PROPERTY => Some(annotations::ANNOTATION_ID),
        MODULE_OUTPUT => Some(FILE_OUTPUT),
        VALUE_RENDERER | BYTES_RENDERER => Some(BASE_RENDERER),
        PCF_RENDERER | JSON_RENDERER => Some(VALUE_RENDERER),
        _ => None,
    }
}
pub(super) fn abstract_class(id: usize) -> bool {
    matches!(id, MODULE | BASE_RENDERER | VALUE_RENDERER | BYTES_RENDERER)
}
pub(super) fn open_class(id: usize) -> bool {
    abstract_class(id) || matches!(id, FILE_OUTPUT | CONVERT_PROPERTY)
}

fn property(name: &str) -> Instruction {
    Instruction::GetProperty {
        name: name.into(),
        owner_depth: 0,
        local: false,
    }
}
struct Definitions<'a> {
    blocks: &'a mut Vec<Vec<Instruction>>,
    metadata: &'a mut Vec<BlockMetadata>,
    source: Arc<crate::value::RuntimeSource>,
}
impl Definitions<'_> {
    fn member(
        &mut self,
        class: &str,
        name: &str,
        ty: TypeCheck,
        mut code: Vec<Instruction>,
        hidden: bool,
    ) -> Member {
        let class_start = self
            .source
            .text
            .find(&format!("class {class}"))
            .expect("builtin class source");
        let start = class_start
            + self.source.text[class_start..]
                .find(&format!("{name}:"))
                .expect("builtin property source");
        let name_span = Span::new(start, start + name.len());
        let type_start = name_span.end + 2;
        let type_end = self.source.text[type_start..]
            .find(['=', '\n'])
            .map_or(self.source.text.len(), |i| type_start + i);
        let type_end = self.source.text[..type_end].trim_end().len();
        let type_span = Span::new(type_start, type_end);
        let check = code.len();
        code.extend([Instruction::CheckType(ty.clone()), Instruction::Return]);
        let block = self.blocks.len();
        self.metadata.push(BlockMetadata {
            source: Some(self.source.clone()),
            member: format!("#{class}.{name}").into(),
            spans: (0..code.len())
                .map(|i| Some(if i == check { type_span } else { name_span }))
                .collect(),
            declaration: Some(Span::new(start, type_end)),
            value_span: Some(name_span),
            property_typecheck: Some(check),
            ..Default::default()
        });
        self.blocks.push(code);
        Member {
            flags: if hidden { 0x08 } else { 0 },
            entry: false,
            name: name.into(),
            code: block,
            local: false,
            method: false,
            ty: Some(ty),
            type_span: Some(type_span),
            constant: false,
            fixed: false,
        }
    }
    fn expression_spans(
        &self,
        start: usize,
        code: &[Instruction],
        names: &[&str],
    ) -> Vec<Option<Span>> {
        code.iter()
            .zip(names)
            .map(|(_, name)| {
                if name.is_empty() {
                    None
                } else {
                    let offset = start
                        + self.source.text[start..]
                            .find(name)
                            .expect("builtin expression source");
                    Some(Span::new(offset, offset + name.len()))
                }
            })
            .collect()
    }
    fn method(&mut self, class: &str, name: &str, body: Instruction) -> Member {
        let start = self
            .source
            .text
            .find(&format!("class {class}"))
            .expect("class");
        let start = start
            + self.source.text[start..]
                .find(&format!("function {name}("))
                .expect("method");
        let end = start + self.source.text[start..].find('\n').unwrap();
        let body_block = self.blocks.len();
        self.blocks.push(vec![
            Instruction::LoadLocal(0),
            Instruction::BindLocal {
                depth: Some(0),
                ty: TypeCheck::Any,
            },
            Instruction::LoadLocal(0),
            body,
            Instruction::Return,
        ]);
        self.metadata.push(BlockMetadata {
            source: Some(self.source.clone()),
            member: format!("#{class}.{name}").into(),
            declaration: Some(Span::new(start, end)),
            spans: vec![None; 5],
            ..Default::default()
        });
        let block = self.blocks.len();
        self.blocks.push(vec![
            Instruction::MakeFunction {
                block: body_block,
                arity: 1,
                method: true,
                capture_custom_this: false,
            },
            Instruction::Return,
        ]);
        self.metadata.push(BlockMetadata {
            source: Some(self.source.clone()),
            member: format!("#{class}.{name}").into(),
            declaration: Some(Span::new(start, end)),
            spans: vec![None; 2],
            ..Default::default()
        });
        Member {
            flags: 0,
            entry: false,
            name: name.into(),
            code: block,
            local: false,
            method: true,
            ty: None,
            type_span: None,
            constant: false,
            fixed: false,
        }
    }
}

pub(super) fn builtin_members(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
    objects: &mut Vec<Vec<Member>>,
) -> BTreeMap<usize, Vec<Member>> {
    let mut d = Definitions {
        blocks,
        metadata,
        source: crate::stdlib_sources::base(),
    };
    let empty = objects.len();
    objects.push(Vec::new());
    let instantiate = |id| {
        vec![
            Instruction::BuiltinClass(builtins::ty(id)),
            Instruction::MakeBuiltinInstance { template: empty },
        ]
    };
    let value = d.member(
        "FileOutput",
        "value",
        TypeCheck::Nullable(Box::new(TypeCheck::Any)),
        vec![Instruction::Constant(Value::Null)],
        false,
    );
    let renderer = d.member(
        "FileOutput",
        "renderer",
        builtins::ty(BASE_RENDERER),
        instantiate(PCF_RENDERER),
        false,
    );
    // Branches remain ordinary property reads/method calls so authored overrides
    // and their suspension/type-check/diagnostic frames use the common engine.
    let text_code = vec![
        property("renderer"),
        Instruction::TestType(builtins::ty(VALUE_RENDERER)),
        Instruction::JumpIfFalse(9),
        property("renderer"),
        Instruction::GetMethod("renderDocument".into()),
        Instruction::PrepareMethodCall {
            lookup: 4,
            arity: 0,
            after_arguments: false,
        },
        property("value"),
        Instruction::Call {
            arity: 1,
            method: true,
        },
        Instruction::Jump(11),
        Instruction::Constant(Value::string(
            "unabled to render text when renderer is a BytesRenderer",
        )),
        Instruction::Throw,
    ];
    let text = d.member("FileOutput", "text", TypeCheck::String, text_code, false);
    let bytes_code = vec![
        property("renderer"),
        Instruction::TestType(builtins::ty(BYTES_RENDERER)),
        Instruction::JumpIfFalse(9),
        property("renderer"),
        Instruction::GetMethod("renderDocument".into()),
        Instruction::PrepareMethodCall {
            lookup: 4,
            arity: 0,
            after_arguments: false,
        },
        property("value"),
        Instruction::Call {
            arity: 1,
            method: true,
        },
        Instruction::Jump(11),
        property("text"),
        Instruction::EncodeUtf8,
    ];
    let bytes = d.member(
        "FileOutput",
        "bytes",
        TypeCheck::Bytes,
        bytes_code.clone(),
        false,
    );
    for (member, names) in [
        (
            &text,
            vec![
                "renderer",
                "renderer is ValueRenderer",
                "",
                "renderer",
                "renderer.renderDocument(value)",
                "renderer.renderDocument(value)",
                "value",
                "renderer.renderDocument(value)",
                "",
                "",
                "throw(\"unabled to render text when renderer is a BytesRenderer\")",
                "String",
                "",
            ],
        ),
        (
            &bytes,
            vec![
                "renderer",
                "renderer is BytesRenderer",
                "",
                "renderer",
                "renderer.renderDocument(value)",
                "renderer.renderDocument(value)",
                "value",
                "renderer.renderDocument(value)",
                "",
                "text",
                "text.encodeToBytes(\"UTF-8\")",
                "Bytes",
                "",
            ],
        ),
    ] {
        let start = d.metadata[member.code].declaration.unwrap().start;
        let mut spans = d.expression_spans(start, &d.blocks[member.code], &names);
        let check = d.metadata[member.code].property_typecheck.unwrap();
        spans[check] = member.type_span;
        d.metadata[member.code].spans = spans;
    }
    let files = d.member(
        "ModuleOutput",
        "files",
        TypeCheck::Nullable(Box::new(TypeCheck::ParameterizedCollection {
            kind: crate::heap::CollectionKind::Mapping,
            arguments: vec![
                (TypeCheck::String, Span::new(0, 0)),
                (builtins::ty(FILE_OUTPUT), Span::new(0, 0)),
            ],
        })),
        vec![Instruction::Constant(Value::Null)],
        false,
    );
    let output_value = d.member(
        "Module",
        "output",
        builtins::ty(MODULE_OUTPUT),
        vec![
            Instruction::BuiltinClass(builtins::ty(MODULE_OUTPUT)),
            Instruction::MakeBuiltinInstance {
                template: empty + 1,
            },
        ],
        true,
    );
    // This amendment captures the module property frame as its lexical outer.
    let vblock = d.blocks.len();
    d.blocks
        .push(vec![Instruction::LoadOuter, Instruction::Return]);
    let module_start = d.source.text.find("hidden output:").expect("module output");
    d.metadata.push(BlockMetadata {
        source: Some(d.source.clone()),
        member: "#Module.output.value".into(),
        spans: d.expression_spans(module_start, &d.blocks[vblock], &["outer", ""]),
        ..Default::default()
    });
    let mut m = value.clone();
    m.code = vblock;
    let rblock = d.blocks.len();
    d.blocks.push(vec![
        Instruction::Constant(Value::string("prop:pkl.outputFormat")),
        Instruction::ReadResourceOrNull,
        Instruction::OutputRenderer,
        Instruction::Return,
    ]);
    d.metadata.push(BlockMetadata {
        source: Some(d.source.clone()),
        member: "#Module.output.renderer".into(),
        spans: d.expression_spans(
            module_start,
            &d.blocks[rblock],
            &[
                "\"prop:pkl.outputFormat\"",
                "read?(\"prop:pkl.outputFormat\")",
                "throw(\n            \"Unknown output format:",
                "",
            ],
        ),
        ..Default::default()
    });
    let let_start = module_start + d.source.text[module_start..].find("let (format =").unwrap();
    let let_end = let_start + d.source.text[let_start..].find("\n    text =").unwrap();
    d.metadata[rblock].let_scopes.push(LetScope {
        body: 2..3,
        frame: RuntimeFrame {
            source: Some(d.source.clone()),
            span: Span::new(let_start, let_end),
            member: "#Module.output.renderer.<let expr>".into(),
        },
        writes_slot: true,
    });
    let mut r = renderer.clone();
    r.code = rblock;
    let mut t = text.clone();
    let tblock = d.blocks.len();
    let mut tc = d.blocks[text.code].clone();
    tc[9] = Instruction::Constant(Value::string(
        "Unable to render text when renderer is a BytesRenderer",
    ));
    d.blocks.push(tc);
    let mut tm = d.metadata[text.code].clone();
    tm.member = "#Module.output.text".into();
    let text_start = module_start + d.source.text[module_start..].find("text =").unwrap();
    tm.spans = d.expression_spans(
        text_start,
        &d.blocks[tblock],
        &[
            "renderer",
            "renderer is ValueRenderer",
            "",
            "renderer",
            "renderer.renderDocument(value)",
            "renderer.renderDocument(value)",
            "value",
            "renderer.renderDocument(value)",
            "",
            "",
            "throw(\"Unable to render text when renderer is a BytesRenderer\")",
            "",
            "",
        ],
    );
    d.metadata.push(tm);
    t.code = tblock;
    let mut b = bytes.clone();
    b.code = d.blocks.len();
    d.blocks.push(d.blocks[bytes.code].clone());
    let mut bm = d.metadata[bytes.code].clone();
    bm.member = "#Module.output.bytes".into();
    let bytes_start = text_start + d.source.text[text_start..].find("bytes =").unwrap();
    bm.spans = d.expression_spans(
        bytes_start,
        &d.blocks[b.code],
        &[
            "renderer",
            "renderer is BytesRenderer",
            "",
            "renderer",
            "renderer.renderDocument(value)",
            "renderer.renderDocument(value)",
            "value",
            "renderer.renderDocument(value)",
            "",
            "text",
            "text.encodeToBytes(\"UTF-8\")",
            "",
            "",
        ],
    );
    d.metadata.push(bm);
    objects.push(vec![m, r, t, b]);
    let mapping = || {
        vec![
            Instruction::BeginCollection {
                kind: crate::heap::CollectionKind::Mapping,
                has_parent: false,
                specialized: true,
            },
            Instruction::EndCollection,
        ]
    };
    let converter_start = d
        .source
        .text
        .find("converters: Mapping<Class | String, (unknown) -> Any>")
        .unwrap();
    let part = |text: &str| {
        let start = converter_start + d.source.text[converter_start..].find(text).unwrap();
        Span::new(start, start + text.len())
    };
    let class_span = part("Class");
    let string_span = part("String");
    let union_span = Span::new(class_span.start, string_span.end);
    let key_type = TypeCheck::Union {
        string_literals: false,
        types: vec![TypeCheck::MetaClass("Class".into()), TypeCheck::String],
        labels: vec![class_span, string_span],
        spans: vec![class_span, string_span],
        default: None,
    };
    let function = TypeCheck::Function {
        parameters: Some(vec![TypeCheck::Unknown]),
        result: Box::new(TypeCheck::Any),
        syntax: true,
    };
    let converters = d.member(
        "BaseValueRenderer",
        "converters",
        TypeCheck::ParameterizedCollection {
            kind: crate::heap::CollectionKind::Mapping,
            arguments: vec![(key_type, union_span), (function, part("(unknown) -> Any"))],
        },
        mapping(),
        false,
    );
    let transformer_start = d
        .source
        .text
        .find("convertPropertyTransformers: Mapping<Class, Mixin<ConvertProperty>>")
        .unwrap();
    let class_start = transformer_start + d.source.text[transformer_start..].find("Class").unwrap();
    let mixin_start = class_start
        + d.source.text[class_start..]
            .find("Mixin<ConvertProperty>")
            .unwrap();
    let mixin = amendment::mixin_type(builtins::ty(CONVERT_PROPERTY));
    let transformers = d.member(
        "BaseValueRenderer",
        "convertPropertyTransformers",
        TypeCheck::ParameterizedCollection {
            kind: crate::heap::CollectionKind::Mapping,
            arguments: vec![
                (
                    TypeCheck::MetaClass("Class".into()),
                    Span::new(class_start, class_start + 5),
                ),
                (
                    mixin.clone(),
                    Span::new(mixin_start, mixin_start + "Mixin<ConvertProperty>".len()),
                ),
            ],
        },
        vec![
            Instruction::BeginCollection {
                kind: crate::heap::CollectionKind::Mapping,
                has_parent: false,
                specialized: true,
            },
            Instruction::SetCollectionTypeDefault {
                ty: mixin,
                origin: Some(RuntimeFrame {
                    source: Some(d.source.clone()),
                    span: Span::new(mixin_start, mixin_start + "Mixin<ConvertProperty>".len()),
                    member: "#BaseValueRenderer.convertPropertyTransformers.default".into(),
                }),
            },
            Instruction::EndCollection,
        ],
        false,
    );
    let pair = TypeCheck::Pair {
        arguments: vec![
            (TypeCheck::String, Span::new(0, 0)),
            (TypeCheck::Any, Span::new(0, 0)),
        ],
    };
    let render = d.member(
        "ConvertProperty",
        "render",
        TypeCheck::Function {
            parameters: Some(vec![pair.clone(), builtins::ty(BASE_RENDERER)]),
            result: Box::new(pair),
            syntax: true,
        },
        vec![Instruction::Undefined("render".into())],
        true,
    );
    let extension = d.member(
        "BaseValueRenderer",
        "extension",
        TypeCheck::Nullable(Box::new(TypeCheck::String)),
        vec![Instruction::Constant(Value::Null)],
        false,
    );
    let indent = d.member(
        "PcfRenderer",
        "indent",
        TypeCheck::String,
        vec![Instruction::Constant(Value::string("  "))],
        false,
    );
    let omit = d.member(
        "PcfRenderer",
        "omitNullProperties",
        TypeCheck::Boolean,
        vec![Instruction::Constant(Value::Bool(false))],
        false,
    );
    let custom = d.member(
        "PcfRenderer",
        "useCustomStringDelimiters",
        TypeCheck::Boolean,
        vec![Instruction::Constant(Value::Bool(false))],
        false,
    );
    let mut ext = extension.clone();
    ext.code = d.blocks.len();
    d.blocks.push(vec![
        Instruction::Constant(Value::string("pcf")),
        Instruction::Return,
    ]);
    d.metadata.push(d.metadata[extension.code].clone());
    let document = d.method(
        "PcfRenderer",
        "renderDocument",
        Instruction::RenderPcf { document: true },
    );
    let render_value = d.method(
        "PcfRenderer",
        "renderValue",
        Instruction::RenderPcf { document: false },
    );
    let json_indent = d.member(
        "JsonRenderer",
        "indent",
        TypeCheck::String,
        vec![Instruction::Constant(Value::string("  "))],
        false,
    );
    let json_omit = d.member(
        "JsonRenderer",
        "omitNullProperties",
        TypeCheck::Boolean,
        vec![Instruction::Constant(Value::Bool(true))],
        false,
    );
    let mut json_extension = extension.clone();
    json_extension.code = d.blocks.len();
    d.blocks.push(vec![
        Instruction::Constant(Value::string("json")),
        Instruction::Return,
    ]);
    let json_class = d.source.text.find("class JsonRenderer extends").unwrap();
    let json_extension_start =
        json_class + d.source.text[json_class..].find("extension =").unwrap();
    let json_literal = json_extension_start + "extension = ".len();
    let json_value_span = Span::new(json_literal, json_literal + "\"json\"".len());
    d.metadata.push(BlockMetadata {
        source: Some(d.source.clone()),
        member: "#JsonRenderer.extension".into(),
        declaration: Some(Span::new(json_extension_start, json_value_span.end)),
        value_span: Some(json_value_span),
        spans: vec![Some(json_value_span), None],
        ..Default::default()
    });
    let json_document = d.method(
        "JsonRenderer",
        "renderDocument",
        Instruction::RenderJson { document: true },
    );
    let json_value = d.method(
        "JsonRenderer",
        "renderValue",
        Instruction::RenderJson { document: false },
    );
    let directive_text = d.member(
        "RenderDirective",
        "text",
        TypeCheck::String,
        vec![Instruction::Undefined("text".into())],
        false,
    );
    let directive_bytes = d.member(
        "RenderDirective",
        "bytes",
        TypeCheck::Bytes,
        vec![property("text"), Instruction::EncodeUtf8],
        false,
    );
    let directive_start = d.metadata[directive_bytes.code].declaration.unwrap().start;
    d.metadata[directive_bytes.code].spans = d.expression_spans(
        directive_start,
        &d.blocks[directive_bytes.code],
        &["text", "text.encodeToBytes(\"UTF-8\")", "Bytes", ""],
    );
    BTreeMap::from([
        (MODULE, vec![output_value]),
        (CONVERT_PROPERTY, vec![render]),
        (FILE_OUTPUT, vec![value, renderer, text, bytes]),
        (MODULE_OUTPUT, vec![files]),
        (BASE_RENDERER, vec![converters, transformers, extension]),
        (VALUE_RENDERER, vec![]),
        (BYTES_RENDERER, vec![]),
        (
            JSON_RENDERER,
            vec![
                json_extension,
                json_indent,
                json_omit,
                json_document,
                json_value,
            ],
        ),
        (RENDER_DIRECTIVE, vec![directive_text, directive_bytes]),
        (
            PCF_RENDERER,
            vec![ext, indent, omit, custom, document, render_value],
        ),
    ])
}

impl Vm {
    pub(super) fn check_module_output(&self) -> Result<(), RuntimeError> {
        let value = self
            .stack
            .last()
            .ok_or_else(|| invalid("missing module output"))?;
        if let Value::Object(id) = value
            && collections::typed_class(&self.heap, *id)?.is_some_and(|c| c.id == MODULE_OUTPUT)
        {
            return Ok(());
        }
        let uri = self
            .context
            .primary_source
            .as_ref()
            .map_or("", |s| s.uri.as_ref());
        let name =
            crate::diagnostic_values::type_name(value, &self.heap, &self.program.module_name)?;
        let mut error = RuntimeError::new(
            ErrorKind::Type,
            format!(
                "Expected `output` of module `{uri}` to be of type `ModuleOutput`, but got type `{name}`."
            ),
        );
        if let Some(Value::Object(module)) = self.inputs.first()
            && let Some(member) = self
                .heap
                .get(*module)?
                .members
                .iter()
                .find(|m| m.name.as_ref() == "output")
        {
            let metadata = &self.program.metadata[member.code];
            if let Some(span) = metadata.value_span {
                error.frames.push(RuntimeFrame {
                    span,
                    member: "output".into(),
                    source: metadata
                        .source
                        .as_ref()
                        .or(self.context.primary_source.as_ref())
                        .map(|source| {
                            Arc::new(crate::value::RuntimeSource {
                                module_name: "".into(),
                                ..(**source).clone()
                            })
                        }),
                });
            }
        }
        Err(error)
    }
    pub(super) fn output_renderer(&mut self) -> Result<(), RuntimeError> {
        let format = self.pop()?;
        if format.is_null() || format == Value::string("pcf") || format == Value::string("json") {
            let renderer = if format == Value::string("json") {
                JSON_RENDERER
            } else {
                PCF_RENDERER
            };
            let value = self.builtin_class(builtins::ty(renderer));
            self.stack
                .push(Value::Object(value.heap_id().expect("class")));
            return Ok(());
        }
        let Value::String(format) = format else {
            return Err(invalid("output format is not a String"));
        };
        if matches!(
            format.as_str(),
            Some("jsonnet" | "plist" | "properties" | "textproto" | "xml" | "yaml" | "pkl-binary")
        ) {
            return Err(RuntimeError::new(
                ErrorKind::Unsupported,
                PklString::from("The `")
                    .concat(&format)
                    .concat(&"` output renderer is not implemented.".into()),
            ));
        }
        Err(RuntimeError::new(
            ErrorKind::Type,
            PklString::from("Unknown output format: `")
                .concat(&format)
                .concat(&"`. Supported formats are `json`, `jsonnet`, `pcf`, `plist`, `properties`, `textproto`, `xml`, `yaml`, `pkl-binary`.".into()),
        ))
    }
}

#[cfg(test)]
mod tests;

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
pub(crate) const PCF_RENDER_DIRECTIVE: usize = MODULE - 10;
pub(crate) const YAML_RENDERER: usize = MODULE - 11;
pub(crate) const PLIST_RENDERER: usize = MODULE - 12;
pub(crate) const PROPERTIES_RENDERER: usize = MODULE - 13;
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
    "PcfRenderDirective",
    "YamlRenderer",
    "PListRenderer",
    "PropertiesRenderer",
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
        PCF_RENDERER | JSON_RENDERER | YAML_RENDERER | PLIST_RENDERER | PROPERTIES_RENDERER => {
            Some(VALUE_RENDERER)
        }
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
pub(super) struct Definitions<'a> {
    pub(super) blocks: &'a mut Vec<Vec<Instruction>>,
    pub(super) metadata: &'a mut Vec<BlockMetadata>,
    pub(super) source: Arc<crate::value::RuntimeSource>,
}
impl Definitions<'_> {
    pub(super) fn member(
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
            + self
                .source
                .text
                .entire()
                .range(class_start..)
                .find(&format!("{name}:"))
                .expect("builtin property source");
        let name_span = Span::new(start, start + name.len());
        let type_start = name_span.end + 2;
        let type_end = self
            .source
            .text
            .entire()
            .range(type_start..)
            .as_str()
            .expect("pinned builtin source is scalar")
            .find(['=', '\n'])
            .map_or(self.source.text.len_bytes(), |i| type_start + i);
        let type_end = self
            .source
            .text
            .entire()
            .range(..type_end)
            .as_str()
            .expect("pinned builtin source is scalar")
            .trim_end()
            .len();
        let type_span = Span::new(type_start, type_end);
        let check = code.len();
        code.extend([
            Instruction::CheckType(Box::new(ty.clone())),
            Instruction::Return,
        ]);
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
                        + self
                            .source
                            .text
                            .entire()
                            .range(start..)
                            .find(name)
                            .expect("builtin expression source");
                    Some(Span::new(offset, offset + name.len()))
                }
            })
            .collect()
    }
    pub(super) fn method(&mut self, class: &str, name: &str, body: Instruction) -> Member {
        let start = self
            .source
            .text
            .find(&format!("class {class}"))
            .expect("class");
        let start = start
            + self
                .source
                .text
                .entire()
                .range(start..)
                .find(&format!("external function {name}("))
                .expect("method");
        let end = start + self.source.text.entire().range(start..).find("\n").unwrap();
        let body_block = self.blocks.len();
        self.blocks.push(vec![
            Instruction::LoadLocal(0),
            Instruction::BindLocal {
                depth: Some(0),
                ty: Box::new(TypeCheck::Any),
            },
            Instruction::LoadLocal(0),
            body,
            Instruction::Return,
        ]);
        self.metadata.push(BlockMetadata {
            source: Some(self.source.clone()),
            member: format!("#{class}.{name}").into(),
            declaration: Some(Span::new(start, end)),
            method_signature: Some((format!("{name}(value)").into(), 1)),
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
            method_signature: Some((format!("{name}(value)").into(), 1)),
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
    pub(super) fn abstract_method(&mut self, class: &str, name: &str) -> Member {
        let start = self
            .source
            .text
            .find(&format!("class {class}"))
            .expect("class");
        let start = start
            + self
                .source
                .text
                .entire()
                .range(start..)
                .find(&format!("abstract function {name}("))
                .expect("abstract method");
        let end = start + self.source.text.entire().range(start..).find("\n").unwrap();
        let decl_span = Span::new(start, end);
        let member_name: PklString = format!("#{class}.{name}").into();
        let body_block = self.blocks.len();
        self.blocks.push(vec![
            Instruction::LoadLocal(0),
            Instruction::BindLocal {
                depth: Some(0),
                ty: Box::new(TypeCheck::Any),
            },
            Instruction::AbstractMember {
                name: member_name.clone(),
                method: true,
            },
            Instruction::Return,
        ]);
        self.metadata.push(BlockMetadata {
            source: Some(self.source.clone()),
            member: member_name.clone(),
            declaration: Some(decl_span),
            method_signature: Some((format!("{name}(value)").into(), 1)),
            spans: vec![None, None, Some(decl_span), None],
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
            member: member_name,
            declaration: Some(decl_span),
            method_signature: Some((format!("{name}(value)").into(), 1)),
            spans: vec![None; 2],
            ..Default::default()
        });
        Member {
            flags: crate::syntax::ModifierKind::Abstract.flag(),
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

#[cfg_attr(target_arch = "wasm32", inline(never))]
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
            Instruction::BuiltinClass(Box::new(builtins::ty(id))),
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
        Instruction::TestType(Box::new(builtins::ty(VALUE_RENDERER))),
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
        Instruction::TestType(Box::new(builtins::ty(BYTES_RENDERER))),
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
    let files_decl_start = d
        .source
        .text
        .find("files: Mapping<String, FileOutput>?")
        .expect("ModuleOutput.files");
    let files_key_start = files_decl_start + "files: Mapping<".len();
    let files_val_start = files_decl_start + "files: Mapping<String, ".len();
    let files_key_span = Span::new(files_key_start, files_key_start + "String".len());
    let files_val_span = Span::new(files_val_start, files_val_start + "FileOutput".len());
    let mut files = d.member(
        "ModuleOutput",
        "files",
        TypeCheck::Nullable(Box::new(TypeCheck::ParameterizedCollection {
            kind: crate::heap::CollectionKind::Mapping,
            arguments: vec![
                (TypeCheck::String, files_key_span),
                (builtins::ty(FILE_OUTPUT), files_val_span),
            ],
        })),
        vec![
            Instruction::BeginCollection {
                kind: crate::heap::CollectionKind::Mapping,
                has_parent: false,
                specialized: true,
            },
            Instruction::SetCollectionTypeDefault {
                ty: Box::new(builtins::ty(FILE_OUTPUT)),
                origin: Some(Box::new(RuntimeFrame {
                    anonymous: false,
                    source: Some(d.source.clone()),
                    span: files_val_span,
                    member: "#ModuleOutput.files.default".into(),
                })),
            },
            Instruction::EndCollection,
            Instruction::WrapNullableDefault { defined: true },
        ],
        false,
    );
    // NullableTypeNode reports the underlying Mapping<String, FileOutput> section.
    let files_type_span = files.type_span.as_mut().unwrap();
    files_type_span.end -= 1;
    let files_check = d.metadata[files.code].property_typecheck.unwrap();
    d.metadata[files.code].spans[files_check] = Some(*files_type_span);
    let output_value = d.member(
        "Module",
        "output",
        builtins::ty(MODULE_OUTPUT),
        vec![
            Instruction::BuiltinClass(Box::new(builtins::ty(MODULE_OUTPUT))),
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
    enum OutputFormatTarget {
        Builtin {
            class: usize,
            class_name: &'static str,
            expr: &'static str,
        },
        Module {
            load: fn() -> Instruction,
            ref_name: &'static str,
            expr: &'static str,
        },
    }
    let format_branches: [(&'static str, &'static str, &'static str, OutputFormatTarget); 9] = [
        (
            "json",
            "\"json\"",
            "format == \"json\"",
            OutputFormatTarget::Builtin {
                class: JSON_RENDERER,
                class_name: "JsonRenderer",
                expr: "new JsonRenderer {}",
            },
        ),
        (
            "jsonnet",
            "\"jsonnet\"",
            "format == \"jsonnet\"",
            OutputFormatTarget::Module {
                load: || Instruction::LoadJsonnetModule,
                ref_name: "jsonnet.Renderer",
                expr: "new jsonnet.Renderer {}",
            },
        ),
        (
            "pcf",
            "\"pcf\"",
            "format == \"pcf\"",
            OutputFormatTarget::Builtin {
                class: PCF_RENDERER,
                class_name: "PcfRenderer",
                expr: "new PcfRenderer {}",
            },
        ),
        (
            "plist",
            "\"plist\"",
            "format == \"plist\"",
            OutputFormatTarget::Builtin {
                class: PLIST_RENDERER,
                class_name: "PListRenderer",
                expr: "new PListRenderer {}",
            },
        ),
        (
            "properties",
            "\"properties\"",
            "format == \"properties\"",
            OutputFormatTarget::Builtin {
                class: PROPERTIES_RENDERER,
                class_name: "PropertiesRenderer",
                expr: "new PropertiesRenderer {}",
            },
        ),
        (
            "textproto",
            "\"textproto\"",
            "format == \"textproto\"",
            OutputFormatTarget::Module {
                load: || Instruction::LoadProtobufModule,
                ref_name: "protobuf.Renderer",
                expr: "new protobuf.Renderer {}",
            },
        ),
        (
            "xml",
            "\"xml\"",
            "format == \"xml\"",
            OutputFormatTarget::Module {
                load: || Instruction::LoadXmlModule,
                ref_name: "xml.Renderer",
                expr: "new xml.Renderer {}",
            },
        ),
        (
            "yaml",
            "\"yaml\"",
            "format == \"yaml\"",
            OutputFormatTarget::Builtin {
                class: YAML_RENDERER,
                class_name: "YamlRenderer",
                expr: "new YamlRenderer {}",
            },
        ),
        (
            "pkl-binary",
            "\"pkl-binary\"",
            "format == \"pkl-binary\"",
            OutputFormatTarget::Module {
                load: || Instruction::LoadPklBinaryModule,
                ref_name: "pklbinary.Renderer",
                expr: "new pklbinary.Renderer {}",
            },
        ),
    ];
    let mut rcode = vec![
        Instruction::Constant(Value::string("prop:pkl.outputFormat")),
        Instruction::ReadResourceOrNull,
        Instruction::Duplicate,
        Instruction::JumpIfNotNull(6),
        Instruction::Pop,
        Instruction::Constant(Value::string("pcf")),
    ];
    let mut rspan_names = vec![
        "\"prop:pkl.outputFormat\"",
        "read?(\"prop:pkl.outputFormat\")",
        "read?(\"prop:pkl.outputFormat\") ?? \"pcf\"",
        "read?(\"prop:pkl.outputFormat\") ?? \"pcf\"",
        "",
        "\"pcf\"",
    ];
    let mut end_jumps = Vec::new();
    for (format, lit_span, eq_span, target) in format_branches {
        rcode.push(Instruction::Duplicate);
        rspan_names.push("format");
        rcode.push(Instruction::Constant(Value::string(format)));
        rspan_names.push(lit_span);
        rcode.push(Instruction::Equal);
        rspan_names.push(eq_span);
        let branch_jump = rcode.len();
        rcode.push(Instruction::JumpIfFalse(0));
        rspan_names.push(eq_span);
        rcode.push(Instruction::Pop);
        rspan_names.push("");
        match target {
            OutputFormatTarget::Builtin {
                class,
                class_name,
                expr,
            } => {
                rcode.push(Instruction::BuiltinClass(Box::new(builtins::ty(class))));
                rspan_names.push(class_name);
                rcode.push(Instruction::MakeBuiltinInstance { template: empty });
                rspan_names.push(expr);
            }
            OutputFormatTarget::Module {
                load,
                ref_name,
                expr,
            } => {
                rcode.push(load());
                rspan_names.push(ref_name);
                rcode.push(Instruction::GetMember("Renderer".into()));
                rspan_names.push(ref_name);
                rcode.push(Instruction::MakeBuiltinInstance { template: empty });
                rspan_names.push(expr);
            }
        }
        end_jumps.push(rcode.len());
        rcode.push(Instruction::Jump(0));
        rspan_names.push("");
        let next_branch = rcode.len();
        rcode[branch_jump] = Instruction::JumpIfFalse(next_branch);
    }
    rcode.push(Instruction::OutputRenderer);
    rspan_names.push("throw(\n            \"Unknown output format:");
    let return_pc = rcode.len();
    for jump_pc in end_jumps {
        rcode[jump_pc] = Instruction::Jump(return_pc);
    }
    rcode.push(Instruction::Return);
    rspan_names.push("");
    d.blocks.push(rcode);
    d.metadata.push(BlockMetadata {
        source: Some(d.source.clone()),
        member: "#Module.output.renderer".into(),
        spans: d.expression_spans(module_start, &d.blocks[rblock], &rspan_names),
        ..Default::default()
    });
    let let_start = module_start
        + d.source
            .text
            .entire()
            .range(module_start..)
            .find("let (format =")
            .unwrap();
    let let_end = let_start
        + d.source
            .text
            .entire()
            .range(let_start..)
            .find("\n    text =")
            .unwrap();
    d.metadata[rblock].let_scopes.push(LetScope {
        body: 6..return_pc,
        frame: RuntimeFrame {
            anonymous: false,
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
    let text_start = module_start
        + d.source
            .text
            .entire()
            .range(module_start..)
            .find("text =")
            .unwrap();
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
    let bytes_start = text_start
        + d.source
            .text
            .entire()
            .range(text_start..)
            .find("bytes =")
            .unwrap();
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
        let start = converter_start
            + d.source
                .text
                .entire()
                .range(converter_start..)
                .find(text)
                .unwrap();
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
    let class_start = transformer_start
        + d.source
            .text
            .entire()
            .range(transformer_start..)
            .find("Class")
            .unwrap();
    let mixin_start = class_start
        + d.source
            .text
            .entire()
            .range(class_start..)
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
                ty: Box::new(mixin),
                origin: Some(Box::new(RuntimeFrame {
                    anonymous: false,
                    source: Some(d.source.clone()),
                    span: Span::new(mixin_start, mixin_start + "Mixin<ConvertProperty>".len()),
                    member: "#BaseValueRenderer.convertPropertyTransformers.default".into(),
                })),
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
    let json_extension_start = json_class
        + d.source
            .text
            .entire()
            .range(json_class..)
            .find("extension =")
            .unwrap();
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
    let relative_path = d.method(
        "Module",
        "relativePathTo",
        Instruction::ModuleRelativePathTo,
    );
    let Instruction::MakeFunction {
        block: relative_body,
        ..
    } = d.blocks[relative_path.code][0]
    else {
        unreachable!("ordinary external method wrapper")
    };
    let parameter_start = d
        .source
        .text
        .find("relativePathTo(other: Module)")
        .expect("pinned Module method")
        + "relativePathTo(other: ".len();
    d.blocks[relative_body] = vec![
        Instruction::LoadLocal(0),
        Instruction::CheckType(Box::new(builtins::ty(MODULE))),
        Instruction::Pop,
        Instruction::LoadLocal(0),
        Instruction::ModuleRelativePathTo,
        Instruction::Return,
    ];
    d.metadata[relative_body].spans = vec![
        None,
        Some(Span::new(parameter_start, parameter_start + "Module".len())),
        None,
        None,
        None,
        None,
    ];
    for block in [relative_path.code, relative_body] {
        d.metadata[block].method_signature = Some(("relativePathTo(other)".into(), 1));
    }
    let value_render_document = d.abstract_method("ValueRenderer", "renderDocument");
    let value_render_value = d.abstract_method("ValueRenderer", "renderValue");
    let bytes_render_document = d.abstract_method("BytesRenderer", "renderDocument");
    let bytes_render_value = d.abstract_method("BytesRenderer", "renderValue");
    BTreeMap::from([
        (MODULE, vec![output_value, relative_path]),
        (CONVERT_PROPERTY, vec![render]),
        (FILE_OUTPUT, vec![value, renderer, text, bytes]),
        (MODULE_OUTPUT, vec![files]),
        (BASE_RENDERER, vec![converters, transformers, extension]),
        (
            VALUE_RENDERER,
            vec![value_render_document, value_render_value],
        ),
        (
            BYTES_RENDERER,
            vec![bytes_render_document, bytes_render_value],
        ),
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
            && collections::typed_class_ref(&self.heap, *id)?.is_some_and(|c| c.id == MODULE_OUTPUT)
        {
            return Ok(());
        }
        let uri = self
            .context
            .primary_source
            .as_ref()
            .map_or_else(PklString::default, |s| s.uri.clone());
        let name =
            crate::diagnostic_values::type_name(value, &self.heap, &self.program.module_name)?;
        let mut error = RuntimeError::new(
            ErrorKind::Type,
            PklString::from("Expected `output` of module `")
                .concat(&uri)
                .concat(&"` to be of type `ModuleOutput`, but got type `".into())
                .concat(&name)
                .concat(&"`.".into()),
        );
        if let Some(Value::Object(module)) = self.inputs.first()
            && let Some(member) = self
                .heap
                .get(*module)?
                .members
                .iter()
                .find(|m| m.name == "output")
        {
            let metadata = &self.program.metadata[member.code];
            if let Some(span) = metadata.value_span {
                error.frames.push(RuntimeFrame {
                    anonymous: false,
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
        let Value::String(format) = format else {
            return Err(invalid("output format is not a String"));
        };
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

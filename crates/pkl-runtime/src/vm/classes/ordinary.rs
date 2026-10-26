//! Ordinary base classes whose identity and lazy members do not require the
//! external renderer or regular-expression algorithms.
use super::super::*;
use output::Definitions;

fn property(name: &str) -> Instruction {
    Instruction::GetProperty {
        name: name.into(),
        owner_depth: 0,
        local: false,
    }
}

fn literal(
    d: &mut Definitions<'_>,
    class: &str,
    name: &str,
    ty: TypeCheck,
    value: Value,
) -> Member {
    let member = d.member(class, name, ty, vec![Instruction::Constant(value)], false);
    let declaration = d.metadata[member.code].declaration.unwrap();
    let start = declaration.end
        + d.source.text[declaration.end..]
            .find('=')
            .expect("initializer")
        + 2;
    let end = start + d.source.text[start..].find('\n').unwrap();
    let span = Span::new(start, end);
    d.metadata[member.code].value_span = Some(span);
    d.metadata[member.code].spans[0] = Some(span);
    member
}

fn extension(d: &mut Definitions<'_>, class: &str, value: &str) -> Member {
    let start = d
        .source
        .text
        .find(&format!("class {class} "))
        .expect("renderer class");
    let start = start
        + d.source.text[start..]
            .find("extension =")
            .expect("extension");
    let literal_start = start + "extension = ".len();
    let span = Span::new(literal_start, literal_start + value.len() + 2);
    let code = d.blocks.len();
    d.blocks.push(vec![
        Instruction::Constant(Value::string(value)),
        Instruction::Return,
    ]);
    d.metadata.push(BlockMetadata {
        source: Some(d.source.clone()),
        member: format!("#{class}.extension").into(),
        declaration: Some(Span::new(start, span.end)),
        value_span: Some(span),
        spans: vec![Some(span), None],
        ..Default::default()
    });
    Member {
        name: "extension".into(),
        code,
        flags: 0,
        entry: false,
        local: false,
        method: false,
        ty: None,
        type_span: None,
        constant: false,
        fixed: false,
    }
}

fn yaml_mode(d: &Definitions<'_>) -> TypeCheck {
    let start = d
        .source
        .text
        .find("  mode: \"compat\"")
        .expect("YamlRenderer.mode")
        + "  mode: ".len();
    let literals = ["compat", "1.1", "1.2"];
    let mut offset = start;
    let spans: Vec<_> = literals
        .iter()
        .map(|literal| {
            let quoted = format!("\"{literal}\"");
            let start = offset + d.source.text[offset..].find(&quoted).unwrap();
            offset = start + quoted.len();
            Span::new(start, offset)
        })
        .collect();
    TypeCheck::Union {
        types: literals
            .into_iter()
            .map(|text| TypeCheck::StringLiteral(text.into()))
            .collect(),
        string_literals: true,
        labels: spans.clone(),
        spans,
        default: None,
    }
}

fn yaml_indent(d: &mut Definitions<'_>) -> TypeCheck {
    let start = d
        .source
        .text
        .find("indentWidth: Int(this > 1)")
        .expect("YamlRenderer.indentWidth")
        + "indentWidth: ".len();
    let span = Span::new(start + 4, start + "Int(this > 1".len());
    let this = Span::new(span.start, span.start + 4);
    let one = Span::new(span.end - 1, span.end);
    let block = d.blocks.len();
    d.blocks.push(vec![
        Instruction::LoadCustomThis,
        Instruction::Observe(this),
        Instruction::Constant(Value::Int(1.into())),
        Instruction::Observe(one),
        Instruction::Greater,
        Instruction::Observe(span),
        Instruction::Return,
    ]);
    d.metadata.push(BlockMetadata {
        source: Some(d.source.clone()),
        member: "#YamlRenderer.indentWidth".into(),
        spans: vec![Some(this), None, Some(one), None, Some(span), None, None],
        inline_constraint: true,
        ..Default::default()
    });
    TypeCheck::Constrained {
        base: Box::new(TypeCheck::Int),
        base_span: Span::new(start, start + 3),
        constraints: vec![ConstraintCode {
            block,
            arguments: BTreeMap::new(),
            argument_sources: BTreeMap::new(),
            span,
            module_environment: false,
            source: d.source.text[span.start..span.end].into(),
        }],
    }
}

fn regex_to_string(d: &mut Definitions<'_>) -> Member {
    let class = d.source.text.find("class RegexMatch {").unwrap();
    let start = class
        + d.source.text[class..]
            .find("function toString(): String = value")
            .unwrap();
    let header_end = start + "function toString(): String".len();
    let value = Span::new(header_end + 3, header_end + 8);
    let body = d.blocks.len();
    // Standard-library method return checks are disabled by AstBuilder.
    d.blocks.push(vec![property("value"), Instruction::Return]);
    d.metadata.push(BlockMetadata {
        source: Some(d.source.clone()),
        member: "#RegexMatch.toString".into(),
        declaration: Some(Span::new(start, header_end)),
        spans: vec![Some(value), None],
        ..Default::default()
    });
    let code = d.blocks.len();
    d.blocks.push(vec![
        Instruction::MakeFunction {
            block: body,
            arity: 0,
            method: true,
            capture_custom_this: false,
        },
        Instruction::Return,
    ]);
    d.metadata.push(BlockMetadata {
        source: Some(d.source.clone()),
        member: "#RegexMatch.toString".into(),
        declaration: Some(Span::new(start, header_end)),
        spans: vec![None; 2],
        ..Default::default()
    });
    Member {
        name: "toString".into(),
        code,
        flags: 0,
        entry: false,
        local: false,
        method: true,
        ty: None,
        type_span: None,
        constant: false,
        fixed: false,
    }
}

pub(in crate::vm) fn builtin_members(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<usize, Vec<Member>> {
    let mut d = Definitions {
        blocks,
        metadata,
        source: crate::stdlib_sources::base(),
    };
    let mut result = BTreeMap::new();
    let mut directive = Vec::new();
    for name in ["before", "value", "after"] {
        let (ty, body) = if name == "value" {
            (TypeCheck::Any, Instruction::Undefined(name.into()))
        } else {
            (
                TypeCheck::Nullable(Box::new(TypeCheck::String)),
                Instruction::Constant(Value::Null),
            )
        };
        let mut member = d.member("PcfRenderDirective", name, ty, vec![body], false);
        if name != "value" {
            // NullableTypeNode reports the underlying String node's section.
            let span = member.type_span.as_mut().unwrap();
            span.end -= 1;
            let check = d.metadata[member.code].property_typecheck.unwrap();
            d.metadata[member.code].spans[check] = Some(*span);
        }
        directive.push(member);
    }
    result.insert(output::PCF_RENDER_DIRECTIVE, directive);

    let mut regex = Vec::new();
    for (name, ty) in [
        ("value", TypeCheck::String),
        ("start", TypeCheck::Int),
        ("end", TypeCheck::Int),
    ] {
        regex.push(d.member(
            "RegexMatch",
            name,
            ty,
            vec![Instruction::Undefined(name.into())],
            false,
        ));
    }
    let group_start =
        d.source.text.find("groups: List<RegexMatch?>").unwrap() + "groups: List<".len();
    regex.push(d.member(
        "RegexMatch",
        "groups",
        TypeCheck::ParameterizedCollection {
            kind: crate::heap::CollectionKind::List,
            arguments: vec![(
                TypeCheck::Nullable(Box::new(builtins::ty(builtins::REGEX_MATCH))),
                Span::new(group_start, group_start + "RegexMatch".len()),
            )],
        },
        vec![Instruction::MakeList { length: 0 }],
        false,
    ));
    regex.push(regex_to_string(&mut d));
    result.insert(builtins::REGEX_MATCH, regex);

    for (id, name, suffix) in [
        (output::YAML_RENDERER, "YamlRenderer", "yaml"),
        (output::PLIST_RENDERER, "PListRenderer", "plist"),
        (
            output::PROPERTIES_RENDERER,
            "PropertiesRenderer",
            "properties",
        ),
    ] {
        let mut members = vec![extension(&mut d, name, suffix)];
        if id == output::YAML_RENDERER {
            let mode = yaml_mode(&d);
            members.push(literal(&mut d, name, "mode", mode, Value::string("compat")));
            let indent = yaml_indent(&mut d);
            members.push(literal(
                &mut d,
                name,
                "indentWidth",
                indent,
                Value::Int(2.into()),
            ));
        } else if id == output::PLIST_RENDERER {
            members.push(literal(
                &mut d,
                name,
                "indent",
                TypeCheck::String,
                Value::string("  "),
            ));
        }
        if id != output::PLIST_RENDERER {
            members.push(literal(
                &mut d,
                name,
                "omitNullProperties",
                TypeCheck::Boolean,
                Value::Bool(true),
            ));
        }
        if id == output::YAML_RENDERER {
            members.push(literal(
                &mut d,
                name,
                "isStream",
                TypeCheck::Boolean,
                Value::Bool(false),
            ));
        } else if id == output::PROPERTIES_RENDERER {
            members.push(literal(
                &mut d,
                name,
                "restrictCharset",
                TypeCheck::Boolean,
                Value::Bool(false),
            ));
        }
        for method in ["renderDocument", "renderValue"] {
            members.push(d.method(
                name,
                method,
                if id == output::YAML_RENDERER {
                    Instruction::RenderYaml {
                        document: method == "renderDocument",
                    }
                } else if id == output::PROPERTIES_RENDERER {
                    Instruction::RenderProperties {
                        document: method == "renderDocument",
                    }
                } else {
                    Instruction::Unsupported(format!("{name}.{method} is not implemented.").into())
                },
            ));
        }
        result.insert(id, members);
    }
    result
}

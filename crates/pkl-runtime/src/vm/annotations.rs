//! Declaration annotations use normal object constructors and VM frames.
use super::*;
mod java_decimal_digit_ranges;

pub(super) fn matches_module_version(value: &PklString) -> bool {
    let Some(value) = value.as_str() else {
        return false;
    };
    let mut count = 0;
    value.split('.').all(|part| {
        count += 1;
        let length = part.chars().count();
        (1..=2).contains(&length)
            && part.chars().all(|point| {
                let point = point as u32;
                let ranges = java_decimal_digit_ranges::JAVA_DECIMAL_DIGIT_RANGES;
                let index = ranges.partition_point(|(_, end)| *end < point);
                ranges.get(index).is_some_and(|(start, _)| *start <= point)
            })
    }) && count == 3
}

pub(crate) const ANNOTATION_ID: usize = usize::MAX - 100;
pub(crate) const DEPRECATED_ID: usize = usize::MAX - 101;
const NAMES: &[&str] = &[
    "Annotation",
    "Deprecated",
    "Since",
    "Unlisted",
    "AlsoKnownAs",
    "DocExample",
    "SourceCode",
    "ModuleInfo",
];

pub(crate) fn builtin_id(name: &str) -> Option<usize> {
    NAMES
        .iter()
        .position(|candidate| *candidate == name)
        .map(|index| ANNOTATION_ID - index)
}

pub(crate) fn builtin_name(id: usize) -> Option<&'static str> {
    ANNOTATION_ID
        .checked_sub(id)
        .and_then(|index| NAMES.get(index).copied())
}

pub(super) fn builtin_members(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<usize, Vec<Member>> {
    let source = crate::stdlib_sources::base();
    let nullable = || TypeCheck::Nullable(Box::new(TypeCheck::String));
    let listing = || TypeCheck::ParameterizedCollection {
        kind: crate::heap::CollectionKind::Listing,
        arguments: vec![(TypeCheck::String, Span::new(0, 0))],
    };
    let mut result = BTreeMap::new();
    for name in NAMES {
        let properties = match *name {
            "Deprecated" => vec![
                ("since", nullable()),
                ("message", nullable()),
                ("replaceWith", nullable()),
            ],
            "Since" => vec![("version", TypeCheck::String)],
            "ModuleInfo" => vec![("minPklVersion", TypeCheck::String)],
            "AlsoKnownAs" => vec![("names", listing())],
            "DocExample" => vec![("subjects", listing())],
            "SourceCode" => vec![
                ("language", TypeCheck::String),
                ("prefix", nullable()),
                ("suffix", nullable()),
            ],
            _ => vec![],
        };
        let class_start = source
            .text
            .find(&format!("class {name}"))
            .expect("pinned annotation class");
        let mut members = Vec::new();
        for (property, mut ty) in properties {
            let start = class_start
                + source.text[class_start..]
                    .find(&format!("{property}:"))
                    .expect("pinned annotation property");
            let name_span = Span::new(start, start + property.len());
            let type_start = name_span.end + 2;
            let mut type_end = source.text[type_start..]
                .find('\n')
                .map_or(source.text.len(), |end| type_start + end);
            if *name == "SourceCode" && property == "language" {
                type_end = type_start
                    + source.text[type_start..]
                        .find("\n\n")
                        .expect("source code language type end");
                let literals = [
                    "Go",
                    "HTML",
                    "Java",
                    "JavaScript",
                    "Markdown",
                    "Pkl",
                    "PklExpr",
                    "Python",
                    "Ruby",
                    "SQL",
                    "Swift",
                ];
                let mut offset = type_start;
                let mut spans = Vec::new();
                let mut types = Vec::new();
                for literal in literals {
                    let text = format!("\"{literal}\"");
                    let start = offset
                        + source.text[offset..type_end]
                            .find(&text)
                            .expect("literal union type");
                    offset = start + text.len();
                    spans.push(Span::new(start, offset));
                    types.push(TypeCheck::StringLiteral(literal.into()));
                }
                let start = offset
                    + source.text[offset..type_end]
                        .find("String")
                        .expect("string union type");
                spans.push(Span::new(start, start + 6));
                types.push(TypeCheck::String);
                ty = TypeCheck::Union {
                    string_literals: false,
                    types,
                    labels: spans.clone(),
                    spans,
                    default: None,
                };
            }
            if *name == "ModuleInfo" {
                // The pinned Unicode digit version constraint is a bounded native
                // implementation of its exact regex, using ordinary constraint
                // evaluation and its source-backed diagnostic metadata.
                let span = Span::new(type_start + "String(".len(), type_end - 1);
                let block = blocks.len();
                let regex_start = span.start + "matches(".len();
                let regex_span = Span::new(regex_start, span.end - 1);
                blocks.push(vec![
                    Instruction::LoadCustomThis,
                    Instruction::ObserveNativeLiteral {
                        span: regex_span,
                        rendered: source.text[regex_span.start..regex_span.end].into(),
                    },
                    Instruction::MatchModuleVersion,
                    Instruction::Observe(span),
                    Instruction::Return,
                ]);
                metadata.push(BlockMetadata {
                    source: Some(source.clone()),
                    member: "#ModuleInfo.minPklVersion".into(),
                    spans: vec![Some(span), None, Some(span), None, None],
                    inline_constraint: true,
                    ..Default::default()
                });
                ty = TypeCheck::Constrained {
                    base: Box::new(TypeCheck::String),
                    base_span: Span::new(type_start, type_start + 6),
                    constraints: vec![ConstraintCode {
                        block,
                        arguments: BTreeMap::new(),
                        argument_sources: BTreeMap::new(),
                        span,
                        module_environment: false,
                        source: source.text[span.start..span.end].into(),
                    }],
                };
            }
            if let TypeCheck::ParameterizedCollection { arguments, .. } = &mut ty {
                let start = type_start
                    + source.text[type_start..type_end]
                        .find("String")
                        .expect("annotation collection argument");
                arguments[0].1 = Span::new(start, start + 6);
            }
            let type_span = Span::new(
                type_start,
                if matches!(ty, TypeCheck::Nullable(_)) {
                    type_end - 1
                } else {
                    type_end
                },
            );
            let mut instructions = match &ty {
                TypeCheck::Nullable(_) => vec![Instruction::Constant(Value::Null)],
                TypeCheck::ParameterizedCollection { kind, .. } => vec![
                    Instruction::BeginCollection {
                        kind: *kind,
                        has_parent: false,
                        specialized: true,
                    },
                    Instruction::EndCollection,
                ],
                _ => vec![Instruction::Undefined(Arc::from(property))],
            };
            instructions.push(Instruction::CheckType(ty.clone()));
            instructions.push(Instruction::Return);
            let code = blocks.len();
            metadata.push(BlockMetadata {
                source: Some(source.clone()),
                spans: instructions
                    .iter()
                    .map(|instruction| {
                        Some(if matches!(instruction, Instruction::CheckType(_)) {
                            type_span
                        } else {
                            name_span
                        })
                    })
                    .collect(),
                member: format!("#{name}.{property}").into(),
                declaration: Some(Span::new(start, type_end)),
                value_span: Some(name_span),
                property_typecheck: Some(instructions.len() - 2),
                ..BlockMetadata::default()
            });
            blocks.push(instructions);
            members.push(Member {
                flags: 0,
                entry: false,
                name: Arc::from(property),
                code,
                local: false,
                method: false,
                ty: Some(ty),
                type_span: Some(type_span),
                constant: false,
                fixed: false,
            });
        }
        result.insert(builtin_id(name).expect("annotation builtin"), members);
    }
    result
}

impl Vm {
    pub(super) fn read_module_version(&mut self) -> Result<(), RuntimeError> {
        let module = self
            .stack
            .last()
            .ok_or_else(|| invalid("missing module version owner"))?
            .as_object()?;
        let annotation = self.heap.get(module)?.annotations.iter().find_map(|value| {
            let id = value.as_object().ok()?;
            let class = collections::typed_class(&self.heap, id).ok()??;
            (Some(class.id) == builtin_id("ModuleInfo")).then_some(id)
        });
        if let Some(annotation) = annotation {
            self.force(
                annotation,
                annotation,
                "minPklVersion".into(),
                false,
                true,
                false,
            )
        } else {
            self.stack.push(Value::Null);
            Ok(())
        }
    }

    pub(super) fn check_module_version(&mut self) -> Result<(), RuntimeError> {
        let value = self.pop()?;
        if value.is_null() {
            return Ok(());
        }
        let Value::String(version) = value else {
            return Err(invalid("checked ModuleInfo version"));
        };
        let parts = version
            .as_str()
            .filter(|value| value.is_ascii())
            .and_then(|text| {
                text.split('.')
                    .map(str::parse::<u32>)
                    .collect::<Result<Vec<_>, _>>()
                    .ok()
            });
        let Some(parts) = parts.filter(|parts| parts.len() == 3) else {
            return Err(RuntimeError::new(
                ErrorKind::Unsupported,
                "The Java ModuleInfo version parser fails internally for this Unicode version.",
            ));
        };
        if parts.as_slice() > [0, 32, 0].as_slice() {
            let module = self
                .stack
                .last()
                .ok_or_else(|| invalid("missing module version owner"))?
                .as_object()?;
            let unit = self
                .heap
                .get(module)?
                .module_unit
                .ok_or_else(|| invalid("missing module version unit"))?;
            return Err(RuntimeError::new(
                ErrorKind::Type,
                format!(
                    "Module `{}` requires Pkl version {}.{}.{} or higher, but your Pkl version is 0.32.0.",
                    self.program.units[unit.0 as usize].name, parts[0], parts[1], parts[2]
                ),
            ));
        }
        Ok(())
    }

    pub(super) fn append_annotation(&mut self) -> Result<(), RuntimeError> {
        let annotation = self.pop()?;
        let owner = self
            .stack
            .last()
            .ok_or_else(|| invalid("missing annotation owner"))?
            .heap_id()
            .ok_or_else(|| invalid("annotation owner is not a declaration"))?;
        self.heap.get_mut(owner)?.annotations.push(annotation);
        Ok(())
    }

    pub(super) fn initialize_deprecation(&mut self) -> Result<(), RuntimeError> {
        let owner = self
            .stack
            .last()
            .ok_or_else(|| invalid("missing annotation list"))?
            .as_object()?;
        let annotation = self.heap.get(owner)?.annotations.iter().find_map(|value| {
            let id = value.as_object().ok()?;
            let class = collections::typed_class(&self.heap, id).ok()??;
            (class.id == DEPRECATED_ID).then_some(id)
        });
        if let Some(id) = annotation {
            self.heap.get_mut(owner)?.deprecation = Some(PklString::default());
            let caller = self.frames.len() - 1;
            self.force(id, id, Arc::from("message"), false, true, false)?;
            if self.frames.len() > caller + 1 {
                self.frames[caller].callsite_hidden = true;
            }
        } else {
            self.stack.push(Value::Null);
        }
        Ok(())
    }

    pub(super) fn publish_member_annotations(&mut self, code: usize) -> Result<(), RuntimeError> {
        let annotations = self.pop()?.as_object()?;
        let owner = self
            .stack
            .last()
            .ok_or_else(|| invalid("missing annotation owner"))?
            .heap_id()
            .ok_or_else(|| invalid("annotation owner is not a declaration"))?;
        self.publish_annotations(owner, annotations, code)
    }

    pub(super) fn store_deprecation(&mut self) -> Result<(), RuntimeError> {
        let message = self.pop()?;
        let owner = self
            .stack
            .last()
            .ok_or_else(|| invalid("missing annotation list"))?
            .as_object()?;
        if let Value::String(message) = message {
            self.heap.get_mut(owner)?.deprecation = Some(message);
        }
        Ok(())
    }

    pub(super) fn publish_annotations(
        &mut self,
        owner: ObjectId,
        annotations: ObjectId,
        code: usize,
    ) -> Result<(), RuntimeError> {
        let values = self.heap.get(annotations)?.annotations.clone();
        let deprecation = self.heap.get(annotations)?.deprecation.clone();
        let owner = self.heap.get_mut(owner)?;
        owner.member_annotations.insert(code, values);
        if let Some(message) = deprecation {
            owner.member_deprecations.insert(code, message);
        }
        Ok(())
    }
}

impl Compiler<'_> {
    pub(super) fn declaration_annotations(
        &mut self,
        expression: &crate::syntax::Expr,
        output: &mut Code,
    ) -> Result<(), RuntimeError> {
        let annotations = self
            .resolved
            .module
            .members
            .iter()
            .find(|member| member.expr.span == expression.span)
            .map(|member| member.annotations.clone())
            .unwrap_or_default();
        self.annotations(&annotations, output)
    }

    pub(super) fn annotations(
        &mut self,
        annotations: &[crate::syntax::Expr],
        output: &mut Code,
    ) -> Result<(), RuntimeError> {
        for annotation in annotations {
            self.expression(annotation, output)?;
            output.push(Instruction::AppendAnnotation);
        }
        Ok(())
    }

    pub(super) fn member_annotations(
        &mut self,
        member: &crate::syntax::Member,
        output: &mut Code,
        class: bool,
    ) -> Result<(), RuntimeError> {
        output.push(Instruction::BeginMemberAnnotations);
        let previous = self.member.clone();
        self.member = if previous.is_empty() {
            format!("#{}", member.name).into()
        } else {
            previous.concat(&format!(".{}", member.name).into())
        };
        // A class member annotation executes in its class declaration's frame.
        let class_scope = class.then(|| self.type_scopes.pop().expect("class type scope"));
        let result = self.annotations(&member.annotations, output);
        if let Some(scope) = class_scope {
            self.type_scopes.push(scope);
        }
        self.member = previous;
        result
    }
}

#[cfg(test)]
mod tests;

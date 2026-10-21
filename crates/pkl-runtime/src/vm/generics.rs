//! Lexical type substitutions for reusable constraint bytecode.
use super::*;

impl TypeCheck {
    /// Resolve a finite type expression against the invocation that created it.
    /// Constraint bodies stay in Program; only their argument expressions are
    /// substituted, so changing recursive arguments never expand bytecode.
    pub(super) fn substituted(&self, environment: &Environment) -> Result<Self, RuntimeError> {
        use TypeCheck::*;
        let sub = |ty: &TypeCheck| ty.substituted(environment);
        Ok(match self {
            Parameter { block, name } => environment
                .type_arguments
                .get(&(*block, name.clone()))
                .cloned()
                .ok_or_else(|| invalid("unbound generic constraint parameter"))?,
            NamedAlias {
                declaration,
                name,
                arguments,
                expanded,
            } => NamedAlias {
                declaration: *declaration,
                name: name.clone(),
                arguments: arguments.iter().map(sub).collect::<Result<_, _>>()?,
                expanded: Box::new(sub(expanded)?),
            },
            Function {
                parameters,
                result,
                syntax,
            } => Function {
                parameters: parameters
                    .as_ref()
                    .map(|args| args.iter().map(sub).collect::<Result<_, _>>())
                    .transpose()?,
                result: Box::new(sub(result)?),
                syntax: *syntax,
            },
            Reference { domain, referent } => Reference {
                domain: Box::new(sub(domain)?),
                referent: Box::new(sub(referent)?),
            },
            ParameterizedCollection { kind, arguments } => ParameterizedCollection {
                kind: *kind,
                arguments: arguments
                    .iter()
                    .map(|(ty, span)| Ok((sub(ty)?, *span)))
                    .collect::<Result<_, RuntimeError>>()?,
            },
            Nullable(inner) => Nullable(Box::new(sub(inner)?)),
            Alias(inner) => Alias(Box::new(sub(inner)?)),
            GenericAlias(inner) => GenericAlias(Box::new(sub(inner)?)),
            Union {
                types,
                spans,
                labels,
                default,
            } => Union {
                types: types.iter().map(sub).collect::<Result<_, _>>()?,
                spans: spans.clone(),
                labels: labels.clone(),
                default: *default,
            },
            Constrained {
                base,
                base_span,
                constraints,
            } => Constrained {
                base: Box::new(sub(base)?),
                base_span: *base_span,
                constraints: constraints
                    .iter()
                    .map(|constraint| {
                        let mut result = constraint.clone();
                        result.arguments = constraint
                            .arguments
                            .iter()
                            .map(|(name, ty)| Ok((name.clone(), sub(ty)?)))
                            .collect::<Result<_, RuntimeError>>()?;
                        Ok(result)
                    })
                    .collect::<Result<_, RuntimeError>>()?,
            },
            _ => self.clone(),
        })
    }
}

impl TypeCheck {
    /// Whether selecting a default requires this invocation's actual argument.
    /// Nullable and List defaults do not depend on their element type; constraints
    /// do not run while obtaining the base type's default.
    pub(super) fn default_depends_on_parameters(&self) -> bool {
        match self {
            Self::Parameter { .. } => true,
            Self::NamedAlias { expanded, .. }
            | Self::Alias(expanded)
            | Self::GenericAlias(expanded)
            | Self::Constrained { base: expanded, .. } => expanded.default_depends_on_parameters(),
            Self::Union {
                types,
                default: Some(index),
                ..
            } => types[*index].default_depends_on_parameters(),
            Self::ParameterizedCollection { kind, arguments }
                if *kind != crate::heap::CollectionKind::List =>
            {
                arguments
                    .last()
                    .is_some_and(|(ty, _)| ty.default_depends_on_parameters())
            }
            _ => false,
        }
    }
}

impl Vm {
    /// Runtime counterpart of Compiler::default_type for a substituted argument.
    /// Type resolution has already initialized actual class arguments. Preserve
    /// their evaluator-local declaration identity and leave member bodies lazy.
    pub(super) fn generic_default_value(
        &mut self,
        ty: &TypeCheck,
        environment: &Environment,
    ) -> Result<Option<Value>, RuntimeError> {
        use crate::heap::CollectionKind;
        Ok(match ty {
            TypeCheck::Null | TypeCheck::Nullable(_) => Some(Value::Null),
            TypeCheck::StringLiteral(value) => Some(Value::String(value.clone())),
            TypeCheck::NamedAlias { expanded, .. }
            | TypeCheck::Alias(expanded)
            | TypeCheck::GenericAlias(expanded)
            | TypeCheck::Constrained { base: expanded, .. } => {
                return self.generic_default_value(expanded, environment);
            }
            TypeCheck::Union {
                types,
                default: Some(index),
                ..
            } => return self.generic_default_value(&types[*index], environment),
            TypeCheck::Class { id, .. } => {
                let object = self
                    .heap
                    .find_class(*id)
                    .ok_or_else(|| invalid("missing resolved default class"))?;
                if self
                    .heap
                    .get(object)?
                    .class_definition
                    .as_ref()
                    .is_none_or(|class| class.abstract_)
                {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        "Abstract class has no default value.",
                    ));
                }
                Some(Value::Object(object))
            }
            TypeCheck::Collection(kind) | TypeCheck::ParameterizedCollection { kind, .. } => {
                let mut object = Object {
                    collection_kind: *kind,
                    ..Object::default()
                };
                if *kind != CollectionKind::List {
                    object.environment = environment.clone();
                    object.specialized_literal = true;
                    if let TypeCheck::ParameterizedCollection { arguments, .. } = ty {
                        let (argument, _) = arguments
                            .last()
                            .ok_or_else(|| invalid("missing collection argument"))?;
                        if !argument.noop() {
                            object.collection_default =
                                Some(self.generic_default_value(argument, environment)?);
                        }
                    }
                }
                Some(Value::Object(self.heap.allocate(object)))
            }
            TypeCheck::Parameter { .. } => {
                return Err(invalid("unbound generic default parameter"));
            }
            _ => None,
        })
    }
}

impl TypeCheck {
    fn contains_parameter(&self) -> bool {
        match self {
            Self::Parameter { .. } => true,
            _ => self.type_children().any(Self::contains_parameter),
        }
    }

    /// Structural metadata children, excluding executable constraint bodies.
    fn type_children(&self) -> Box<dyn Iterator<Item = &Self> + '_> {
        match self {
            Self::NamedAlias {
                arguments,
                expanded,
                ..
            } => Box::new(arguments.iter().chain(std::iter::once(expanded.as_ref()))),
            Self::Function {
                parameters, result, ..
            } => Box::new(
                parameters
                    .iter()
                    .flatten()
                    .chain(std::iter::once(result.as_ref())),
            ),
            Self::Reference { domain, referent } => {
                Box::new([domain.as_ref(), referent.as_ref()].into_iter())
            }
            Self::Nullable(inner)
            | Self::Alias(inner)
            | Self::GenericAlias(inner)
            | Self::Constrained { base: inner, .. } => Box::new(std::iter::once(inner.as_ref())),
            Self::ParameterizedCollection { arguments, .. } => {
                Box::new(arguments.iter().map(|(ty, _)| ty))
            }
            Self::Union { types, .. } => Box::new(types.iter()),
            _ => Box::new(std::iter::empty()),
        }
    }

    pub(super) fn has_deferred_reference_validation(&self) -> bool {
        matches!(self, Self::Reference { referent, .. } if referent.contains_parameter())
            || self
                .type_children()
                .any(Self::has_deferred_reference_validation)
    }

    pub(super) fn validate_reference_arguments(
        &self,
        span: Span,
        aliases: &[RuntimeFrame],
        origins: &BTreeMap<usize, (Vec<RuntimeFrame>, Vec<RuntimeFrame>)>,
    ) -> Result<(), RuntimeError> {
        for child in self.type_children() {
            child.validate_reference_arguments(span, aliases, origins)?;
        }
        if let Self::Reference { referent, .. } = self
            && let Some(Self::Constrained { constraints, .. }) =
                reference_constrained_type(referent)
        {
            let mut error = RuntimeError::new(
                ErrorKind::Type,
                "`Reference` referent type argument may not include type constraints.",
            )
            .at(span);
            // Keep the compiler's existing origin-frame policy for actual
            // arguments; do not fabricate source locations from their values.
            if let Some((mut frames, origin_aliases)) = constraints
                .first()
                .and_then(|constraint| origins.get(&constraint.block))
                .cloned()
            {
                if origin_aliases.starts_with(aliases) {
                    frames.truncate(origin_aliases.len().saturating_sub(aliases.len() + 1));
                }
                frames.retain(|frame| {
                    std::iter::once(span)
                        .chain(aliases.iter().map(|alias| alias.span))
                        .all(|usage| frame.span.start < usage.start || frame.span.end > usage.end)
                });
                error.frames = frames;
            }
            return Err(error);
        }
        Ok(())
    }
}

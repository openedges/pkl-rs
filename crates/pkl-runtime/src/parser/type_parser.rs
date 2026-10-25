//! Explicit continuations for type syntax, including union and arrow binding.
use super::{Diagnostic, DiagnosticOrigin, Kind, Parser, Span, Type, TypeKind};

struct Parsed {
    ty: Type,
    height: usize,
    suffix: bool,
}

struct Union {
    start: usize,
    types: Vec<Type>,
    height: usize,
    default: Option<usize>,
    expectation: Option<&'static str>,
}

struct Named {
    start: usize,
    path: Vec<String>,
    name_spans: Vec<Span>,
    arguments: Vec<Type>,
    height: usize,
}

struct Parameters {
    expectation: Option<&'static str>,
    start: usize,
    parameters: Vec<Type>,
    height: usize,
}

enum Work {
    Type(Option<&'static str>),
    Leave,
    Atom(Option<&'static str>),
    Union(Union),
    Named(Named),
    Parameters(Parameters, bool),
    Return(Parameters),
    Suffix(usize),
}

struct Constraint {
    base: Parsed,
    start: usize,
}

impl Parser<'_> {
    pub(super) fn type_inner(
        &mut self,
        expectation: Option<&'static str>,
    ) -> Result<Type, Diagnostic> {
        let base_depth = self.depth;
        let mut work = vec![Work::Type(expectation)];
        let mut values = Vec::new();
        while let Some(task) = work.pop() {
            // The step's larger match frame is gone before constraints enter
            // the expression parser. Both parsers retain the shared depth.
            if let Some(request) = self.type_step(task, &mut work, &mut values, base_depth)? {
                let constraints = self.arguments()?;
                let height = request.base.height + 1;
                self.type_height(height, base_depth)?;
                values.push(Parsed {
                    ty: Type {
                        parenthesized: false,
                        unparenthesized_span: None,
                        kind: TypeKind::Constrained {
                            base: Box::new(request.base.ty),
                            constraints,
                        },
                        span: Span::new(request.start, self.previous_end),
                    },
                    height,
                    suffix: true,
                });
                work.push(Work::Suffix(request.start));
            }
        }
        Ok(values.pop().expect("type machine result").ty)
    }

    fn type_height(&self, height: usize, base_depth: usize) -> Result<(), Diagnostic> {
        if base_depth + height > 32 {
            return Err(self.type_limit());
        }
        Ok(())
    }

    fn type_limit(&self) -> Diagnostic {
        Diagnostic::limit(
            DiagnosticOrigin::Parser,
            "type-nesting",
            self.token.span,
            "Type nesting limit exceeded.",
        )
    }

    fn type_step(
        &mut self,
        task: Work,
        work: &mut Vec<Work>,
        values: &mut Vec<Parsed>,
        base_depth: usize,
    ) -> Result<Option<Constraint>, Diagnostic> {
        match task {
            Work::Type(expectation) => {
                if self.depth >= 32 {
                    return Err(self.type_limit());
                }
                self.depth += 1;
                let start = self.token.span.start;
                let default = if self.is_symbol('*') {
                    self.advance()?;
                    Some(0)
                } else {
                    None
                };
                work.push(Work::Leave);
                work.push(Work::Union(Union {
                    start,
                    types: Vec::new(),
                    height: 0,
                    default,
                    expectation,
                }));
                work.push(Work::Atom(expectation));
            }
            Work::Leave => self.depth -= 1,
            Work::Union(mut union) => {
                let value = values.pop().expect("type union member");
                union.height = union.height.max(value.height);
                union.types.push(value.ty);
                if self.is_symbol('|') {
                    self.advance()?;
                    if self.is_symbol('*') {
                        if union.default.is_some() {
                            return Err(Diagnostic::language(
                                DiagnosticOrigin::Parser,
                                super::DiagnosticKind::Syntax,
                                self.token.span,
                                "A type union cannot have more than one default type.",
                            ));
                        }
                        union.default = Some(union.types.len());
                        self.advance()?;
                    }
                    let expectation = union.expectation;
                    work.push(Work::Union(union));
                    work.push(Work::Atom(expectation));
                } else if union.types.len() == 1 {
                    if union.default.is_some() {
                        return Err(Diagnostic::language(
                            DiagnosticOrigin::Parser,
                            super::DiagnosticKind::Syntax,
                            Span::new(union.start, union.types[0].span.end),
                            "Only type unions can have a default marker (*).",
                        ));
                    }
                    values.push(Parsed {
                        ty: union.types.pop().unwrap(),
                        height: union.height,
                        suffix: true,
                    });
                } else {
                    let height = union.height + 1;
                    self.type_height(height, base_depth)?;
                    values.push(Parsed {
                        ty: Type {
                            parenthesized: false,
                            unparenthesized_span: None,
                            kind: TypeKind::Union {
                                types: union.types,
                                default: union.default,
                            },
                            span: Span::new(union.start, self.previous_end),
                        },
                        height,
                        suffix: true,
                    });
                }
            }
            Work::Atom(expectation) => {
                let start = self.token.span.start;
                work.push(Work::Suffix(start));
                let kind = match self.token.kind.clone() {
                    Kind::Name(name)
                        if matches!(name.as_str(), "unknown" | "nothing" | "module") =>
                    {
                        self.advance()?;
                        match name.as_str() {
                            "unknown" => TypeKind::Unknown,
                            "nothing" => TypeKind::Nothing,
                            _ => TypeKind::Module,
                        }
                    }
                    Kind::Name(_) | Kind::QuotedName(_) => {
                        let (name, name_span) = self.identifier()?;
                        let mut named = Named {
                            start,
                            path: vec![name],
                            name_spans: vec![name_span],
                            arguments: Vec::new(),
                            height: 0,
                        };
                        while self.is_symbol('.') {
                            self.advance()?;
                            let (name, span) = self.identifier()?;
                            named.path.push(name);
                            named.name_spans.push(span);
                        }
                        if self.is_symbol('<') {
                            self.advance()?;
                            if self.is_symbol('>') {
                                let mut error = self.unexpected(None);
                                error.message.push_str(" Expected a type.");
                                return Err(error);
                            }
                            work.push(Work::Named(named));
                            work.push(Work::Type(None));
                            return Ok(None);
                        }
                        TypeKind::Named {
                            path: named.path,
                            name_spans: named.name_spans,
                            arguments: named.arguments,
                        }
                    }
                    Kind::String(value) => {
                        self.advance()?;
                        TypeKind::String(value)
                    }
                    Kind::Symbol('(') => {
                        self.advance()?;
                        let empty = self.is_symbol(')');
                        work.push(Work::Parameters(
                            Parameters {
                                expectation,
                                start,
                                parameters: Vec::new(),
                                height: 0,
                            },
                            !empty,
                        ));
                        if !empty {
                            work.push(Work::Type(Some(")")));
                        }
                        return Ok(None);
                    }
                    _ => {
                        let token = &self.lexer.source[self.token.span.start..self.token.span.end];
                        let suffix = expectation
                            .map_or_else(|| ".".into(), |expected| format!(" or `{expected}`."));
                        return Err(Diagnostic::language(
                            DiagnosticOrigin::Parser,
                            super::DiagnosticKind::Syntax,
                            self.token.span,
                            format!("Unexpected token `{token}`. Expected a type{suffix}"),
                        ));
                    }
                };
                values.push(Parsed {
                    ty: Type {
                        parenthesized: false,
                        unparenthesized_span: None,
                        kind,
                        span: Span::new(start, self.previous_end),
                    },
                    height: 1,
                    suffix: true,
                });
            }
            Work::Named(mut named) => {
                let value = values.pop().expect("type argument");
                named.height = named.height.max(value.height);
                named.arguments.push(value.ty);
                if self.is_symbol(',') {
                    self.advance()?;
                    if !self.is_symbol('>') {
                        work.push(Work::Named(named));
                        work.push(Work::Type(None));
                        return Ok(None);
                    }
                }
                self.expect('>')?;
                let height = named.height + 1;
                self.type_height(height, base_depth)?;
                values.push(Parsed {
                    ty: Type {
                        parenthesized: false,
                        unparenthesized_span: None,
                        kind: TypeKind::Named {
                            path: named.path,
                            name_spans: named.name_spans,
                            arguments: named.arguments,
                        },
                        span: Span::new(named.start, self.previous_end),
                    },
                    height,
                    suffix: true,
                });
            }
            Work::Parameters(mut parameters, child) => {
                if child {
                    let value = values.pop().expect("type parameter");
                    parameters.height = parameters.height.max(value.height);
                    parameters.parameters.push(value.ty);
                    if self.is_symbol(',') {
                        self.advance()?;
                        if !self.is_symbol(')') {
                            work.push(Work::Parameters(parameters, true));
                            work.push(Work::Type(Some(")")));
                            return Ok(None);
                        }
                    }
                }
                if !self.is_symbol(')') {
                    return Err(self.unexpected(Some(",` or `)")));
                }
                let closing = self.token.span;
                self.advance()?;
                if self.is_operator("->") {
                    self.advance()?;
                    let expectation = parameters.expectation;
                    work.push(Work::Return(parameters));
                    work.push(Work::Type(expectation));
                } else {
                    if parameters.parameters.is_empty() {
                        // ParserImpl.parseTypeAtom reports the closing token
                        // of an empty non-function type, not a missing arrow.
                        return Err(Diagnostic::language(
                            DiagnosticOrigin::Parser,
                            super::DiagnosticKind::Syntax,
                            closing,
                            "Unexpected token `)`. Expected a type.",
                        ));
                    }
                    if parameters.parameters.len() != 1 {
                        return Err(self.unexpected(Some("->")));
                    }
                    let mut ty = parameters.parameters.pop().unwrap();
                    ty.unparenthesized_span.get_or_insert(ty.span);
                    ty.span = Span::new(parameters.start, self.previous_end);
                    ty.parenthesized = true;
                    values.push(Parsed {
                        ty,
                        height: parameters.height,
                        suffix: true,
                    });
                }
            }
            Work::Return(parameters) => {
                let value = values.pop().expect("function return type");
                let height = parameters.height.max(value.height) + 1;
                self.type_height(height, base_depth)?;
                let end = value.ty.span.end;
                values.push(Parsed {
                    ty: Type {
                        parenthesized: false,
                        unparenthesized_span: None,
                        kind: TypeKind::Function {
                            parameters: parameters.parameters,
                            return_type: Box::new(value.ty),
                        },
                        span: Span::new(parameters.start, end),
                    },
                    height,
                    // An arrow consumes the complete return type. Postfix
                    // syntax binds to that return type, not the function.
                    suffix: false,
                });
            }
            Work::Suffix(start) => {
                let mut value = values.pop().expect("type suffix base");
                if value.suffix {
                    while self.is_symbol('?') {
                        self.type_height(value.height + 1, base_depth)?;
                        let end = self.advance()?.span.end;
                        value = Parsed {
                            ty: Type {
                                parenthesized: false,
                                unparenthesized_span: None,
                                kind: TypeKind::Nullable(Box::new(value.ty)),
                                span: Span::new(start, end),
                            },
                            height: value.height + 1,
                            suffix: true,
                        };
                    }
                    if self.is_symbol('(') && self.same_line() {
                        self.type_height(value.height + 1, base_depth)?;
                        return Ok(Some(Constraint { base: value, start }));
                    }
                }
                values.push(value);
            }
        }
        Ok(None)
    }
}

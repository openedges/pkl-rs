//! Heap-backed emission continuations for ordinary authored expression trees.
use super::*;
use crate::heap::FastMap;
use crate::resolver::Binding;
use crate::syntax::{BinaryOp, Expr, ExprKind, UnaryOp};

fn literal_value(expression: &Expr) -> Option<Value> {
    Some(match &expression.kind {
        ExprKind::Int(value) => Value::Int((*value).into()),
        ExprKind::Float(value) => Value::Float((*value).into()),
        ExprKind::Null => Value::Null,
        ExprKind::String(value) => Value::string(value.source_literal()),
        ExprKind::Bool(value) => Value::Bool(*value),
        _ => return None,
    })
}

#[derive(Clone)]
// Keep declared parent recipes inline; boxing adds an allocation to each recipe
// and clone. Revisit this layout with compiler memory measurements.
#[allow(clippy::large_enum_variant)]
enum InferredParent {
    DeclaredType(TypeCheck, RuntimeFrame, bool),
    Property,
    Entry,
    EmptyDynamic,
}

fn register_inferred_parents(
    body: &Expr,
    parent: InferredParent,
    parents: &mut FastMap<Span, InferredParent>,
) {
    let mut bodies = vec![body];
    while let Some(body) = bodies.pop() {
        match &body.kind {
            ExprKind::Object { parent: None, .. } => {
                parents.insert(body.span, parent.clone());
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                bodies.push(else_branch);
                bodies.push(then_branch);
                bodies.push(condition);
            }
            ExprKind::Let { body, .. } => bodies.push(body),
            ExprKind::Trace(value) => bodies.push(value),
            _ => {}
        }
    }
}

enum Work<'a> {
    Visit(&'a Expr),
    Finish(&'a Expr, usize),
    Emit(Instruction, Option<Span>),
    SuperSubscript,
    BinaryRight(BinaryOp, &'a Expr),
    LogicalEnd(BinaryOp, usize),
    NullableMember(&'a Expr, &'a PklString),
    Lookup(&'a Expr, &'a Expr, &'a [Expr], usize),
    CallEnd(usize, bool, Option<usize>, Option<(usize, Span)>),
    LocalMethodCall(&'a Expr, usize, Span),
    Cast(TypeCheck, Span),
    IfThen(&'a Expr, &'a Expr, &'a Expr),
    IfElse(&'a Expr, usize),
    JumpEnd(usize),
    LetValue(&'a Expr),
    LetEnd(&'a Expr, usize, PklString),
    FunctionBegin(&'a Expr),
    FunctionEnd(&'a Expr, PklString, Code, Vec<usize>),
    ObjectBody(
        &'a [crate::syntax::ObjectMember],
        &'a [crate::syntax::Parameter],
        crate::heap::CollectionKind,
        bool,
        Option<Span>,
    ),
    CollectionBody(&'a Expr),
    ObjectEnd(Vec<crate::syntax::Member>, Option<Span>, usize, bool, bool),
    PropertyBegin(&'a crate::syntax::Member),
    PropertyEnd(Box<PropertyCompilation<'a>>),
}

struct PropertyCompilation<'a> {
    member: &'a crate::syntax::Member,
    previous_member: PklString,
    previous_modifiers: Vec<crate::syntax::Modifier>,
    previous_bound: bool,
    collection_scope: Vec<TypeScopeClass>,
    compiled_type: Option<TypeCheck>,
    initialization_error: Option<RuntimeError>,
    parent: Code,
    pending: Vec<usize>,
}

struct ExpressionContextState<'a, 'b> {
    output: &'b mut Code,
    work: &'b mut Vec<Work<'a>>,
    pending_spans: &'b mut Vec<usize>,
    inferred_parents: &'b mut FastMap<Span, InferredParent>,
    emitted_start: &'b mut usize,
}

impl Compiler<'_> {
    // PowerAssertions.isLiteral distinguishes parser literals on direct base
    // calls from AstBuilder's ConstantNode-folded collection constructors.
    // Resolve the method namespace first so authored shadows stay observable.
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn power_literal_constructor(&self, expression: &Expr) -> bool {
        let ExprKind::Call { callee, arguments } = &expression.kind else {
            return false;
        };
        let ExprKind::Name(name) = &callee.kind else {
            return false;
        };
        if self.resolved.names.get(&callee.span) != Some(&Binding::ImplicitMethod) {
            return false;
        }
        match name.as_str() {
            Some("List" | "Set" | "Map" | "Bytes") => self.power_constant_node(expression),
            Some("IntSeq" | "Pair" | "Regex" | "Null") => arguments
                .iter()
                .all(|argument| self.power_parser_literal(argument)),
            _ => false,
        }
    }

    fn power_parser_literal(&self, expression: &Expr) -> bool {
        let literal = match &expression.kind {
            ExprKind::Int(_)
            | ExprKind::Float(_)
            | ExprKind::Bool(_)
            | ExprKind::Null
            | ExprKind::String(_) => true,
            // Pinned StringLiteralExpr.hasInterpolation checks children.size()>1.
            // A sole interpolation is therefore classified as a parser literal
            // by PowerAssertions, while its inner expression remains visible.
            ExprKind::InterpolatedString(parts) => parts.len() == 1,
            _ => false,
        };
        literal
            // Java retains unary minus in its parser despite AstBuilder folding.
            && !self.resolved.module.folded_integer_spans.contains_key(&expression.span)
    }

    fn power_constant_node(&self, expression: &Expr) -> bool {
        let mut pending = vec![expression];
        while let Some(expression) = pending.pop() {
            match &expression.kind {
                ExprKind::Int(_)
                | ExprKind::Float(_)
                | ExprKind::Bool(_)
                | ExprKind::Null
                | ExprKind::String(_) => {}
                ExprKind::Parenthesized(inner) => pending.push(inner),
                ExprKind::Unary {
                    op: UnaryOp::Negate,
                    operand,
                } if matches!(operand.kind, ExprKind::Float(_)) => {}
                ExprKind::Member(receiver, name)
                    if (name
                        .as_str()
                        .and_then(crate::duration::DurationUnit::parse)
                        .is_some()
                        || name
                            .as_str()
                            .and_then(crate::data_size::DataSizeUnit::parse)
                            .is_some())
                        && matches!(
                            &receiver.unparenthesized().kind,
                            ExprKind::Int(_) | ExprKind::Float(_)
                        ) => {}
                ExprKind::Call { callee, arguments }
                    if self.resolved.names.get(&callee.span) == Some(&Binding::ImplicitMethod) =>
                {
                    let ExprKind::Name(name) = &callee.kind else {
                        return false;
                    };
                    match name.as_str() {
                        Some("List" | "Set" | "Map") => pending.extend(arguments),
                        Some("Bytes") if arguments.iter().all(|argument| {
                            matches!(argument.kind, ExprKind::Int(value) if (0..=127).contains(&value))
                                && !self.resolved.module.folded_integer_spans.contains_key(&argument.span)
                        }) => {},
                        _ => return false,
                    }
                }
                _ => return false,
            }
        }
        true
    }

    fn register_property_parents(
        &mut self,
        member: &crate::syntax::Member,
        parents: &mut FastMap<Span, InferredParent>,
    ) -> Result<(), RuntimeError> {
        let parent = if let Some(ty) = &member.ty {
            let check = self.compile_type(ty)?;
            if matches!(check.resolved(), TypeCheck::Unknown) {
                register_inferred_parents(&member.expr, InferredParent::EmptyDynamic, parents);
                return Ok(());
            }
            InferredParent::DeclaredType(
                check,
                RuntimeFrame {
                    anonymous: false,
                    source: None,
                    span: self.type_source_span(ty),
                    member: self.member.concat(&".default".into()),
                },
                false,
            )
        } else if !member.local {
            InferredParent::Property
        } else {
            InferredParent::EmptyDynamic
        };
        register_inferred_parents(&member.expr, parent, parents);
        Ok(())
    }

    pub(super) fn expression(
        &mut self,
        expression: &Expr,
        output: &mut Code,
    ) -> Result<(), RuntimeError> {
        self.expression_with_property_parent(expression, output, None)
    }

    pub(super) fn expression_with_property_parent(
        &mut self,
        expression: &Expr,
        output: &mut Code,
        property_parent: Option<&crate::syntax::Member>,
    ) -> Result<(), RuntimeError> {
        self.expression_context(expression, output, property_parent, false)
    }

    pub(super) fn expression_with_entry_parent(
        &mut self,
        expression: &Expr,
        output: &mut Code,
    ) -> Result<(), RuntimeError> {
        self.expression_context(expression, output, None, true)
    }

    fn expression_context(
        &mut self,
        expression: &Expr,
        output: &mut Code,
        property_parent: Option<&crate::syntax::Member>,
        entry_parent: bool,
    ) -> Result<(), RuntimeError> {
        if !self.instrument {
            if let Some(value) = literal_value(expression) {
                output.push_at(Instruction::Constant(value), expression.span);
                return Ok(());
            }
            match &expression.kind {
                ExprKind::This => {
                    let instr = if self.resolved.custom_this.contains(&expression.span) {
                        Instruction::LoadCustomThis
                    } else {
                        Instruction::LoadThis
                    };
                    output.push_at(instr, expression.span);
                    return Ok(());
                }
                ExprKind::Outer => {
                    output.push_at(Instruction::LoadOuter, expression.span);
                    return Ok(());
                }
                ExprKind::Module => {
                    output.push_at(Instruction::LoadModule, expression.span);
                    return Ok(());
                }
                _ => {}
            }
        }
        let mut work = Vec::with_capacity(8);
        work.push(Work::Visit(expression));
        // Children own their emitted source spans. Only outstanding instructions
        // need their parent's span; never rescan an already completed subtree.
        let mut pending_spans = Vec::new();
        let mut active_spans = Vec::new();
        // Only the new expressions connected directly to a method body through
        // if/let nodes inherit its return type. Nested properties and lambdas
        // establish their own inference contexts.
        let mut inferred_parents = FastMap::default();
        if let Some(member) = property_parent {
            self.register_property_parents(member, &mut inferred_parents)?;
        } else if entry_parent {
            register_inferred_parents(expression, InferredParent::Entry, &mut inferred_parents);
        }
        let result = (|| {
            while let Some(task) = work.pop() {
                let mut emitted_start = output.len();
                match task {
                    Work::Finish(expression, start) => {
                        active_spans.pop();
                        for index in pending_spans.drain(start..) {
                            output.spans[index] = Some(expression.span);
                        }
                        // In Java Pkl, GetModuleNode replaces itself with an uninstrumented
                        // receiver node without source section, so `module` is ignored by PowerAssertions.
                        if self.instrument
                            && !matches!(
                                expression.kind,
                                ExprKind::Parenthesized(_) | ExprKind::Module
                            )
                            && !self.power_literal_constructor(expression)
                            && !(matches!(expression.kind, ExprKind::InterpolatedString(_))
                                && self.power_parser_literal(expression))
                        {
                            output.push_at(Instruction::Observe(expression.span), expression.span);
                        }
                    }
                    Work::Emit(instruction, span) => match span {
                        Some(span) => output.push_at(instruction, span),
                        None => output.push(instruction),
                    },
                    Work::SuperSubscript => {
                        // The key has already been evaluated. A successful super
                        // read skips the fallback; otherwise its key remains
                        // rooted beneath the lazily read receiver.default.
                        output.push(Instruction::SuperSubscript {
                            end: output.len() + 3,
                        });
                        output.push(Instruction::Swap);
                        output.push(Instruction::ApplyEntryDefault);
                    }
                    Work::Cast(check, span) => {
                        output.type_casts.push(output.len());
                        output.push_at(Instruction::CheckType(Box::new(check)), span);
                    }
                    Work::NullableMember(expression, name) => {
                        output.push(Instruction::StripNullDefault);
                        output.push(Instruction::Duplicate);
                        let branch = output.len();
                        output.push(Instruction::JumpIfNull(0));
                        output.push(if self.resolved.const_names.contains(&expression.span) {
                            Instruction::GetConstMember(PklString::from(name))
                        } else {
                            Instruction::GetMember(PklString::from(name))
                        });
                        output[branch] = Instruction::JumpIfNull(output.len());
                    }
                    Work::BinaryRight(op, right) => {
                        if matches!(op, BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce) {
                            if op != BinaryOp::NullCoalesce {
                                output.push(Instruction::CheckLogicalLeft {
                                    or: op == BinaryOp::Or,
                                });
                            }
                            output.push(Instruction::Duplicate);
                            if op == BinaryOp::Or {
                                output.push(Instruction::Not);
                            }
                            let branch = output.len();
                            output.push(if op == BinaryOp::NullCoalesce {
                                Instruction::JumpIfNotNull(0)
                            } else {
                                Instruction::JumpIfFalse(0)
                            });
                            output.push(Instruction::Pop);
                            work.push(Work::LogicalEnd(op, branch));
                            work.push(Work::Visit(right));
                        } else if self.options.fuse_constant_add
                            && op == BinaryOp::Add
                            && let ExprKind::Int(value) = right.kind
                        {
                            output.push(Instruction::AddIntConstant(value));
                        } else {
                            let instruction = match op {
                                BinaryOp::Pipe => Instruction::PipeApply,
                                BinaryOp::Add => Instruction::Add,
                                BinaryOp::Subtract => Instruction::Subtract,
                                BinaryOp::Multiply => Instruction::Multiply,
                                BinaryOp::Divide => Instruction::Divide,
                                BinaryOp::IntegerDivide => Instruction::IntegerDivide,
                                BinaryOp::Remainder => Instruction::Remainder,
                                BinaryOp::Power => Instruction::Power,
                                BinaryOp::Equal => Instruction::Equal,
                                BinaryOp::NotEqual => Instruction::NotEqual,
                                BinaryOp::Less => Instruction::Less,
                                BinaryOp::LessEqual => Instruction::LessEqual,
                                BinaryOp::Greater => Instruction::Greater,
                                BinaryOp::GreaterEqual => Instruction::GreaterEqual,
                                BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce => {
                                    unreachable!()
                                }
                            };
                            work.push(Work::Emit(instruction, None));
                            work.push(Work::Visit(right));
                        }
                    }
                    Work::LogicalEnd(op, branch) => {
                        if op != BinaryOp::NullCoalesce {
                            output.push(Instruction::CheckLogicalRight {
                                or: op == BinaryOp::Or,
                            });
                        }
                        output[branch] = if op == BinaryOp::NullCoalesce {
                            Instruction::JumpIfNotNull(output.len())
                        } else {
                            Instruction::JumpIfFalse(output.len())
                        };
                    }
                    Work::Lookup(expression, callee, arguments, lookup_start) => {
                        self.compile_work_lookup(
                            expression,
                            callee,
                            arguments,
                            lookup_start,
                            output,
                            &mut work,
                        );
                    }
                    Work::LocalMethodCall(callee, arity, span) => {
                        self.compile_work_local_method_call(callee, arity, span, output);
                    }
                    Work::CallEnd(arity, method, nullable_branch, method_site) => {
                        if let Some((lookup, span)) = method_site {
                            output.push_at(
                                Instruction::PrepareMethodCall {
                                    lookup,
                                    arity,
                                    after_arguments: true,
                                },
                                span,
                            );
                        }
                        output.push(Instruction::Call { arity, method });
                        if let Some(branch) = nullable_branch {
                            output[branch] = Instruction::JumpIfNull(output.len());
                        }
                    }
                    Work::IfThen(condition, then_branch, else_branch) => {
                        let branch = output.len();
                        output.push_at(Instruction::JumpIfFalse(0), condition.span);
                        work.push(Work::IfElse(else_branch, branch));
                        work.push(Work::Visit(then_branch));
                    }
                    Work::IfElse(else_branch, branch) => {
                        let end = output.len();
                        output.push(Instruction::Jump(0));
                        output[branch] = Instruction::JumpIfFalse(output.len());
                        work.push(Work::JumpEnd(end));
                        work.push(Work::Visit(else_branch));
                    }
                    Work::JumpEnd(end) => output[end] = Instruction::Jump(output.len()),
                    Work::LetValue(expression) => {
                        let ExprKind::Let { body, ty, .. } = &expression.kind else {
                            unreachable!()
                        };
                        let check = if let Some(ty) = ty {
                            let Some(check) = self.resolve_expression_type(ty, output) else {
                                pending_spans.extend(
                                    (emitted_start..output.len())
                                        .filter(|&index| output.spans[index].is_none()),
                                );
                                continue;
                            };
                            Some((check, self.type_source_span(ty)))
                        } else {
                            None
                        };
                        let (ty, span) = check.unwrap_or((TypeCheck::Unknown, expression.span));
                        output.push_at(
                            Instruction::BindLocal {
                                depth: None,
                                ty: Box::new(ty),
                            },
                            span,
                        );
                        let previous = self.member.clone();
                        if self.let_members.last() != Some(&self.member) {
                            self.member = self.member.concat(&".<let expr>".into());
                        }
                        self.let_members.push(self.member.clone());
                        work.push(Work::LetEnd(expression, output.len(), previous));
                        work.push(Work::Visit(body));
                    }
                    Work::LetEnd(expression, start, previous) => {
                        let ExprKind::Let { name, .. } = &expression.kind else {
                            unreachable!()
                        };
                        output.let_scopes.push(LetScope {
                            body: start..output.len(),
                            writes_slot: name != "_",
                            frame: RuntimeFrame {
                                anonymous: false,
                                source: self.source.clone(),
                                span: expression.span,
                                member: self.member.clone(),
                            },
                        });
                        self.let_members.pop();
                        self.member = previous;
                        output.push(Instruction::PopLocal);
                    }
                    Work::FunctionBegin(..)
                    | Work::FunctionEnd(..)
                    | Work::CollectionBody(..)
                    | Work::ObjectBody(..)
                    | Work::ObjectEnd(..)
                    | Work::PropertyBegin(..)
                    | Work::PropertyEnd(..) => {
                        let mut ctx = ExpressionContextState {
                            output,
                            work: &mut work,
                            pending_spans: &mut pending_spans,
                            inferred_parents: &mut inferred_parents,
                            emitted_start: &mut emitted_start,
                        };
                        if self.compile_structured_work(task, &mut ctx)? {
                            continue;
                        }
                    }
                    Work::Visit(expression) => {
                        active_spans.push(expression.span);
                        work.push(Work::Finish(expression, pending_spans.len()));
                        if let Some(parent) = inferred_parents.remove(&expression.span) {
                            let ExprKind::Object {
                                members,
                                parameters,
                                ..
                            } = &expression.kind
                            else {
                                unreachable!()
                            };
                            let span = Span::new(expression.span.start, expression.span.start + 3);
                            let defined = match parent {
                                InferredParent::DeclaredType(ty, origin, cache) => {
                                    output.push_at(
                                        Instruction::ResolvedInferredParent {
                                            ty: Box::new(ty),
                                            origin: Some(Box::new(origin)),
                                            cache,
                                        },
                                        span,
                                    );
                                    true
                                }
                                InferredParent::Property => {
                                    output.push_at(Instruction::InferPropertyParent, span);
                                    true
                                }
                                InferredParent::Entry => {
                                    output.inferred_entry_parent_spans.push(span);
                                    output.push_at(Instruction::EntryDefault, span);
                                    output.push_at(Instruction::EntryKey, span);
                                    output.push_at(Instruction::ApplyEntryDefault, span);
                                    true
                                }
                                InferredParent::EmptyDynamic => {
                                    output.push_at(Instruction::EmptyDynamic, span);
                                    true
                                }
                            };
                            if defined {
                                if members.iter().all(|member| {
                                    matches!(member, crate::syntax::ObjectMember::Property(_))
                                }) {
                                    work.push(Work::ObjectBody(
                                        members,
                                        parameters,
                                        crate::heap::CollectionKind::Dynamic,
                                        true,
                                        Some(span),
                                    ));
                                } else {
                                    let start = output.len();
                                    self.collection_body(
                                        members,
                                        parameters,
                                        crate::heap::CollectionKind::Dynamic,
                                        true,
                                        output,
                                    )?;
                                    for location in &mut output.spans[start..] {
                                        if location.is_none() {
                                            *location = Some(span);
                                        }
                                    }
                                }
                            } else {
                                output.push_at(
                                    Instruction::Fail(Box::new(cannot_infer_parent())),
                                    span,
                                );
                            }
                        } else if let Some(value) = literal_value(expression) {
                            // Leaves do not need either large expression dispatcher.
                            // This also bounds annotation-property compilation.
                            output.push(Instruction::Constant(value));
                        } else if matches!(expression.kind, ExprKind::Annotation { .. }) {
                            // Annotation compilation can reenter the expression compiler.
                            // Do not retain either general expression-dispatch frame.
                            self.annotation_expression(expression, output)
                                .map_err(|error| error.at(expression.span))?;
                        } else if matches!(expression.kind, ExprKind::Class { .. }) {
                            // Class member compilation can reenter the expression compiler.
                            // Do not retain either general expression-dispatch frame.
                            self.class_declaration_expression(expression, output)
                                .map_err(|error| error.at(expression.span))?;
                        } else if matches!(expression.kind, ExprKind::TypeAlias { .. }) {
                            // Annotation compilation can reenter the expression compiler.
                            // Do not retain either general expression-dispatch frame.
                            self.type_alias_expression(expression, output)
                                .map_err(|error| error.at(expression.span))?;
                        } else {
                            self.schedule_expression(expression, output, &mut work)
                                .map_err(|error| error.at(expression.span))?;
                        }
                    }
                }
                pending_spans.extend(
                    (emitted_start..output.len()).filter(|&index| output.spans[index].is_none()),
                );
            }
            Ok(())
        })();
        result.map_err(|error: RuntimeError| {
            error.at(active_spans.last().copied().unwrap_or(expression.span))
        })
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn compile_work_lookup<'a>(
        &self,
        expression: &'a Expr,
        callee: &'a Expr,
        arguments: &'a [Expr],
        lookup_start: usize,
        output: &mut Code,
        work: &mut Vec<Work<'a>>,
    ) {
        let mut nullable_branch = None;
        let method = match &callee.kind {
            ExprKind::Member(receiver, name) | ExprKind::NullableMember(receiver, name) => {
                let nullable = matches!(callee.kind, ExprKind::NullableMember(..));
                if nullable {
                    output.push(Instruction::StripNullDefault);
                    output.push(Instruction::Duplicate);
                    nullable_branch = Some(output.len());
                    output.push(Instruction::JumpIfNull(0));
                }
                let constant = self.resolved.const_names.contains(&callee.span);
                let super_receiver =
                    !nullable && matches!(receiver.unparenthesized().kind, ExprKind::Super);
                let name = name.clone();
                output.push(match (constant, super_receiver) {
                    (true, true) => Instruction::GetConstSuperMethod(name.clone()),
                    (false, true) => Instruction::GetSuperMethod(name.clone()),
                    (true, false) => Instruction::GetConstMethod(name.clone()),
                    (false, false) => Instruction::GetMethod(name.clone()),
                });
                name != "apply"
            }
            _ => {
                if matches!(callee.kind, ExprKind::Name(_))
                    && matches!(
                        output.get(lookup_start),
                        Some(Instruction::RequireConstProperty { method: true, .. })
                    )
                {
                    output.spans[lookup_start] = Some(expression.span);
                }
                true
            }
        };
        let mut method_site = None;
        if matches!(
            callee.kind,
            ExprKind::Name(_) | ExprKind::Member(..) | ExprKind::NullableMember(..)
        ) && let Some(lookup) = output[lookup_start..].iter().rposition(|instruction| {
            matches!(
                instruction,
                Instruction::GetMethod(_)
                    | Instruction::GetConstMethod(_)
                    | Instruction::GetSuperMethod(_)
                    | Instruction::GetConstSuperMethod(_)
                    | Instruction::GetMethodProperty { .. }
                    | Instruction::GetImplicitMethod(_)
                    | Instruction::GetImplicitCustomMethod(_)
            )
        }) {
            let lookup = lookup_start + lookup;
            output.method_arities.insert(lookup, arguments.len());
            method_site = Some((lookup, expression.span));
            if matches!(callee.kind, ExprKind::Name(_)) {
                // The pinned implicit InvokeMethodVirtualNode
                // owns the full call, including its arguments.
                output.spans[lookup] = Some(expression.span);
            }
        }
        if let Some((lookup, span)) = method_site {
            output.push_at(
                Instruction::PrepareMethodCall {
                    lookup,
                    arity: 0,
                    after_arguments: false,
                },
                span,
            );
        }
        work.push(Work::CallEnd(
            arguments.len(),
            method,
            nullable_branch,
            method_site,
        ));
        work.extend(arguments.iter().rev().map(Work::Visit));
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn compile_work_local_method_call(
        &self,
        callee: &Expr,
        arity: usize,
        span: Span,
        output: &mut Code,
    ) {
        let ExprKind::Name(name) = &callee.kind else {
            unreachable!()
        };
        let Some(Binding::Method {
            owner_depth,
            local: true,
        }) = self.resolved.names.get(&callee.span)
        else {
            unreachable!()
        };
        let lookup = output.len();
        output.push_at(
            Instruction::GetMethodProperty {
                name: name.clone(),
                owner_depth: *owner_depth,
                local: true,
            },
            span,
        );
        output.method_arities.insert(lookup, arity);
        if self.instrument {
            output.push_at(Instruction::Observe(callee.span), callee.span);
        }
        output.push_at(Instruction::RotateStackRight(arity + 1), span);
        output.push_at(
            Instruction::PrepareMethodCall {
                lookup,
                arity,
                after_arguments: true,
            },
            span,
        );
        output.push_at(
            Instruction::Call {
                arity,
                method: true,
            },
            span,
        );
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn compile_structured_work<'a>(
        &mut self,
        task: Work<'a>,
        ctx: &mut ExpressionContextState<'a, '_>,
    ) -> Result<bool, RuntimeError> {
        match task {
            Work::FunctionBegin(expression) => {
                let ExprKind::Function {
                    type_parameters,
                    parameters,
                    return_type,
                    body,
                    method,
                    header_span,
                    ..
                } = &expression.kind
                else {
                    unreachable!()
                };
                if !type_parameters.is_empty() {
                    return Err(unsupported_type("generic methods").at(expression.span));
                }
                let previous = self.member.clone();
                if !method {
                    self.member =
                        self.function_names
                            .entry(expression.span)
                            .or_insert_with(|| {
                                if !self.replaying_prefix_error
                                    && let Some(owner) =
                                        self.resolved.function_owners.get(&expression.span)
                                {
                                    let nested_root =
                                        owner.encode_utf16().next() == Some(u16::from(b'.'));
                                    let prefix = if self.identity.as_ref().is_some_and(|identity| {
                                        identity.original_uri == "repl:text"
                                    }) {
                                        if nested_root {
                                            "repl:text"
                                        } else {
                                            "repl:text."
                                        }
                                    } else if nested_root {
                                        ""
                                    } else if self.module_annotation {
                                        "."
                                    } else {
                                        "#"
                                    };
                                    PklString::from(prefix).concat(owner)
                                } else {
                                    let count =
                                        self.function_counts.entry(previous.clone()).or_default();
                                    *count += 1;
                                    previous.concat(&format!(".<function#{count}>").into())
                                }
                            })
                            .clone();
                }
                let mut failed = false;
                if !method {
                    for ty in parameters
                        .iter()
                        .filter_map(|parameter| parameter.ty.as_ref())
                        .chain(return_type.iter())
                    {
                        if self.resolve_expression_type(ty, ctx.output).is_none() {
                            failed = true;
                            break;
                        }
                    }
                }
                if failed {
                    self.member = previous;
                } else {
                    let mut instructions = Code::default();
                    if *method {
                        let check = return_type
                            .as_ref()
                            .map(|ty| self.compile_type(ty))
                            .transpose()?
                            .unwrap_or(TypeCheck::Unknown);
                        let parent = if matches!(check.resolved(), TypeCheck::Unknown) {
                            InferredParent::EmptyDynamic
                        } else {
                            InferredParent::DeclaredType(
                                check,
                                RuntimeFrame {
                                    anonymous: false,
                                    source: None,
                                    span: *header_span,
                                    member: self.member.concat(&".default".into()),
                                },
                                true,
                            )
                        };
                        register_inferred_parents(body, parent, ctx.inferred_parents);
                    }
                    for (index, parameter) in parameters.iter().enumerate() {
                        let depth = parameters.len() - 1 - index;
                        instructions.push(Instruction::LoadLocal(depth));
                        let (ty, span) = match &parameter.ty {
                            Some(ty) => (self.compile_type(ty)?, self.type_source_span(ty)),
                            None => (TypeCheck::Unknown, parameter.span),
                        };
                        instructions.push_at(
                            Instruction::BindLocal {
                                depth: Some(depth),
                                ty: Box::new(ty),
                            },
                            span,
                        );
                    }
                    ctx.pending_spans.extend(
                        (*ctx.emitted_start..ctx.output.len())
                            .filter(|&index| ctx.output.spans[index].is_none()),
                    );
                    let parent = std::mem::replace(ctx.output, instructions);
                    let pending = std::mem::take(ctx.pending_spans);
                    ctx.work
                        .push(Work::FunctionEnd(expression, previous, parent, pending));
                    ctx.work.push(Work::Visit(body));
                    return Ok(true);
                }
            }
            Work::FunctionEnd(expression, previous, parent, pending) => {
                let ExprKind::Function {
                    parameters,
                    return_type,
                    body,
                    method,
                    header_span,
                    ..
                } = &expression.kind
                else {
                    unreachable!()
                };
                // AstBuilder disables class/module method return checks
                // in trusted stdlib modules outside Java's test mode.
                // ObjectMethodNode keeps its check even inside stdlib;
                // super_method_allowed is the existing declaration-context
                // distinction (false for object/amending-module methods).
                // Keep authored signatures, inferred parents and argument
                // checks; URI spelling alone never grants this policy.
                let check_return_type =
                    !(*method && self.resolved.standard_library && self.super_method_allowed);
                if check_return_type && let Some(ty) = return_type {
                    ctx.output.push_at(
                        Instruction::CheckType(Box::new(self.compile_type(ty)?)),
                        self.type_source_span(ty),
                    );
                }
                ctx.output.push(Instruction::Return);
                // The declaration/signature is authored, but Java's
                // external parser/renderer bodies have unavailable source sections.
                // Keep the authored signature's own locations.
                for index in 0..ctx.output.len() {
                    if matches!(
                        ctx.output[index],
                        Instruction::JsonParse
                            | Instruction::YamlParse { .. }
                            | Instruction::RenderJsonnet { .. }
                            | Instruction::RenderXml { .. }
                            | Instruction::RenderPklBinary { .. }
                            | Instruction::RenderProtobuf { .. }
                            | Instruction::ProtobufRenderType
                    ) {
                        ctx.output.spans[index] = None;
                    }
                }
                let instructions = std::mem::replace(ctx.output, parent);
                let block = self.block(
                    instructions,
                    Some(if *method {
                        *header_span
                    } else {
                        expression.span
                    }),
                )?;
                self.program.metadata[block].value_span = Some(self.executable_value_span(body));
                self.member = previous;
                *ctx.pending_spans = pending;
                *ctx.emitted_start = ctx.output.len();
                ctx.output.push(Instruction::MakeFunction {
                    block,
                    arity: parameters.len(),
                    method: *method,
                    capture_custom_this: self.resolved.custom_functions.contains(&expression.span),
                });
            }
            Work::CollectionBody(expression) => {
                let ExprKind::Object {
                    parent,
                    members,
                    parameters,
                } = &expression.kind
                else {
                    unreachable!("scheduled object body")
                };
                let begin = ctx.output.len();
                self.collection_body(
                    members,
                    parameters,
                    crate::heap::CollectionKind::Dynamic,
                    parent.is_some(),
                    ctx.output,
                )?;
                if let Some(parent) = parent {
                    for index in begin..ctx.output.len() {
                        if matches!(
                            ctx.output[index],
                            Instruction::PrepareFunctionAmendment { .. }
                                | Instruction::BeginFunctionAmendment { .. }
                                | Instruction::BeginCollection { .. }
                                | Instruction::AmendEmpty
                        ) {
                            ctx.output.spans[index] = Some(parent.unparenthesized().span);
                        }
                        if matches!(
                            ctx.output[index],
                            Instruction::BeginCollection { .. } | Instruction::AmendEmpty
                        ) {
                            break;
                        }
                    }
                }
            }
            Work::ObjectBody(members, parameters, kind, has_parent, parent_span) => {
                let mut begin = ctx.output.len();
                if has_parent && members.is_empty() {
                    ctx.output.push(Instruction::AmendEmpty);
                } else {
                    let properties = members
                        .iter()
                        .map(|member| match member {
                            crate::syntax::ObjectMember::Property(member) => member.clone(),
                            _ => unreachable!(),
                        })
                        .collect::<Vec<_>>();
                    if has_parent {
                        let start = ctx.output.len();
                        begin = self.compile_function_amendment(parameters, ctx.output)?;
                        if matches!(
                            ctx.output[start],
                            Instruction::PrepareFunctionAmendment { .. }
                        ) {
                            ctx.output.spans[start] = parent_span;
                        }
                        ctx.output.spans[begin] = parent_span;
                    }
                    let previous_super_method_allowed =
                        std::mem::replace(&mut self.super_method_allowed, false);
                    self.type_scopes.push(extract_type_scope(&properties));
                    ctx.output.push(Instruction::BeginCollection {
                        kind,
                        has_parent,
                        specialized: true,
                    });
                    ctx.work.push(Work::ObjectEnd(
                        properties,
                        parent_span,
                        begin,
                        !parameters.is_empty() && !members.is_empty(),
                        previous_super_method_allowed,
                    ));
                    ctx.work
                        .extend(members.iter().rev().map(|member| match member {
                            crate::syntax::ObjectMember::Property(member) => {
                                Work::PropertyBegin(member)
                            }
                            _ => unreachable!(),
                        }));
                }
                if has_parent && members.is_empty() {
                    ctx.output.spans[begin] = parent_span;
                }
            }
            Work::ObjectEnd(
                properties,
                parent,
                begin,
                has_parameters,
                previous_super_method_allowed,
            ) => {
                let parameter_check = has_parameters.then_some(ctx.output.len());
                if has_parameters {
                    ctx.output.push(Instruction::CheckObjectParameters {
                        target: amendment::ParameterTarget::Builder {
                            amendment_end: None,
                        },
                    });
                    *ctx.output.spans.last_mut().expect("parameter check") = parent;
                }
                ctx.output.push(Instruction::EndCollection);
                self.initialize_member_types(&properties, false, ctx.output)?;
                self.type_scopes.pop();
                self.super_method_allowed = previous_super_method_allowed;
                if matches!(
                    ctx.output[begin],
                    Instruction::BeginFunctionAmendment { .. }
                ) {
                    let end = ctx.output.len();
                    if let Instruction::BeginFunctionAmendment { end: target, .. } =
                        &mut ctx.output[begin]
                    {
                        *target = end;
                    }
                    if let Some(check) = parameter_check {
                        ctx.output[check] = Instruction::CheckObjectParameters {
                            target: amendment::ParameterTarget::Builder {
                                amendment_end: Some(end),
                            },
                        };
                    }
                    ctx.output.push(Instruction::EndFunctionAmendment);
                }
                if let Some(parent) = parent {
                    let constructor = begin
                        + usize::from(matches!(
                            ctx.output[begin],
                            Instruction::BeginFunctionAmendment { .. }
                        ));
                    ctx.output.spans[constructor] = Some(parent);
                    ctx.pending_spans.retain(|index| *index != constructor);
                }
            }
            Work::PropertyBegin(member) => {
                let collection_scope = self.type_scopes.pop().expect("collection scope");
                self.type_scopes
                    .push(extract_type_scope(std::slice::from_ref(member)));
                let previous_bound =
                    std::mem::replace(&mut self.constructor_parameters_bound, false);
                let mut visit = false;
                let previous_modifiers = if member.modifiers.is_empty() {
                    std::mem::take(&mut self.modifiers)
                } else {
                    std::mem::replace(&mut self.modifiers, member.modifiers.clone())
                };
                self.validate_modifiers(member)?;
                let previous_member = self.member.clone();
                self.member = append_member_name(&previous_member, &member.name, false);
                let mut initialization_error = None;
                let compiled_type = member.ty.as_ref().map(|ty| {
                    self.compile_type(ty).unwrap_or_else(|error| {
                        initialization_error = Some(error);
                        TypeCheck::Unknown
                    })
                });
                if initialization_error.is_none()
                    && let crate::syntax::ExprKind::Function {
                        parameters,
                        return_type,
                        method: true,
                        ..
                    } = &member.expr.kind
                {
                    for ty in parameters
                        .iter()
                        .filter_map(|p| p.ty.as_ref())
                        .chain(return_type.iter())
                    {
                        if let Err(error) = self.compile_type(ty) {
                            initialization_error = Some(error);
                            break;
                        }
                    }
                }
                let mut instructions = Code::default();
                if matches!(
                    member.expr.kind,
                    crate::syntax::ExprKind::Function { method: true, .. }
                ) {
                    instructions.push(Instruction::BeginObjectMethodSignature);
                    self.initialize_member_types(
                        std::slice::from_ref(member),
                        true,
                        &mut instructions,
                    )?;
                    instructions.push(Instruction::EndObjectMethodSignature);
                }
                if let Some(error) = &initialization_error {
                    // Annotation resolution runs when the owner is initialized. Keep a
                    // stub body so compilation cannot bypass earlier type dependencies.
                    instructions.push_at(
                        Instruction::Fail(Box::new(error.clone())),
                        error.span.unwrap_or(member.span),
                    );
                } else if matches!(member.expr.kind, crate::syntax::ExprKind::Uninitialized) {
                    self.default_property(member, &mut instructions)?;
                } else {
                    visit = true;
                }

                let parent = std::mem::replace(ctx.output, instructions);
                let pending = std::mem::take(ctx.pending_spans);
                ctx.work
                    .push(Work::PropertyEnd(Box::new(PropertyCompilation {
                        member,
                        previous_member,
                        previous_modifiers,
                        previous_bound,
                        collection_scope,
                        compiled_type,
                        initialization_error,
                        parent,
                        pending,
                    })));
                if visit {
                    self.register_property_parents(member, ctx.inferred_parents)?;
                    ctx.work.push(Work::Visit(&member.expr));
                }
                return Ok(true);
            }
            Work::PropertyEnd(state) => {
                let PropertyCompilation {
                    member,
                    previous_member,
                    previous_modifiers,
                    previous_bound,
                    collection_scope,
                    compiled_type,
                    initialization_error,
                    parent,
                    pending,
                } = *state;
                let mut instructions = std::mem::replace(ctx.output, parent);
                let constant_member = !(member.local && member.ty.is_some())
                    && self.constant_member_body(&member.expr)
                    && instructions.box_constant_result();
                let property_typecheck = member.ty.as_ref().map(|_| instructions.len());
                if let Some(ty) = &member.ty {
                    instructions.push_at(
                        Instruction::CheckType(Box::new(
                            compiled_type.clone().expect("member annotation"),
                        )),
                        self.type_source_span(ty),
                    );
                }
                instructions.push(Instruction::Return);
                let code = self.program.blocks.next_id();
                let header_span = Span::new(
                    member.span.start,
                    member
                        .ty
                        .as_ref()
                        .map_or(member.name_span.end, |ty| ty.span.end),
                );
                self.block(instructions, Some(header_span))?;
                self.program.metadata[code].amendment_parent_span =
                    member.is_amendment().then_some(member.name_span);
                self.program.metadata[code].hidden = member
                    .modifiers
                    .iter()
                    .any(|m| m.kind == crate::syntax::ModifierKind::Hidden);
                self.program.metadata[code].constant_member = constant_member;
                self.program.metadata[code].value_span = Some(self.property_value_span(member));
                self.program.metadata[code].member_span = Some(member.span);
                self.program.metadata[code].initialization_error = initialization_error;
                self.program.metadata[code].property_typecheck = property_typecheck;
                self.program.metadata[code].inline_property_typecheck = member.local;
                self.program.metadata[code].amendment = member.is_amendment();
                self.program.metadata[code].fixed_span = member.override_fixed_span;
                self.program.metadata[code].const_span = member.override_const_span;
                if let crate::syntax::ExprKind::Function {
                    parameters,
                    method: true,
                    ..
                } = &member.expr.kind
                {
                    self.program.metadata[code].method_signature = Some((
                        member
                            .name
                            .concat(&"(".into())
                            .concat(&PklString::join(
                                &parameters
                                    .iter()
                                    .map(|p| p.name.clone())
                                    .collect::<Vec<_>>(),
                                ", ",
                            ))
                            .concat(&")".into()),
                        parameters.len(),
                    ));
                }
                self.member = previous_member;
                self.modifiers = previous_modifiers;
                let compiled = Member {
                    flags: Member::declaration_flags(member),
                    entry: false,
                    name: member.name.clone(),
                    code,
                    local: crate::syntax::modifier_flags(&member.modifiers)
                        & crate::syntax::ModifierKind::Local.flag()
                        != 0,
                    ty: compiled_type,
                    type_span: member.ty.as_ref().map(|ty| self.type_source_span(ty)),
                    fixed: crate::syntax::modifier_flags(&member.modifiers)
                        & crate::syntax::ModifierKind::Fixed.flag()
                        != 0,
                    constant: crate::syntax::modifier_flags(&member.modifiers)
                        & crate::syntax::ModifierKind::Const.flag()
                        != 0
                        || matches!(
                            member.expr.kind,
                            crate::syntax::ExprKind::Class { .. }
                                | crate::syntax::ExprKind::TypeAlias { .. }
                        ),
                    method: matches!(
                        member.expr.kind,
                        crate::syntax::ExprKind::Function { method: true, .. }
                    ),
                };
                self.constructor_parameters_bound = previous_bound;
                self.program.objects.try_push(vec![compiled.clone()])?;
                self.type_scopes.pop();
                self.type_scopes.push(collection_scope);
                *ctx.pending_spans = pending;
                *ctx.emitted_start = ctx.output.len();
                let span = if let ExprKind::Function {
                    method: true,
                    header_span,
                    ..
                } = &member.expr.kind
                {
                    Span::new(member.span.start, header_span.end)
                } else {
                    member.name_span
                };
                ctx.output
                    .push_at(Instruction::AddCollectionProperty(Box::new(compiled)), span);
            }
            _ => unreachable!("structured work"),
        }
        Ok(false)
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn schedule_collection<'a>(
        &mut self,
        members: &'a [crate::syntax::ObjectMember],
        parameters: &'a [crate::syntax::Parameter],
        kind: crate::heap::CollectionKind,
        has_parent: bool,
        output: &mut Code,
        work: &mut Vec<Work<'a>>,
    ) -> Result<(), RuntimeError> {
        if members
            .iter()
            .all(|member| matches!(member, crate::syntax::ObjectMember::Property(_)))
        {
            work.push(Work::ObjectBody(
                members, parameters, kind, has_parent, None,
            ));
            Ok(())
        } else {
            self.collection_body(members, parameters, kind, has_parent, output)
        }
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn annotation_expression(
        &mut self,
        expression: &Expr,
        output: &mut Code,
    ) -> Result<(), RuntimeError> {
        let ExprKind::Annotation {
            parameters,
            ty,
            members,
        } = &expression.kind
        else {
            return Err(invalid("annotation expression"));
        };
        self.initialize_type_resolution(ty, output);
        match self.compile_type(ty) {
            Ok(check) => {
                if matches!(check.unaliased(), TypeCheck::Class { .. }) || check.has_declared() {
                    self.class_value(&check, output, ty.span)?;
                    output.push_at(Instruction::CheckAnnotationClass, ty.span);
                    let generator = members.iter().any(|member| {
                        !matches!(
                            member,
                            crate::syntax::ObjectMember::Property(_)
                                | crate::syntax::ObjectMember::Element { .. }
                                | crate::syntax::ObjectMember::Entry { .. }
                        )
                    });
                    if let Some(parameter) = parameters.first().filter(|_| generator) {
                        let (kind, message, span) = match &parameter.ty {
                            Some(ty) => (
                                ErrorKind::Type,
                                "Expected a function that returns an object, but got an object.",
                                ty.span,
                            ),
                            None => (
                                ErrorKind::Unsupported,
                                "Untyped parameterized annotation generators are not implemented.",
                                parameter.span,
                            ),
                        };
                        output.push_at(
                            Instruction::Fail(Box::new(RuntimeError::new(kind, message))),
                            span,
                        );
                    } else {
                        self.instance_body(members, parameters, ty.span, ty.span, output)?;
                        if !parameters.is_empty() && !members.is_empty() {
                            output.push_at(
                                Instruction::Fail(Box::new(RuntimeError::new(
                                    ErrorKind::Type,
                                    "Expected a function that returns an object, but got an object.",
                                ))),
                                ty.span,
                            );
                        }
                    }
                } else {
                    output.push_at(
                        Instruction::Fail(Box::new(RuntimeError::new(
                            ErrorKind::Type,
                            "Expected an annotation class.",
                        ))),
                        ty.span,
                    );
                }
            }
            Err(error) => {
                output.push_at(
                    Instruction::Fail(Box::new(error.clone())),
                    error.span.unwrap_or(ty.span),
                );
            }
        }
        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    fn schedule_expression<'a>(
        &mut self,
        expression: &'a Expr,
        output: &mut Code,
        work: &mut Vec<Work<'a>>,
    ) -> Result<(), RuntimeError> {
        match &expression.kind {
            ExprKind::Annotation { .. } => self.annotation_expression(expression, output)?,
            ExprKind::New {
                ty,
                members,
                parameters,
            } => {
                let check = match self.compile_type(ty) {
                    Ok(check) => check,
                    Err(error) => {
                        self.initialize_type_resolution(ty, output);
                        let span = error.span.unwrap_or(ty.span);
                        output.push_at(Instruction::Fail(Box::new(error)), span);
                        return Ok(());
                    }
                };
                self.initialize_type_resolution(ty, output);
                let check = if self.constructor_parameters_bound {
                    check
                } else {
                    let mut environment = Environment::default();
                    for parameter in self.active_type_parameters.values() {
                        if let TypeCheck::Parameter { block, name } = parameter {
                            environment
                                .type_arguments
                                .insert((*block, name.clone()), TypeCheck::Unknown);
                        }
                    }
                    check.substituted(&environment)?
                };
                // GetParentForTypeNode selects the default for every authored
                // type shape. Resolution/default work remains in the VM so a
                // deferred alias or module can suspend without recursive evaluation.
                output.push_at(
                    Instruction::GenericConstructorParent {
                        cache: !check.contains_parameter(),
                        ty: Box::new(check.clone()),
                        source: Box::new(TypeArgumentSource {
                            span: ty.span,
                            text: crate::source::SourceText::from(
                                self.resolved
                                    .module
                                    .source
                                    .entire()
                                    .range(ty.span.start..ty.span.end)
                                    .to_pkl_string(),
                            ),
                        }),
                    },
                    expression.span,
                );
                // Java adds a result cast only for an explicit declared type
                // with arguments, not for arbitrary composite type syntax.
                if matches!(&ty.kind, crate::syntax::TypeKind::Named { arguments, .. } if !arguments.is_empty())
                {
                    work.push(Work::Emit(
                        Instruction::CheckType(Box::new(check)),
                        Some(self.type_source_span(ty)),
                    ));
                }
                self.schedule_collection(
                    members,
                    parameters,
                    crate::heap::CollectionKind::Dynamic,
                    true,
                    output,
                    work,
                )?;
            }
            ExprKind::Object {
                parent,
                members,
                parameters,
            } if members
                .iter()
                .all(|member| matches!(member, crate::syntax::ObjectMember::Property(_))) =>
            {
                work.push(Work::ObjectBody(
                    members,
                    parameters,
                    crate::heap::CollectionKind::Dynamic,
                    parent.is_some(),
                    parent.as_ref().map(|parent| parent.unparenthesized().span),
                ));
                if let Some(parent) = parent {
                    work.push(Work::Visit(parent));
                }
            }
            ExprKind::Object { parent, .. } => {
                // Mixed bodies use the same parent worklist as property-only
                // bodies. Retaining a recursive expression compiler frame for
                // each amendment would exhaust small native thread stacks.
                work.push(Work::CollectionBody(expression));
                if let Some(parent) = parent {
                    work.push(Work::Visit(parent));
                }
            }
            ExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                work.push(Work::IfThen(condition, then_branch, else_branch));
                work.push(Work::Visit(condition));
            }
            ExprKind::Let { value, .. } => {
                work.push(Work::LetValue(expression));
                work.push(Work::Visit(value));
            }
            ExprKind::Function { .. } => work.push(Work::FunctionBegin(expression)),
            ExprKind::Parenthesized(value) => work.push(Work::Visit(value)),
            ExprKind::Unary { op, operand } => {
                work.push(Work::Emit(
                    match op {
                        UnaryOp::Negate => Instruction::Negate,
                        UnaryOp::Not => Instruction::Not,
                    },
                    None,
                ));
                work.push(Work::Visit(operand));
            }
            ExprKind::NonNull(value) => {
                work.push(Work::Emit(Instruction::NonNull, Some(value.span)));
                work.push(Work::Visit(value));
            }
            ExprKind::Throw(value) => {
                work.push(Work::Emit(Instruction::Throw, None));
                work.push(Work::Visit(value));
            }
            ExprKind::Trace(value) => {
                work.push(Work::Emit(
                    Instruction::Trace {
                        operand_span: self.executable_value_span(value),
                    },
                    None,
                ));
                work.push(Work::Visit(value));
            }
            ExprKind::ReadResourceOrNull(value) => {
                work.push(Work::Emit(Instruction::ReadResourceOrNull, None));
                work.push(Work::Visit(value));
            }
            ExprKind::ReadResource(value) => {
                work.push(Work::Emit(Instruction::ReadResource, None));
                work.push(Work::Visit(value));
            }
            ExprKind::ReadResourceGlob(value) => {
                let spec = self.glob_spec(expression, None)?;
                work.push(Work::Emit(
                    Instruction::ReadResourceGlob(Box::new(spec)),
                    None,
                ));
                work.push(Work::Visit(value));
            }
            ExprKind::TypeCheck { value, ty } | ExprKind::TypeCast { value, ty } => {
                if let Some(check) = self.resolve_expression_type_for_use_site(
                    ty,
                    output,
                    ReferenceUseSite::ExpressionCastOrTest,
                ) {
                    work.push(if matches!(expression.kind, ExprKind::TypeCast { .. }) {
                        Work::Cast(check, self.type_source_span(ty))
                    } else {
                        Work::Emit(Instruction::TestType(Box::new(check)), None)
                    });
                    work.push(Work::Visit(value));
                }
            }
            ExprKind::Binary { op, left, right } => {
                work.push(Work::BinaryRight(*op, right));
                work.push(Work::Visit(left));
            }
            ExprKind::Subscript { receiver, key } => {
                let super_receiver = matches!(receiver.unparenthesized().kind, ExprKind::Super);
                work.push(if super_receiver {
                    Work::SuperSubscript
                } else {
                    Work::Emit(
                        Instruction::Subscript {
                            key_span: self.executable_value_span(key),
                        },
                        None,
                    )
                });
                work.push(Work::Visit(key));
                if !super_receiver {
                    work.push(Work::Visit(receiver));
                }
            }
            ExprKind::Member(receiver, name) | ExprKind::NullableMember(receiver, name) => {
                if let Some(unit) = name.as_str().and_then(crate::duration::DurationUnit::parse) {
                    let number = match &receiver.unparenthesized().kind {
                        ExprKind::Int(value) => Some(*value as f64),
                        ExprKind::Float(value) => Some(*value),
                        ExprKind::Unary {
                            op: UnaryOp::Negate,
                            operand,
                        } => match operand.kind {
                            ExprKind::Float(value) => Some(-value),
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(value) = number {
                        // AstBuilder folds numeric literal units into one
                        // ConstantValueNode. Reusing this code preserves its
                        // wrapper identity; dynamic getters allocate per read.
                        output.push(Instruction::Constant(Value::Duration(
                            crate::duration::Duration::new(value, unit),
                        )));
                        return Ok(());
                    }
                }
                if let Some(unit) = name
                    .as_str()
                    .and_then(crate::data_size::DataSizeUnit::parse)
                {
                    let number = match &receiver.unparenthesized().kind {
                        ExprKind::Int(value) => Some(*value as f64),
                        ExprKind::Float(value) => Some(*value),
                        ExprKind::Unary {
                            op: UnaryOp::Negate,
                            operand,
                        } => match operand.kind {
                            ExprKind::Float(value) => Some(-value),
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(value) = number {
                        // AstBuilder folds numeric literal units into one
                        // ConstantValueNode. Reusing this code preserves its
                        // wrapper identity; dynamic getters allocate per read.
                        output.push(Instruction::Constant(Value::DataSize(
                            crate::data_size::DataSize::new(value, unit),
                        )));
                        return Ok(());
                    }
                }
                if matches!(expression.kind, ExprKind::NullableMember(_, _)) {
                    work.push(Work::NullableMember(expression, name));
                    work.push(Work::Visit(receiver));
                    return Ok(());
                }
                let constant = self.resolved.const_names.contains(&expression.span);
                let super_receiver = matches!(receiver.unparenthesized().kind, ExprKind::Super);
                let name = name.clone();
                work.push(Work::Emit(
                    match (constant, super_receiver) {
                        (true, true) => Instruction::GetConstSuperMember(name),
                        (false, true) => Instruction::GetSuperMember(name),
                        (true, false) => Instruction::GetConstMember(name),
                        (false, false) => Instruction::GetMember(name),
                    },
                    None,
                ));
                if !super_receiver {
                    work.push(Work::Visit(receiver));
                }
            }
            ExprKind::InterpolatedString(parts) => {
                if parts.is_empty() {
                    output.push(Instruction::Constant(Value::string("")));
                } else if parts.len() == 1 {
                    let part = &parts[0];
                    if matches!(part.kind, ExprKind::String(_)) && part.span == expression.span {
                        output.push_at(
                            Instruction::Constant(literal_value(part).expect("string chars")),
                            part.span,
                        );
                    } else if matches!(part.kind, ExprKind::String(_)) {
                        work.push(Work::Visit(part));
                    } else {
                        // AstBuilder reuses the complete string literal's
                        // source section for its sole interpolation child.
                        work.push(Work::Emit(Instruction::ToString, Some(expression.span)));
                        work.push(Work::Visit(part));
                    }
                } else {
                    work.push(Work::Emit(
                        Instruction::FinishInterpolation(parts.len()),
                        None,
                    ));
                    for part in parts.iter().rev() {
                        if matches!(part.kind, ExprKind::String(_)) {
                            if part.span == expression.span {
                                // Synthetic StringChars: do not increment observation count.
                                work.push(Work::Emit(
                                    Instruction::Constant(
                                        literal_value(part).expect("string chars"),
                                    ),
                                    Some(part.span),
                                ));
                            } else {
                                work.push(Work::Visit(part));
                            }
                        } else {
                            let part_span = self
                                .resolved
                                .module
                                .interpolation_spans
                                .get(&part.span)
                                .copied()
                                .unwrap_or(expression.span);
                            work.push(Work::Emit(Instruction::CheckInterpolate, Some(part_span)));
                            work.push(Work::Visit(part));
                        }
                    }
                }
            }
            ExprKind::Call { callee, arguments } => {
                if !self.super_method_allowed
                    && matches!(
                        &callee.kind,
                        ExprKind::Member(receiver, _)
                            if matches!(receiver.unparenthesized().kind, ExprKind::Super)
                    )
                {
                    let mut error = RuntimeError::new(
                        ErrorKind::Type,
                        "Cannot invoke a supermethod from here.",
                    )
                    .at(expression.span);
                    error.frames.push(RuntimeFrame {
                        anonymous: false,
                        span: expression.span,
                        member: self.member.clone(),
                        source: None,
                    });
                    return Err(error);
                }
                if let ExprKind::Name(name) = &callee.kind
                    && self.resolved.names.get(&callee.span) == Some(&Binding::ImplicitMethod)
                {
                    match name.as_str() {
                        Some("Bytes") => {
                            if arguments.is_empty() {
                                output.push(Instruction::Constant(Value::Bytes(
                                    crate::value::Bytes::empty(),
                                )));
                                return Ok(());
                            }
                            let constant = arguments
                                .iter()
                                .map(|argument| match argument.kind {
                                    ExprKind::Int(value)
                                        if (0..=127).contains(&value)
                                            && !self
                                                .resolved
                                                .module
                                                .source
                                                .entire()
                                                .range(argument.span.start..argument.span.end)
                                                .starts_with("-") =>
                                    {
                                        Some(value as u8)
                                    }
                                    _ => None,
                                })
                                .collect::<Option<Vec<_>>>();
                            if let Some(bytes) = constant {
                                output.push(Instruction::Constant(Value::Bytes(bytes.into())));
                            } else {
                                work.push(Work::Emit(
                                    Instruction::MakeBytes {
                                        length: arguments.len(),
                                    },
                                    None,
                                ));
                                for argument in arguments.iter().rev() {
                                    work.push(Work::Emit(
                                        Instruction::CheckByte,
                                        Some(self.executable_value_span(argument)),
                                    ));
                                    work.push(Work::Visit(argument));
                                }
                            }
                            return Ok(());
                        }
                        Some("Map") => {
                            if arguments.len() % 2 != 0 {
                                let span = Span::new(expression.span.end - 1, expression.span.end);
                                let mut error = RuntimeError::new(ErrorKind::Type,
                                    "Number of `Map()` arguments must be a multiple of two: `Map(key1, value1, key2, value2, ..., keyN, valueN)`").at(span);
                                error.frames.push(RuntimeFrame {
                                    anonymous: false,
                                    span,
                                    member: self.member.clone(),
                                    source: None,
                                });
                                return Err(error);
                            }
                            output.push(Instruction::BeginCollection {
                                kind: crate::heap::CollectionKind::Map,
                                has_parent: false,
                                specialized: false,
                            });
                            work.push(Work::Emit(Instruction::EndCollection, None));
                            for pair in arguments.as_chunks::<2>().0.iter().rev() {
                                work.push(Work::Emit(
                                    Instruction::AddMapEntry,
                                    Some(expression.span),
                                ));
                                work.push(Work::Emit(
                                    Instruction::PrepareMapEntry,
                                    Some(expression.span),
                                ));
                                work.push(Work::Visit(&pair[1]));
                                work.push(Work::Visit(&pair[0]));
                            }
                            return Ok(());
                        }
                        Some("Set") => {
                            output.push(Instruction::BeginCollection {
                                kind: crate::heap::CollectionKind::Set,
                                has_parent: false,
                                specialized: false,
                            });
                            work.push(Work::Emit(Instruction::EndCollection, None));
                            for argument in arguments.iter().rev() {
                                work.push(Work::Emit(
                                    Instruction::AddSetElement,
                                    Some(expression.span),
                                ));
                                work.push(Work::Emit(
                                    Instruction::PrepareSetElement,
                                    Some(expression.span),
                                ));
                                work.push(Work::Visit(argument));
                            }
                            return Ok(());
                        }
                        Some("List") => {
                            work.push(Work::Emit(
                                Instruction::MakeList {
                                    length: arguments.len(),
                                },
                                None,
                            ));
                            work.extend(arguments.iter().rev().map(Work::Visit));
                            return Ok(());
                        }
                        _ => {}
                    }
                }
                let callee = callee.unparenthesized();
                if let ExprKind::Name(name) = &callee.kind
                    && let Some(Binding::Method {
                        owner_depth,
                        local: true,
                    }) = self.resolved.names.get(&callee.span)
                {
                    // AbstractInvokeMethodLexicalNode checks const, evaluates
                    // arguments, then requests the local method's call target.
                    if self.resolved.const_names.contains(&callee.span) {
                        output.push_at(
                            Instruction::RequireConstProperty {
                                name: name.clone(),
                                owner_depth: *owner_depth,
                                local: true,
                                method: true,
                            },
                            expression.span,
                        );
                    }
                    work.push(Work::LocalMethodCall(
                        callee,
                        arguments.len(),
                        expression.span,
                    ));
                    work.extend(arguments.iter().rev().map(Work::Visit));
                    return Ok(());
                }
                work.push(Work::Lookup(expression, callee, arguments, output.len()));
                match &callee.kind {
                    ExprKind::Member(receiver, _)
                        if matches!(receiver.unparenthesized().kind, ExprKind::Super) => {}
                    ExprKind::Member(receiver, _) | ExprKind::NullableMember(receiver, _) => {
                        work.push(Work::Visit(receiver))
                    }
                    _ => work.push(Work::Visit(callee)),
                }
            }
            _ => self.inner_expression(expression, output)?,
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_module;
    use crate::resolver::resolve_module;

    #[test]
    fn constant_member_frames_retain_authored_header_suffixes() {
        for (source, expected) in [
            ("value: String = (-1)", "(-1)"),
            ("value: Int = (\"λ\")", "(\"λ\")"),
            ("value: Int = (false)", "(false)"),
            ("value: Int = (null)", "(null)"),
            ("value: Int = /* 한😀 */ (\"λ\")", "/* 한😀 */ (\"λ\")"),
            ("value: String = (-(1))", "-(1)"),
            ("value: String = (--1)", "--1"),
            ("value: Int = (List(-1, (\"λ\")))", "(List(-1, (\"λ\")))"),
            ("value: Int = (Set(-1))", "(Set(-1))"),
            ("value: Int = (Map(\"λ\", -1))", "(Map(\"λ\", -1))"),
            (
                "local value: Int = (List(-1, (\"λ\")))",
                "List(-1, (\"λ\"))",
            ),
        ] {
            let module = parse_module(source).unwrap();
            let resolved = resolve_module(&module).unwrap();
            let program = compile_module(&resolved).unwrap();
            let metadata = program
                .metadata
                .iter()
                .find(|metadata| metadata.member == "#value")
                .unwrap();
            let span = metadata.value_span.unwrap();
            assert_eq!(&source[span.start..span.end], expected, "{source}");
        }
    }

    #[test]
    fn executable_value_frames_keep_authored_assertion_observations() {
        for (authored, value, observations) in [
            ("-1", "1", vec!["-1"]),
            ("(-  /* gap */ 1)", "1", vec!["-  /* gap */ 1"]),
            (
                "((-0x8000000000000000))",
                "0x8000000000000000",
                vec!["-0x8000000000000000"],
            ),
            ("(-0.0)", "0.0", vec!["0.0", "-0.0"]),
            ("- 1.25", "1.25", vec!["1.25", "- 1.25"]),
            ("(-(1))", "-(1)", vec!["1", "-(1)"]),
            ("--1", "--1", vec!["-1", "--1"]),
            ("--1.25", "--1.25", vec!["1.25", "-1.25", "--1.25"]),
        ] {
            let source = format!("local value: Any = {authored}");
            let module = parse_module(&source).unwrap();
            let resolved = resolve_module(&module).unwrap();
            let authored_span = module.members[0].expr.span;
            assert_eq!(&source[authored_span.start..authored_span.end], authored);

            for instrument in [false, true] {
                let program =
                    compile_module_configured(&resolved, CompileOptions::default(), instrument)
                        .unwrap();
                let block = program
                    .metadata
                    .iter()
                    .position(|metadata| metadata.member == "#value")
                    .unwrap();
                let span = program.metadata[block].value_span.unwrap();
                assert_eq!(&source[span.start..span.end], value, "{authored}");
                let actual_observations: Vec<_> = program.blocks[block]
                    .iter()
                    .filter_map(|instruction| match instruction {
                        Instruction::Observe(span) => Some(&source[span.start..span.end]),
                        _ => None,
                    })
                    .collect();
                if instrument {
                    assert_eq!(actual_observations, observations, "{authored}");
                } else {
                    assert!(actual_observations.is_empty());
                }
            }
        }
    }
}

#[cfg(test)]
mod stdlib_return_policy_tests {
    use super::super::*;

    fn done(vm: &mut Vm) -> Result<Value, RuntimeError> {
        for _ in 0..10_000 {
            match vm.poll(1_000) {
                VmPoll::Ready(value) => return value,
                VmPoll::Yielded => {}
                other => panic!("unexpected return-policy effect: {other:?}"),
            }
        }
        panic!("bounded return-policy evaluation did not settle");
    }

    // Synthetic trusted AST controls exercise declaration context. They are
    // runtime invariants, not new Java observations or user-loadable stdlib.
    fn evaluate(source: &str, trusted: bool) -> Result<Value, RuntimeError> {
        let parsed = crate::parser::parse_module(source).unwrap();
        let resolved = if trusted {
            crate::resolver::resolve_stdlib_module(&parsed)
        } else {
            crate::resolver::resolve_module(&parsed)
        }
        .unwrap();
        let program = compile_module(&resolved).unwrap();
        let mut vm = Vm::new(program.clone());
        vm.start(program.entry, Vec::new()).unwrap();
        let module = done(&mut vm).unwrap().as_object().unwrap();
        vm.start_member(module, "result").unwrap();
        let result = done(&mut vm);
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
        result
    }

    #[test]
    fn trusted_stdlib_class_and_module_methods_skip_only_return_checks() {
        for source in [
            "function f(): Int = \"text\"\nresult = f()",
            "class C { function f(): Int = \"text\" }\nresult = new C {}.f()",
        ] {
            assert_eq!(
                evaluate(source, true).unwrap(),
                Value::String("text".into())
            );
            assert_eq!(evaluate(source, false).unwrap_err().kind, ErrorKind::Type);
        }
        let argument = "function f(x: Int): Int = x\nresult = f(\"text\")";
        assert_eq!(evaluate(argument, true).unwrap_err().kind, ErrorKind::Type);
    }

    #[test]
    fn object_local_methods_inside_trusted_modules_keep_return_checks() {
        let source = "holder = new Dynamic { local function f(): Int = \"text\"; result = f() }\nresult = holder.result";
        assert_eq!(evaluate(source, true).unwrap_err().kind, ErrorKind::Type);
    }
}

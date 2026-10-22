//! Heap-backed emission continuations for ordinary authored expression trees.
use super::*;
use crate::resolver::Binding;
use crate::syntax::{BinaryOp, Expr, ExprKind, UnaryOp};

#[derive(Clone)]
enum InferredParent {
    DeclaredType(TypeCheck, RuntimeFrame),
    Property,
}

fn register_inferred_parents(
    body: &Expr,
    parent: InferredParent,
    parents: &mut BTreeMap<Span, InferredParent>,
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
    BinaryRight(BinaryOp, &'a Expr),
    LogicalEnd(BinaryOp, usize),
    NullableMember(&'a Expr, &'a str),
    Lookup(&'a Expr, &'a Expr, &'a [Expr], usize),
    CallEnd(usize, bool, Option<usize>),
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
        crate::heap::CollectionKind,
        bool,
        Option<Span>,
    ),
    ObjectEnd(Vec<crate::syntax::Member>, Option<Span>, usize),
    PropertyBegin(&'a crate::syntax::Member),
    PropertyEnd(Box<PropertyCompilation<'a>>),
}

struct PropertyCompilation<'a> {
    member: &'a crate::syntax::Member,
    previous_member: PklString,
    previous_modifiers: Vec<crate::syntax::Modifier>,
    previous_bound: bool,
    collection_scope: Vec<crate::syntax::Member>,
    compiled_type: Option<TypeCheck>,
    initialization_error: Option<RuntimeError>,
    parent: Code,
    pending: Vec<usize>,
}

impl Compiler<'_> {
    fn register_property_parents(
        &mut self,
        member: &crate::syntax::Member,
        parents: &mut BTreeMap<Span, InferredParent>,
    ) -> Result<(), RuntimeError> {
        let parent = if let Some(ty) = &member.ty {
            let check = self.compile_type(ty)?;
            if matches!(check, TypeCheck::Unknown) {
                return Ok(());
            }
            InferredParent::DeclaredType(
                check,
                RuntimeFrame {
                    source: None,
                    span: self.type_source_span(ty),
                    member: self.member.concat(&".default".into()),
                },
            )
        } else if !member.local {
            InferredParent::Property
        } else {
            return Ok(());
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
        let mut work = vec![Work::Visit(expression)];
        // Children own their emitted source spans. Only outstanding instructions
        // need their parent's span; never rescan an already completed subtree.
        let mut pending_spans = Vec::new();
        let mut active_spans = Vec::new();
        // Only the new expressions connected directly to a method body through
        // if/let nodes inherit its return type. Nested properties and lambdas
        // establish their own inference contexts.
        let mut inferred_parents = BTreeMap::new();
        if let Some(member) = property_parent {
            self.register_property_parents(member, &mut inferred_parents)?;
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
                        if self.instrument && !matches!(expression.kind, ExprKind::Parenthesized(_))
                        {
                            output.push_at(Instruction::Observe(expression.span), expression.span);
                        }
                    }
                    Work::Emit(instruction, span) => match span {
                        Some(span) => output.push_at(instruction, span),
                        None => output.push(instruction),
                    },
                    Work::Cast(check, span) => {
                        output.type_casts.push(output.len());
                        output.push_at(Instruction::CheckType(check), span);
                    }
                    Work::NullableMember(expression, name) => {
                        output.push(Instruction::StripNullDefault);
                        output.push(Instruction::Duplicate);
                        let branch = output.len();
                        output.push(Instruction::JumpIfNull(0));
                        output.push(if self.resolved.const_names.contains(&expression.span) {
                            Instruction::GetConstMember(Arc::from(name))
                        } else {
                            Instruction::GetMember(Arc::from(name))
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
                        let mut nullable_branch = None;
                        let method = match &callee.kind {
                            ExprKind::Member(receiver, name)
                            | ExprKind::NullableMember(receiver, name) => {
                                let nullable = matches!(callee.kind, ExprKind::NullableMember(..));
                                if nullable {
                                    output.push(Instruction::StripNullDefault);
                                    output.push(Instruction::Duplicate);
                                    nullable_branch = Some(output.len());
                                    output.push(Instruction::JumpIfNull(0));
                                }
                                let constant = self.resolved.const_names.contains(&callee.span);
                                let super_receiver = !nullable
                                    && matches!(receiver.unparenthesized().kind, ExprKind::Super);
                                let name: Arc<str> = Arc::from(name.as_str());
                                output.push(match (constant, super_receiver) {
                                    (true, true) => Instruction::GetConstSuperMethod(name.clone()),
                                    (false, true) => Instruction::GetSuperMethod(name.clone()),
                                    (true, false) => Instruction::GetConstMethod(name.clone()),
                                    (false, false) => Instruction::GetMethod(name.clone()),
                                });
                                name.as_ref() != "apply"
                            }
                            _ => {
                                if matches!(callee.kind, ExprKind::Name(_))
                                    && matches!(
                                        output.get(lookup_start),
                                        Some(Instruction::RequireConstProperty {
                                            method: true,
                                            ..
                                        })
                                    )
                                {
                                    output.spans[lookup_start] = Some(expression.span);
                                }
                                true
                            }
                        };
                        if matches!(
                            callee.kind,
                            ExprKind::Name(_) | ExprKind::Member(..) | ExprKind::NullableMember(..)
                        ) && let Some(lookup) =
                            output[lookup_start..].iter().rposition(|instruction| {
                                matches!(
                                    instruction,
                                    Instruction::GetMethod(_)
                                        | Instruction::GetConstMethod(_)
                                        | Instruction::GetSuperMethod(_)
                                        | Instruction::GetConstSuperMethod(_)
                                        | Instruction::GetMethodProperty { .. }
                                        | Instruction::GetImplicitMethod(_)
                                )
                            })
                        {
                            let lookup = lookup_start + lookup;
                            output.method_arities.insert(lookup, arguments.len());
                            if matches!(callee.kind, ExprKind::Name(_)) {
                                // The pinned implicit InvokeMethodVirtualNode
                                // owns the full call, including its arguments.
                                output.spans[lookup] = Some(expression.span);
                            }
                        }
                        work.push(Work::CallEnd(arguments.len(), method, nullable_branch));
                        work.extend(arguments.iter().rev().map(Work::Visit));
                    }
                    Work::CallEnd(arity, method, nullable_branch) => {
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
                        if let Some((check, span)) = check {
                            output.push_at(Instruction::CheckType(check), span);
                        }
                        output.push(Instruction::PushLocal);
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
                                source: None,
                                span: expression.span,
                                member: self.member.clone(),
                            },
                        });
                        self.let_members.pop();
                        self.member = previous;
                        output.push(Instruction::PopLocal);
                    }
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
                            self.member = self
                                .function_names
                                .entry(expression.span)
                                .or_insert_with(|| {
                                    let count =
                                        self.function_counts.entry(previous.clone()).or_default();
                                    *count += 1;
                                    previous.concat(&format!(".<function#{count}>").into())
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
                                if self.resolve_expression_type(ty, output).is_none() {
                                    failed = true;
                                    break;
                                }
                            }
                        }
                        if failed {
                            self.member = previous;
                        } else {
                            let mut instructions = Code::default();
                            if *method && let Some(ty) = return_type {
                                let check = self.compile_type(ty)?;
                                if !matches!(check, TypeCheck::Unknown) {
                                    register_inferred_parents(
                                        body,
                                        InferredParent::DeclaredType(
                                            check,
                                            RuntimeFrame {
                                                source: None,
                                                span: *header_span,
                                                member: self.member.concat(&".default".into()),
                                            },
                                        ),
                                        &mut inferred_parents,
                                    );
                                }
                            }
                            for (index, parameter) in parameters.iter().enumerate() {
                                if let Some(ty) = &parameter.ty {
                                    instructions
                                        .push(Instruction::LoadLocal(parameters.len() - 1 - index));
                                    instructions.push_at(
                                        Instruction::CheckType(self.compile_type(ty)?),
                                        self.type_source_span(ty),
                                    );
                                    instructions.push(Instruction::StoreLocal(
                                        parameters.len() - 1 - index,
                                    ));
                                }
                            }
                            pending_spans.extend(
                                (emitted_start..output.len())
                                    .filter(|&index| output.spans[index].is_none()),
                            );
                            let parent = std::mem::replace(output, instructions);
                            let pending = std::mem::take(&mut pending_spans);
                            work.push(Work::FunctionEnd(expression, previous, parent, pending));
                            work.push(Work::Visit(body));
                            continue;
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
                        if let Some(ty) = return_type {
                            output.push_at(
                                Instruction::CheckType(self.compile_type(ty)?),
                                self.type_source_span(ty),
                            );
                        }
                        output.push(Instruction::Return);
                        let instructions = std::mem::replace(output, parent);
                        let block = self.block(
                            instructions,
                            Some(if *method {
                                *header_span
                            } else {
                                expression.span
                            }),
                        );
                        self.program.metadata[block].value_span =
                            Some(self.executable_value_span(body));
                        self.member = previous;
                        pending_spans = pending;
                        emitted_start = output.len();
                        output.push(Instruction::MakeFunction {
                            block,
                            arity: parameters.len(),
                            method: *method,
                            capture_custom_this: self
                                .resolved
                                .custom_functions
                                .contains(&expression.span),
                        });
                    }
                    Work::ObjectBody(members, kind, has_parent, parent_span) => {
                        let begin = output.len();
                        if has_parent && members.is_empty() {
                            output.push(Instruction::AmendEmpty);
                        } else {
                            let properties = members
                                .iter()
                                .map(|member| match member {
                                    crate::syntax::ObjectMember::Property(member) => member.clone(),
                                    _ => unreachable!(),
                                })
                                .collect::<Vec<_>>();
                            self.type_scopes.push(properties.clone());
                            if has_parent {
                                output.push(Instruction::BeginFunctionAmendment { end: 0 });
                            }
                            output.push(Instruction::BeginCollection {
                                kind,
                                has_parent,
                                specialized: true,
                            });
                            work.push(Work::ObjectEnd(properties, parent_span, begin));
                            work.extend(members.iter().rev().map(|member| match member {
                                crate::syntax::ObjectMember::Property(member) => {
                                    Work::PropertyBegin(member)
                                }
                                _ => unreachable!(),
                            }));
                        }
                        if has_parent && members.is_empty() {
                            output.spans[begin] = parent_span;
                        }
                    }
                    Work::ObjectEnd(properties, parent, begin) => {
                        output.push(Instruction::EndCollection);
                        self.initialize_member_types(&properties, output)?;
                        self.type_scopes.pop();
                        if matches!(output[begin], Instruction::BeginFunctionAmendment { .. }) {
                            let end = output.len();
                            output[begin] = Instruction::BeginFunctionAmendment { end };
                            output.push(Instruction::EndFunctionAmendment);
                        }
                        if let Some(parent) = parent {
                            let constructor = begin
                                + usize::from(matches!(
                                    output[begin],
                                    Instruction::BeginFunctionAmendment { .. }
                                ));
                            output.spans[constructor] = Some(parent);
                            pending_spans.retain(|index| *index != constructor);
                        }
                    }
                    Work::PropertyBegin(member) => {
                        let collection_scope = self.type_scopes.pop().expect("collection scope");
                        self.type_scopes.push(vec![member.clone()]);
                        let previous_bound =
                            std::mem::replace(&mut self.constructor_parameters_bound, false);
                        let mut visit = false;
                        let previous_modifiers =
                            std::mem::replace(&mut self.modifiers, member.modifiers.clone());
                        self.validate_modifiers(member)?;
                        let previous_member = self.member.clone();
                        self.member = if previous_member.is_empty() {
                            format!("#{}", member.name).into()
                        } else {
                            previous_member.concat(&format!(".{}", member.name).into())
                        };
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
                        if let Some(error) = &initialization_error {
                            // Annotation resolution runs when the owner is initialized. Keep a
                            // stub body so compilation cannot bypass earlier type dependencies.
                            instructions.push_at(
                                Instruction::Fail(error.clone()),
                                error.span.unwrap_or(member.span),
                            );
                        } else if matches!(member.expr.kind, crate::syntax::ExprKind::Uninitialized)
                        {
                            self.default_property(member, &mut instructions)?;
                        } else {
                            visit = true;
                        }

                        let parent = std::mem::replace(output, instructions);
                        let pending = std::mem::take(&mut pending_spans);
                        work.push(Work::PropertyEnd(Box::new(PropertyCompilation {
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
                            self.register_property_parents(member, &mut inferred_parents)?;
                            work.push(Work::Visit(&member.expr));
                        }
                        continue;
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
                        let mut instructions = std::mem::replace(output, parent);
                        let property_typecheck = member.ty.as_ref().map(|_| instructions.len());
                        if let Some(ty) = &member.ty {
                            instructions.push_at(
                                Instruction::CheckType(
                                    compiled_type.clone().expect("member annotation"),
                                ),
                                self.type_source_span(ty),
                            );
                        }
                        instructions.push(Instruction::Return);
                        let code = self.program.blocks.len();
                        let header_span = Span::new(
                            member.span.start,
                            member
                                .ty
                                .as_ref()
                                .map_or(member.name_span.end, |ty| ty.span.end),
                        );
                        self.block(instructions, Some(header_span));
                        self.program.metadata[code].hidden = member
                            .modifiers
                            .iter()
                            .any(|m| m.kind == crate::syntax::ModifierKind::Hidden);
                        self.program.metadata[code].value_span =
                            Some(self.property_value_span(member));
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
                                format!(
                                    "{}({})",
                                    member.name,
                                    parameters
                                        .iter()
                                        .map(|p| p.name.as_str())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                ),
                                parameters.len(),
                            ));
                        }
                        self.member = previous_member;
                        self.modifiers = previous_modifiers;
                        let compiled = Member {
                            flags: Member::declaration_flags(member),
                            entry: false,
                            name: Arc::from(member.name.as_str()),
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
                        self.program.objects.push(vec![compiled.clone()]);
                        self.type_scopes.pop();
                        self.type_scopes.push(collection_scope);
                        pending_spans = pending;
                        emitted_start = output.len();
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
                        output.push_at(Instruction::AddCollectionProperty(compiled), span);
                    }
                    Work::Visit(expression) => {
                        active_spans.push(expression.span);
                        work.push(Work::Finish(expression, pending_spans.len()));
                        if let Some(parent) = inferred_parents.remove(&expression.span) {
                            let ExprKind::Object { members, .. } = &expression.kind else {
                                unreachable!()
                            };
                            let span = Span::new(expression.span.start, expression.span.start + 3);
                            let defined = match parent {
                                InferredParent::DeclaredType(check, origin) => self
                                    .default_type_with_origin(
                                        &check,
                                        output,
                                        span,
                                        Some(&origin),
                                    )?,
                                InferredParent::Property => {
                                    output.push_at(Instruction::InferPropertyParent, span);
                                    true
                                }
                            };
                            if defined {
                                if members.iter().all(|member| {
                                    matches!(member, crate::syntax::ObjectMember::Property(_))
                                }) {
                                    work.push(Work::ObjectBody(
                                        members,
                                        crate::heap::CollectionKind::Dynamic,
                                        true,
                                        Some(span),
                                    ));
                                } else {
                                    let start = output.len();
                                    self.collection_body(
                                        members,
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
                                output.push_at(Instruction::Fail(cannot_infer_parent()), span);
                            }
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

    fn schedule_collection<'a>(
        &mut self,
        members: &'a [crate::syntax::ObjectMember],
        kind: crate::heap::CollectionKind,
        has_parent: bool,
        output: &mut Code,
        work: &mut Vec<Work<'a>>,
    ) -> Result<(), RuntimeError> {
        if members
            .iter()
            .all(|member| matches!(member, crate::syntax::ObjectMember::Property(_)))
        {
            work.push(Work::ObjectBody(members, kind, has_parent, None));
            Ok(())
        } else {
            self.collection_body(members, kind, has_parent, output)
        }
    }

    fn schedule_expression<'a>(
        &mut self,
        expression: &'a Expr,
        output: &mut Code,
        work: &mut Vec<Work<'a>>,
    ) -> Result<(), RuntimeError> {
        match &expression.kind {
            ExprKind::Annotation {
                parameters,
                ty,
                members,
            } => {
                self.initialize_type_resolution(ty, output);
                match self.compile_type(ty) {
                    Ok(check) => {
                        if let TypeCheck::Class { .. } = check.unaliased() {
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
                                // GeneratorObjectLiteralNode checks the parameter descriptor
                                // before executing its members. An untyped parameter triggers
                                // an upstream null-location bug rather than a language error.
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
                                    Instruction::Fail(RuntimeError::new(kind, message)),
                                    span,
                                );
                            } else {
                                self.instance_body(members, ty.span, output)?;
                                if !parameters.is_empty() && !members.is_empty() {
                                    output.push_at(Instruction::Fail(RuntimeError::new(ErrorKind::Type, "Expected a function that returns an object, but got an object.")), ty.span);
                                }
                            }
                        } else {
                            output.push_at(
                                Instruction::Fail(RuntimeError::new(
                                    ErrorKind::Type,
                                    "Expected an annotation class.",
                                )),
                                ty.span,
                            );
                        }
                    }
                    Err(error) => {
                        output.push_at(
                            Instruction::Fail(error.clone()),
                            error.span.unwrap_or(ty.span),
                        );
                    }
                }
            }
            ExprKind::New { ty, members } => {
                let check = match self.compile_type(ty) {
                    Ok(check) => check,
                    Err(error) => {
                        self.initialize_type_resolution(ty, output);
                        let span = error.span.unwrap_or(ty.span);
                        output.push_at(Instruction::Fail(error), span);
                        return Ok(());
                    }
                };
                self.initialize_type_resolution(ty, output);
                if matches!(&check,TypeCheck::NamedAlias{declaration,..} if *declaration==amendment::MIXIN_ID)
                {
                    self.default_type(&check, output, ty.span)?;
                    self.schedule_collection(
                        members,
                        crate::heap::CollectionKind::Dynamic,
                        true,
                        output,
                        work,
                    )?;
                    return Ok(());
                }
                if check.collection_base_default() {
                    self.default_type(&check, output, ty.span)?;
                    self.schedule_collection(
                        members,
                        crate::heap::CollectionKind::Dynamic,
                        true,
                        output,
                        work,
                    )?;
                    return Ok(());
                }
                if let TypeCheck::Collection(kind)
                | TypeCheck::ParameterizedCollection { kind, .. } = check.unaliased()
                {
                    let kind = *kind;
                    if matches!(
                        kind,
                        crate::heap::CollectionKind::Map | crate::heap::CollectionKind::Set
                    ) {
                        output.push_at(
                            Instruction::Fail(collections::external_amendment_error(&format!(
                                "{kind:?}"
                            ))),
                            expression.span,
                        );
                        return Ok(());
                    }
                    let has_default =
                        matches!(check.unaliased(), TypeCheck::ParameterizedCollection { .. });
                    if has_default {
                        self.default_type(&check, output, ty.span)?;
                    }
                    if matches!(check.unaliased(), TypeCheck::ParameterizedCollection { .. }) {
                        work.push(Work::Emit(
                            Instruction::CheckType(check),
                            Some(self.type_source_span(ty)),
                        ));
                    }
                    self.schedule_collection(members, kind, has_default, output, work)?;
                } else if check.contains_parameter() {
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
                    output.push_at(
                        Instruction::GenericConstructorParent {
                            ty: check.clone(),
                            source: TypeArgumentSource {
                                span: ty.span,
                                text: Arc::from(
                                    &self.resolved.module.source[ty.span.start..ty.span.end],
                                ),
                            },
                        },
                        expression.span,
                    );
                    // The cast is syntax-directed: `new T` only selects a parent,
                    // whereas explicit `new Alias<T>` also checks the result.
                    if matches!(&ty.kind, crate::syntax::TypeKind::Named { arguments, .. } if !arguments.is_empty())
                    {
                        work.push(Work::Emit(
                            Instruction::CheckType(check),
                            Some(self.type_source_span(ty)),
                        ));
                    }
                    self.schedule_collection(
                        members,
                        crate::heap::CollectionKind::Dynamic,
                        true,
                        output,
                        work,
                    )?;
                } else {
                    self.class_expression(
                        ty,
                        output,
                        self.type_scopes.len(),
                        &mut BTreeSet::new(),
                    )?;
                    self.instance_body(members, ty.span, output)?;
                }
            }
            ExprKind::Object { parent, members }
                if members
                    .iter()
                    .all(|member| matches!(member, crate::syntax::ObjectMember::Property(_))) =>
            {
                work.push(Work::ObjectBody(
                    members,
                    crate::heap::CollectionKind::Dynamic,
                    parent.is_some(),
                    parent.as_ref().map(|parent| parent.unparenthesized().span),
                ));
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
                        operand_span: value.unparenthesized().span,
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
            ExprKind::TypeCheck { value, ty } | ExprKind::TypeCast { value, ty } => {
                if let Some(check) = self.resolve_expression_type(ty, output) {
                    work.push(if matches!(expression.kind, ExprKind::TypeCast { .. }) {
                        Work::Cast(check, self.type_source_span(ty))
                    } else {
                        Work::Emit(Instruction::TestType(check), None)
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
                work.push(Work::Emit(
                    if super_receiver {
                        Instruction::SuperSubscript
                    } else {
                        Instruction::Subscript
                    },
                    None,
                ));
                work.push(Work::Visit(key));
                if !super_receiver {
                    work.push(Work::Visit(receiver));
                }
            }
            ExprKind::Member(receiver, name) => {
                let constant = self.resolved.const_names.contains(&expression.span);
                let super_receiver = matches!(receiver.unparenthesized().kind, ExprKind::Super);
                let name = Arc::from(name.as_str());
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
            ExprKind::NullableMember(receiver, name) => {
                work.push(Work::NullableMember(expression, name));
                work.push(Work::Visit(receiver));
            }
            ExprKind::InterpolatedString(parts) => {
                output.push(Instruction::Constant(Value::string("")));
                for part in parts.iter().rev() {
                    work.push(Work::Emit(Instruction::Add, None));
                    work.push(Work::Emit(
                        Instruction::ToString,
                        Some(
                            self.resolved
                                .module
                                .interpolation_spans
                                .get(&part.span)
                                .copied()
                                .unwrap_or(expression.span),
                        ),
                    ));
                    work.push(Work::Visit(part));
                }
            }
            ExprKind::Call { callee, arguments } => {
                if let ExprKind::Name(name) = &callee.kind
                    && self.resolved.names.get(&callee.span) == Some(&Binding::ImplicitMethod)
                {
                    match name.as_str() {
                        "Map" => {
                            if arguments.len() % 2 != 0 {
                                let span = Span::new(expression.span.end - 1, expression.span.end);
                                let mut error = RuntimeError::new(ErrorKind::Type,
                                    "Number of `Map()` arguments must be a multiple of two: `Map(key1, value1, key2, value2, ..., keyN, valueN)`").at(span);
                                error.frames.push(RuntimeFrame {
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
                        "Set" => {
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
                        "List" => {
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
                .find(|metadata| metadata.member == PklString::from("#value"))
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
                    .position(|metadata| metadata.member == PklString::from("#value"))
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

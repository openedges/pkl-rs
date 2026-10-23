//! Interpolation continuations share the ordinary expression machine and budget.
use super::{
    Diagnostic, Expr, ExprKind, ExpressionFlow, ExpressionMachine, Kind, ObjectBuild, Parser, Span,
    StringPart,
};

struct StringBuild {
    span: Span,
    parts: std::vec::IntoIter<StringPart>,
    expressions: Vec<Expr>,
}

struct InterpolationFrame<'a> {
    parser: Parser<'a>,
    machine: ExpressionMachine,
    string: StringBuild,
    interpolation: Span,
}

impl ExpressionMachine {
    fn new(minimum: u8, initial_object: Option<Box<ObjectBuild>>) -> Self {
        Self {
            frames: Vec::new(),
            minimum,
            current: None,
            object_root: initial_object.is_some(),
            finish_expression: false,
            object_build: initial_object,
        }
    }
}

impl<'a> Parser<'a> {
    pub(super) fn interpolated_expression_machine(
        &mut self,
        minimum: u8,
        initial_object: Option<Box<ObjectBuild>>,
    ) -> Result<Expr, Diagnostic> {
        let mut machine = ExpressionMachine::new(minimum, initial_object);
        let mut frames: Vec<InterpolationFrame<'a>> = Vec::new();
        let mut string: Option<StringBuild> = None;
        let result = (|| {
            'machine: loop {
                if let Some(mut build) = string.take() {
                    for part in build.parts.by_ref() {
                        match part {
                            StringPart::Value(value) => build.expressions.push(Expr {
                                kind: ExprKind::String(value),
                                span: build.span,
                            }),
                            StringPart::Expression(range, interpolation) => {
                                let mut parser = Self::interpolation_with_errors(
                                    self.lexer.source,
                                    range,
                                    self.lexer.string_errors.clone(),
                                )?;
                                parser.depth = self.depth;
                                parser.expressions = self.expressions;
                                parser.member_path.clone_from(&self.member_path);
                                parser.entry_counts.clone_from(&self.entry_counts);
                                parser.expectation = Some(")".into());
                                parser.enter_expression_scope()?;
                                frames.push(InterpolationFrame {
                                    parser: std::mem::replace(self, parser),
                                    machine: std::mem::replace(
                                        &mut machine,
                                        ExpressionMachine::new(0, None),
                                    ),
                                    string: build,
                                    interpolation,
                                });
                                continue 'machine;
                            }
                            _ => unreachable!("string lexer returns rendered parts"),
                        }
                    }
                    self.advance()?;
                    machine.current = Some(Expr {
                        kind: ExprKind::InterpolatedString(build.expressions),
                        span: build.span,
                    });
                }
                match self.expression_machine_step(&mut machine)? {
                    ExpressionFlow::Continue => {}
                    ExpressionFlow::Primary => {
                        if let Kind::StringTemplate(parts) = &self.token.kind {
                            string = Some(StringBuild {
                                span: self.token.span,
                                parts: parts.clone().into_iter(),
                                expressions: Vec::new(),
                            });
                        } else {
                            machine.current = Some(self.primary()?);
                        }
                    }
                    ExpressionFlow::Done => {
                        let expression = machine.current.take().expect("completed expression");
                        if frames.is_empty() {
                            return Ok(expression);
                        }
                        self.expect(')')?;
                        let mut frame = frames.pop().expect("completed interpolation");
                        frame.parser.expressions = self.expressions;
                        frame.parser.entry_counts = std::mem::take(&mut self.entry_counts);
                        frame.parser.pending.append(&mut self.pending);
                        frame
                            .parser
                            .folded_integer_spans
                            .append(&mut self.folded_integer_spans);
                        frame
                            .parser
                            .interpolation_spans
                            .append(&mut self.interpolation_spans);
                        frame
                            .parser
                            .interpolation_spans
                            .insert(expression.span, frame.interpolation);
                        frame.string.expressions.push(expression);
                        *self = frame.parser;
                        machine = frame.machine;
                        string = Some(frame.string);
                    }
                }
            }
        })();
        // Restore the caller's parser on errors as well as success. Each frame
        // owns only flat parser state and Expr trees with iterative destruction.
        if result.is_err() && !frames.is_empty() {
            *self = frames.remove(0).parser;
        }
        result
    }
}

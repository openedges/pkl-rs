//! Shared pkl:math external methods; pinned declarations own argument checks.
use super::*;

#[path = "math_fdlibm.rs"]
mod fdlibm;

#[derive(Clone, Copy, Debug)]
pub enum Operation {
    Exp,
    Sqrt,
    Cbrt,
    Log,
    Log2,
    Log10,
    Sin,
    Cos,
    Tan,
    Asin,
    Acos,
    Atan,
    Atan2,
    Gcd,
    Lcm,
    IsPowerOfTwo,
    Min,
    Max,
}

impl Operation {
    fn named(name: &str) -> Option<Self> {
        Some(match name {
            "exp" => Self::Exp,
            "sqrt" => Self::Sqrt,
            "cbrt" => Self::Cbrt,
            "log" => Self::Log,
            "log2" => Self::Log2,
            "log10" => Self::Log10,
            "sin" => Self::Sin,
            "cos" => Self::Cos,
            "tan" => Self::Tan,
            "asin" => Self::Asin,
            "acos" => Self::Acos,
            "atan" => Self::Atan,
            "atan2" => Self::Atan2,
            "gcd" => Self::Gcd,
            "lcm" => Self::Lcm,
            "isPowerOfTwo" => Self::IsPowerOfTwo,
            "min" => Self::Min,
            "max" => Self::Max,
            _ => return None,
        })
    }

    fn arity(self) -> usize {
        if matches!(
            self,
            Self::Atan2 | Self::Gcd | Self::Lcm | Self::Min | Self::Max
        ) {
            2
        } else {
            1
        }
    }
}

pub(in crate::vm) fn binding(
    program: &mut Program,
    unit: crate::modules::UnitId,
    source: Arc<crate::value::RuntimeSource>,
    declaration: &crate::syntax::Member,
) -> Result<Option<Vec<Instruction>>, RuntimeError> {
    let declaration_name = declaration.name.clone();
    let Some(name) = declaration_name.as_str() else {
        return Ok(None);
    };
    let Some(operation) = Operation::named(name) else {
        return Ok(None);
    };
    let crate::syntax::ExprKind::Function {
        parameters,
        header_span,
        ..
    } = &declaration.expr.kind
    else {
        return Ok(None);
    };
    let arity = operation.arity();
    if parameters.len() != arity {
        return Err(invalid("pinned pkl:math arity changed"));
    }
    let mut code = Vec::new();
    let mut spans = Vec::new();
    for (index, parameter) in parameters.iter().enumerate() {
        code.push(Instruction::LoadLocal(arity - index - 1));
        spans.push(None);
        code.push(Instruction::CheckType(
            if matches!(operation, Operation::Gcd | Operation::Lcm) {
                TypeCheck::Int
            } else {
                TypeCheck::Number
            },
        ));
        spans.push(Some(
            parameter
                .ty
                .as_ref()
                .expect("pinned math parameter type")
                .span,
        ));
    }
    code.push(Instruction::Primitive(super::Operation::Math(operation)));
    spans.push(None);
    code.push(Instruction::Return);
    spans.push(None);
    let block = program.alloc_block(
        code,
        BlockMetadata {
            unit: Some(unit),
            source: Some(source),
            member: format!("#{name}").into(),
            declaration: Some(*header_span),
            spans,
            ..Default::default()
        },
    )?;
    Ok(Some(vec![Instruction::MakeFunction {
        block,
        arity,
        method: true,
        capture_custom_this: false,
    }]))
}

fn number(value: &Value) -> Result<f64, RuntimeError> {
    match value {
        Value::Int(value) => Ok(value.get() as f64),
        Value::Float(value) => Ok(**value),
        _ => Err(invalid("unchecked math Number argument")),
    }
}

fn nonnegative(value: &Value) -> Result<i64, RuntimeError> {
    let Value::Int(value) = value else {
        return Err(invalid("unchecked math Int argument"));
    };
    let value = value.get();
    if value < 0 {
        return Err(RuntimeError::new(
            ErrorKind::Arithmetic,
            format!(
                "Expected a positive number, but got `{}`.",
                crate::vm::strings::diagnostic_integer(value)
            ),
        ));
    }
    Ok(value)
}

fn gcd(mut left: i64, mut right: i64) -> i64 {
    while right != 0 {
        (left, right) = (right, left % right);
    }
    left
}

fn is_power_of_two(value: &Value) -> Result<bool, RuntimeError> {
    match value {
        Value::Int(value) => Ok(value.get() > 0 && (value.get() as u64).is_power_of_two()),
        Value::Float(value) => {
            let value = **value;
            if value <= 0.0 || !value.is_finite() {
                return Ok(false);
            }
            let bits = value.to_bits();
            let significand =
                (bits & 0x000f_ffff_ffff_ffff) | if bits >> 52 == 0 { 0 } else { 1 << 52 };
            Ok(significand.is_power_of_two())
        }
        _ => Err(invalid("unchecked power-of-two argument")),
    }
}

fn extreme(left: Value, right: Value, maximum: bool) -> Result<Value, RuntimeError> {
    if let (Value::Int(left), Value::Int(right)) = (&left, &right) {
        return Ok(Value::Int(
            if maximum {
                left.get().max(right.get())
            } else {
                left.get().min(right.get())
            }
            .into(),
        ));
    }
    let left = number(&left)?;
    let right = number(&right)?;
    let value = if left.is_nan() {
        left
    } else if right.is_nan() {
        right
    } else if left == 0.0 && right == 0.0 {
        // Java min/max distinguish zero signs, including mixed Int/Float calls.
        if left.is_sign_negative() == maximum {
            right
        } else {
            left
        }
    } else if (left > right) == maximum {
        left
    } else {
        right
    };
    Ok(Value::Float(value.into()))
}

impl Vm {
    pub(super) fn math_module_operation(
        &mut self,
        operation: Operation,
    ) -> Result<(), RuntimeError> {
        let second = if operation.arity() == 2 {
            Some(self.pop()?)
        } else {
            None
        };
        let first = self.pop()?;
        let result = match operation {
            Operation::IsPowerOfTwo => Value::Bool(is_power_of_two(&first)?),
            Operation::Min | Operation::Max => extreme(
                first,
                second.expect("binary math argument"),
                matches!(operation, Operation::Max),
            )?,
            Operation::Gcd | Operation::Lcm => {
                // VmUtils.checkPositive checks x before y, including zero cases.
                let left = nonnegative(&first)?;
                let right = nonnegative(&second.expect("binary math argument"))?;
                let value = if matches!(operation, Operation::Gcd) {
                    gcd(left, right)
                } else if left == 0 || right == 0 {
                    0
                } else {
                    crate::numeric::multiply(left / gcd(left, right), right)
                        .map_err(crate::vm::numeric_error)?
                };
                Value::Int(value.into())
            }
            Operation::Atan2 => Value::Float(
                fdlibm::atan2(
                    number(&first)?,
                    number(&second.expect("binary math argument"))?,
                )
                .into(),
            ),
            operation => {
                let value = number(&first)?;
                let result = match operation {
                    Operation::Asin => fdlibm::asin(value),
                    Operation::Acos => fdlibm::acos(value),
                    Operation::Atan => fdlibm::atan(value),
                    Operation::Log10 => fdlibm::log10(value),
                    Operation::Log => fdlibm::log(value),
                    Operation::Log2 => fdlibm::log(value) / fdlibm::log(2.0),
                    operation => crate::numeric::math(
                        match operation {
                            Operation::Exp => "exp",
                            Operation::Sqrt => "sqrt",
                            Operation::Cbrt => "cbrt",
                            Operation::Sin => "sin",
                            Operation::Cos => "cos",
                            Operation::Tan => "tan",
                            _ => return Err(invalid("invalid unary math operation")),
                        },
                        value,
                    )
                    .expect("existing shared numeric kernel"),
                };
                Value::Float(result.into())
            }
        };
        self.stack.push(result);
        Ok(())
    }
}

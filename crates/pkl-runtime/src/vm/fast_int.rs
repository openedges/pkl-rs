//! General pure-`i64` compact bytecode accelerator for hot integer functions and lambdas.
//!
//! Lazily compiles eligible `Vec<Instruction>` blocks into 8-byte `FastIntOp` sequences
//! and executes them on a flat `i64` scratchpad without touching `Heap`, `Box<Frame>`,
//! `Value`, or `BTreeMap` during execution. Any overflow, recursion-depth limit, step-budget
//! exhaustion, non-integer input, active type tracker, deprecation, or open/abstract method
//! aborts cleanly with zero side effects and falls back to the exact `Vm` interpreter.

use super::*;

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub(super) enum FastIntOp {
    LoadLocal(u16),
    StoreLocal(u16),
    BindNewLocal,
    PopLocal,
    ConstSmall(i32),
    ConstPool(u16),
    Add,
    Sub,
    Mul,
    AddConst(i32),
    SubConst(i32),
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    LtConst(i32),
    LeConst(i32),
    GtConst(i32),
    GeConst(i32),
    Jump(u32),
    JumpIfFalse(u32),
    Pop,
    Dup,
    Swap,
    CallSelf(u8),
    Return,
}

#[derive(Clone, Debug)]
pub(super) struct SelfCallSite {
    pub lookup_pc: usize,
    pub name: PklString,
    pub owner_depth: usize,
    pub local: bool,
    pub arity: usize,
}

#[derive(Clone, Copy, Debug)]
enum FastExpr {
    Arg(u8),
    Const(i64),
    Add(u16, u16),
    Sub(u16, u16),
    Mul(u16, u16),
    AddConst(u16, i32),
    SubConst(u16, i32),
    ArgAddConst(u8, i32),
    ArgSubConst(u8, i32),
    Lt(u16, u16),
    Le(u16, u16),
    Gt(u16, u16),
    Ge(u16, u16),
    Eq(u16, u16),
    Ne(u16, u16),
    LtConst(u16, i32),
    LeConst(u16, i32),
    GtConst(u16, i32),
    GeConst(u16, i32),
    ArgLtConst(u8, i32),
    ArgLeConst(u8, i32),
    ArgGtConst(u8, i32),
    ArgGeConst(u8, i32),
    If {
        cond: u16,
        then_id: u16,
        else_id: u16,
    },
    IfArgLtConst {
        arg: u8,
        k: i32,
        then_id: u16,
        else_id: u16,
    },
    IfArgLeConst {
        arg: u8,
        k: i32,
        then_id: u16,
        else_id: u16,
    },
    CallSelf1(u16),
    CallSelf1ArgSub(i32),
    CallSelf2(u16, u16),
    CallSelf3(u16, u16, u16),
    CallSelf4(u16, u16, u16, u16),
}

#[derive(Clone, Debug)]
struct FastIntTree {
    nodes: Box<[FastExpr]>,
    root: u16,
    arity: u8,
}

impl FastIntTree {
    fn lower(ops: &[FastIntOp], consts: &[i64], arity: usize) -> Option<Self> {
        if arity == 0 || arity > 4 {
            return None;
        }
        // Only acyclic expression bodies reading direct arguments (no local writes/captures).
        for op in ops {
            match op {
                FastIntOp::BindNewLocal | FastIntOp::StoreLocal(_) | FastIntOp::PopLocal => {
                    return None;
                }
                FastIntOp::LoadLocal(d) if (*d as usize) >= arity => return None,
                FastIntOp::CallSelf(a) if (*a as usize) != arity => return None,
                _ => {}
            }
        }
        let mut nodes = Vec::with_capacity(16);
        let root = Self::lower_at(ops, consts, arity, 0, Vec::new(), &mut nodes, 0)?;
        Some(Self {
            nodes: nodes.into_boxed_slice(),
            root,
            arity: arity as u8,
        })
    }

    fn push_node(nodes: &mut Vec<FastExpr>, expr: FastExpr) -> Option<u16> {
        if nodes.len() >= 128 {
            return None;
        }
        let id = nodes.len() as u16;
        nodes.push(expr);
        Some(id)
    }

    fn lower_at(
        ops: &[FastIntOp],
        consts: &[i64],
        arity: usize,
        mut pc: usize,
        mut stack: Vec<u16>,
        nodes: &mut Vec<FastExpr>,
        branch_depth: u8,
    ) -> Option<u16> {
        if branch_depth > 16 {
            return None;
        }
        loop {
            let op = *ops.get(pc)?;
            match op {
                FastIntOp::LoadLocal(d) => {
                    let arg = (arity - 1 - (d as usize)) as u8;
                    let id = Self::push_node(nodes, FastExpr::Arg(arg))?;
                    stack.push(id);
                    pc += 1;
                }
                FastIntOp::ConstSmall(k) => {
                    let id = Self::push_node(nodes, FastExpr::Const(i64::from(k)))?;
                    stack.push(id);
                    pc += 1;
                }
                FastIntOp::ConstPool(idx) => {
                    let id = Self::push_node(nodes, FastExpr::Const(*consts.get(idx as usize)?))?;
                    stack.push(id);
                    pc += 1;
                }
                FastIntOp::Add => {
                    let r = stack.pop()?;
                    let l = stack.pop()?;
                    let id = Self::push_node(nodes, FastExpr::Add(l, r))?;
                    stack.push(id);
                    pc += 1;
                }
                FastIntOp::Sub => {
                    let r = stack.pop()?;
                    let l = stack.pop()?;
                    let id = Self::push_node(nodes, FastExpr::Sub(l, r))?;
                    stack.push(id);
                    pc += 1;
                }
                FastIntOp::Mul => {
                    let r = stack.pop()?;
                    let l = stack.pop()?;
                    let id = Self::push_node(nodes, FastExpr::Mul(l, r))?;
                    stack.push(id);
                    pc += 1;
                }
                FastIntOp::AddConst(k) => {
                    let x = stack.pop()?;
                    let expr = match nodes[x as usize] {
                        FastExpr::Arg(a) => FastExpr::ArgAddConst(a, k),
                        _ => FastExpr::AddConst(x, k),
                    };
                    let id = Self::push_node(nodes, expr)?;
                    stack.push(id);
                    pc += 1;
                }
                FastIntOp::SubConst(k) => {
                    let x = stack.pop()?;
                    let expr = match nodes[x as usize] {
                        FastExpr::Arg(a) => FastExpr::ArgSubConst(a, k),
                        _ => FastExpr::SubConst(x, k),
                    };
                    let id = Self::push_node(nodes, expr)?;
                    stack.push(id);
                    pc += 1;
                }
                FastIntOp::Lt => {
                    let r = stack.pop()?;
                    let l = stack.pop()?;
                    let id = Self::push_node(nodes, FastExpr::Lt(l, r))?;
                    stack.push(id);
                    pc += 1;
                }
                FastIntOp::Le => {
                    let r = stack.pop()?;
                    let l = stack.pop()?;
                    let id = Self::push_node(nodes, FastExpr::Le(l, r))?;
                    stack.push(id);
                    pc += 1;
                }
                FastIntOp::Gt => {
                    let r = stack.pop()?;
                    let l = stack.pop()?;
                    let id = Self::push_node(nodes, FastExpr::Gt(l, r))?;
                    stack.push(id);
                    pc += 1;
                }
                FastIntOp::Ge => {
                    let r = stack.pop()?;
                    let l = stack.pop()?;
                    let id = Self::push_node(nodes, FastExpr::Ge(l, r))?;
                    stack.push(id);
                    pc += 1;
                }
                FastIntOp::Eq => {
                    let r = stack.pop()?;
                    let l = stack.pop()?;
                    let id = Self::push_node(nodes, FastExpr::Eq(l, r))?;
                    stack.push(id);
                    pc += 1;
                }
                FastIntOp::Ne => {
                    let r = stack.pop()?;
                    let l = stack.pop()?;
                    let id = Self::push_node(nodes, FastExpr::Ne(l, r))?;
                    stack.push(id);
                    pc += 1;
                }
                FastIntOp::LtConst(k) => {
                    let x = stack.pop()?;
                    let expr = match nodes[x as usize] {
                        FastExpr::Arg(a) => FastExpr::ArgLtConst(a, k),
                        _ => FastExpr::LtConst(x, k),
                    };
                    let id = Self::push_node(nodes, expr)?;
                    stack.push(id);
                    pc += 1;
                }
                FastIntOp::LeConst(k) => {
                    let x = stack.pop()?;
                    let expr = match nodes[x as usize] {
                        FastExpr::Arg(a) => FastExpr::ArgLeConst(a, k),
                        _ => FastExpr::LeConst(x, k),
                    };
                    let id = Self::push_node(nodes, expr)?;
                    stack.push(id);
                    pc += 1;
                }
                FastIntOp::GtConst(k) => {
                    let x = stack.pop()?;
                    let expr = match nodes[x as usize] {
                        FastExpr::Arg(a) => FastExpr::ArgGtConst(a, k),
                        _ => FastExpr::GtConst(x, k),
                    };
                    let id = Self::push_node(nodes, expr)?;
                    stack.push(id);
                    pc += 1;
                }
                FastIntOp::GeConst(k) => {
                    let x = stack.pop()?;
                    let expr = match nodes[x as usize] {
                        FastExpr::Arg(a) => FastExpr::ArgGeConst(a, k),
                        _ => FastExpr::GeConst(x, k),
                    };
                    let id = Self::push_node(nodes, expr)?;
                    stack.push(id);
                    pc += 1;
                }
                FastIntOp::Dup => {
                    let v = *stack.last()?;
                    stack.push(v);
                    pc += 1;
                }
                FastIntOp::Pop => {
                    stack.pop()?;
                    pc += 1;
                }
                FastIntOp::Swap => {
                    let n = stack.len();
                    if n < 2 {
                        return None;
                    }
                    stack.swap(n - 1, n - 2);
                    pc += 1;
                }
                FastIntOp::CallSelf(a) => {
                    let expr = match a {
                        1 => {
                            let a0 = stack.pop()?;
                            match nodes[a0 as usize] {
                                FastExpr::ArgSubConst(0, k) => FastExpr::CallSelf1ArgSub(k),
                                _ => FastExpr::CallSelf1(a0),
                            }
                        }
                        2 => {
                            let a1 = stack.pop()?;
                            let a0 = stack.pop()?;
                            FastExpr::CallSelf2(a0, a1)
                        }
                        3 => {
                            let a2 = stack.pop()?;
                            let a1 = stack.pop()?;
                            let a0 = stack.pop()?;
                            FastExpr::CallSelf3(a0, a1, a2)
                        }
                        4 => {
                            let a3 = stack.pop()?;
                            let a2 = stack.pop()?;
                            let a1 = stack.pop()?;
                            let a0 = stack.pop()?;
                            FastExpr::CallSelf4(a0, a1, a2, a3)
                        }
                        _ => return None,
                    };
                    let id = Self::push_node(nodes, expr)?;
                    stack.push(id);
                    pc += 1;
                }
                FastIntOp::Jump(target) => {
                    let target = target as usize;
                    if target <= pc {
                        return None;
                    }
                    pc = target;
                }
                FastIntOp::JumpIfFalse(target) => {
                    let target = target as usize;
                    if target <= pc {
                        return None;
                    }
                    let cond = stack.pop()?;
                    let then_id = Self::lower_at(
                        ops,
                        consts,
                        arity,
                        pc + 1,
                        stack.clone(),
                        nodes,
                        branch_depth + 1,
                    )?;
                    let else_id =
                        Self::lower_at(ops, consts, arity, target, stack, nodes, branch_depth + 1)?;
                    let expr = match nodes[cond as usize] {
                        FastExpr::ArgLtConst(arg, k) => FastExpr::IfArgLtConst {
                            arg,
                            k,
                            then_id,
                            else_id,
                        },
                        FastExpr::ArgLeConst(arg, k) => FastExpr::IfArgLeConst {
                            arg,
                            k,
                            then_id,
                            else_id,
                        },
                        _ => FastExpr::If {
                            cond,
                            then_id,
                            else_id,
                        },
                    };
                    return Self::push_node(nodes, expr);
                }
                FastIntOp::Return => {
                    if stack.len() != 1 {
                        return None;
                    }
                    return stack.pop();
                }
                _ => return None,
            }
        }
    }

    #[inline]
    fn execute(&self, args: &[i64], max_steps: usize) -> TreeResult {
        if max_steps == 0 {
            return TreeResult::Abort(0);
        }
        let mut budget = max_steps;
        let res = if self.arity == 1 {
            eval_tree_1(&self.nodes, self.root, self.root, args[0], 0, &mut budget)
        } else {
            let mut a = [0i64; 4];
            a[..args.len()].copy_from_slice(args);
            eval_tree_n(&self.nodes, self.root, self.root, &a, 0, &mut budget)
        };
        let consumed = max_steps.saturating_sub(budget).max(1);
        match res {
            TreeEval::Ok(val) => TreeResult::Done(val, consumed),
            TreeEval::DepthExceeded => TreeResult::DepthExceeded(consumed),
            TreeEval::Abort => TreeResult::Abort(consumed),
        }
    }
}

#[cfg(target_arch = "wasm32")]
const MAX_FAST_TREE_DEPTH: u16 = 128;
#[cfg(not(target_arch = "wasm32"))]
const MAX_FAST_TREE_DEPTH: u16 = 256;

#[derive(Clone, Copy, Debug)]
enum TreeEval {
    Ok(i64),
    DepthExceeded,
    Abort,
}

#[derive(Clone, Copy, Debug)]
enum TreeResult {
    Done(i64, usize),
    DepthExceeded(usize),
    Abort(usize),
}

macro_rules! try_tree {
    ($expr:expr) => {
        match $expr {
            TreeEval::Ok(v) => v,
            TreeEval::DepthExceeded => return TreeEval::DepthExceeded,
            TreeEval::Abort => return TreeEval::Abort,
        }
    };
}

#[inline]
fn eval_tree_1(
    nodes: &[FastExpr],
    root: u16,
    mut id: u16,
    arg0: i64,
    depth: u16,
    budget: &mut usize,
) -> TreeEval {
    if depth >= MAX_FAST_TREE_DEPTH {
        return TreeEval::DepthExceeded;
    }
    let next_depth = depth + 1;
    loop {
        match nodes[id as usize] {
            FastExpr::Arg(_) => return TreeEval::Ok(arg0),
            FastExpr::Const(k) => return TreeEval::Ok(k),
            FastExpr::ArgAddConst(_, k) => {
                return match arg0.checked_add(i64::from(k)) {
                    Some(v) => TreeEval::Ok(v),
                    None => TreeEval::Abort,
                };
            }
            FastExpr::ArgSubConst(_, k) => {
                return match arg0.checked_sub(i64::from(k)) {
                    Some(v) => TreeEval::Ok(v),
                    None => TreeEval::Abort,
                };
            }
            FastExpr::ArgLtConst(_, k) => return TreeEval::Ok(i64::from(arg0 < i64::from(k))),
            FastExpr::ArgLeConst(_, k) => return TreeEval::Ok(i64::from(arg0 <= i64::from(k))),
            FastExpr::ArgGtConst(_, k) => return TreeEval::Ok(i64::from(arg0 > i64::from(k))),
            FastExpr::ArgGeConst(_, k) => return TreeEval::Ok(i64::from(arg0 >= i64::from(k))),
            FastExpr::IfArgLtConst {
                k,
                then_id,
                else_id,
                ..
            } => {
                id = if arg0 < i64::from(k) {
                    then_id
                } else {
                    else_id
                };
            }
            FastExpr::IfArgLeConst {
                k,
                then_id,
                else_id,
                ..
            } => {
                id = if arg0 <= i64::from(k) {
                    then_id
                } else {
                    else_id
                };
            }
            FastExpr::If {
                cond,
                then_id,
                else_id,
            } => {
                let c = try_tree!(eval_tree_1(nodes, root, cond, arg0, next_depth, budget));
                id = if c != 0 { then_id } else { else_id };
            }
            FastExpr::Add(l, r) => {
                let lv = try_tree!(eval_tree_1(nodes, root, l, arg0, next_depth, budget));
                let rv = try_tree!(eval_tree_1(nodes, root, r, arg0, next_depth, budget));
                return match lv.checked_add(rv) {
                    Some(v) => TreeEval::Ok(v),
                    None => TreeEval::Abort,
                };
            }
            FastExpr::Sub(l, r) => {
                let lv = try_tree!(eval_tree_1(nodes, root, l, arg0, next_depth, budget));
                let rv = try_tree!(eval_tree_1(nodes, root, r, arg0, next_depth, budget));
                return match lv.checked_sub(rv) {
                    Some(v) => TreeEval::Ok(v),
                    None => TreeEval::Abort,
                };
            }
            FastExpr::Mul(l, r) => {
                let lv = try_tree!(eval_tree_1(nodes, root, l, arg0, next_depth, budget));
                let rv = try_tree!(eval_tree_1(nodes, root, r, arg0, next_depth, budget));
                return match lv.checked_mul(rv) {
                    Some(v) => TreeEval::Ok(v),
                    None => TreeEval::Abort,
                };
            }
            FastExpr::AddConst(x, k) => {
                let v = try_tree!(eval_tree_1(nodes, root, x, arg0, next_depth, budget));
                return match v.checked_add(i64::from(k)) {
                    Some(r) => TreeEval::Ok(r),
                    None => TreeEval::Abort,
                };
            }
            FastExpr::SubConst(x, k) => {
                let v = try_tree!(eval_tree_1(nodes, root, x, arg0, next_depth, budget));
                return match v.checked_sub(i64::from(k)) {
                    Some(r) => TreeEval::Ok(r),
                    None => TreeEval::Abort,
                };
            }
            FastExpr::Lt(l, r) => {
                let lv = try_tree!(eval_tree_1(nodes, root, l, arg0, next_depth, budget));
                let rv = try_tree!(eval_tree_1(nodes, root, r, arg0, next_depth, budget));
                return TreeEval::Ok(i64::from(lv < rv));
            }
            FastExpr::Le(l, r) => {
                let lv = try_tree!(eval_tree_1(nodes, root, l, arg0, next_depth, budget));
                let rv = try_tree!(eval_tree_1(nodes, root, r, arg0, next_depth, budget));
                return TreeEval::Ok(i64::from(lv <= rv));
            }
            FastExpr::Gt(l, r) => {
                let lv = try_tree!(eval_tree_1(nodes, root, l, arg0, next_depth, budget));
                let rv = try_tree!(eval_tree_1(nodes, root, r, arg0, next_depth, budget));
                return TreeEval::Ok(i64::from(lv > rv));
            }
            FastExpr::Ge(l, r) => {
                let lv = try_tree!(eval_tree_1(nodes, root, l, arg0, next_depth, budget));
                let rv = try_tree!(eval_tree_1(nodes, root, r, arg0, next_depth, budget));
                return TreeEval::Ok(i64::from(lv >= rv));
            }
            FastExpr::Eq(l, r) => {
                let lv = try_tree!(eval_tree_1(nodes, root, l, arg0, next_depth, budget));
                let rv = try_tree!(eval_tree_1(nodes, root, r, arg0, next_depth, budget));
                return TreeEval::Ok(i64::from(lv == rv));
            }
            FastExpr::Ne(l, r) => {
                let lv = try_tree!(eval_tree_1(nodes, root, l, arg0, next_depth, budget));
                let rv = try_tree!(eval_tree_1(nodes, root, r, arg0, next_depth, budget));
                return TreeEval::Ok(i64::from(lv != rv));
            }
            FastExpr::LtConst(x, k) => {
                let v = try_tree!(eval_tree_1(nodes, root, x, arg0, next_depth, budget));
                return TreeEval::Ok(i64::from(v < i64::from(k)));
            }
            FastExpr::LeConst(x, k) => {
                let v = try_tree!(eval_tree_1(nodes, root, x, arg0, next_depth, budget));
                return TreeEval::Ok(i64::from(v <= i64::from(k)));
            }
            FastExpr::GtConst(x, k) => {
                let v = try_tree!(eval_tree_1(nodes, root, x, arg0, next_depth, budget));
                return TreeEval::Ok(i64::from(v > i64::from(k)));
            }
            FastExpr::GeConst(x, k) => {
                let v = try_tree!(eval_tree_1(nodes, root, x, arg0, next_depth, budget));
                return TreeEval::Ok(i64::from(v >= i64::from(k)));
            }
            FastExpr::CallSelf1ArgSub(k) => {
                if *budget < 4 {
                    return TreeEval::Abort;
                }
                *budget -= 4;
                let Some(next_arg) = arg0.checked_sub(i64::from(k)) else {
                    return TreeEval::Abort;
                };
                return eval_tree_1(nodes, root, root, next_arg, next_depth, budget);
            }
            FastExpr::CallSelf1(a) => {
                if *budget < 4 {
                    return TreeEval::Abort;
                }
                *budget -= 4;
                let next_arg = try_tree!(eval_tree_1(nodes, root, a, arg0, next_depth, budget));
                return eval_tree_1(nodes, root, root, next_arg, next_depth, budget);
            }
            _ => return TreeEval::Abort,
        }
    }
}

#[inline]
fn eval_tree_n(
    nodes: &[FastExpr],
    root: u16,
    mut id: u16,
    args: &[i64; 4],
    depth: u16,
    budget: &mut usize,
) -> TreeEval {
    if depth >= MAX_FAST_TREE_DEPTH {
        return TreeEval::DepthExceeded;
    }
    let next_depth = depth + 1;
    loop {
        match nodes[id as usize] {
            FastExpr::Arg(a) => return TreeEval::Ok(args[a as usize]),
            FastExpr::Const(k) => return TreeEval::Ok(k),
            FastExpr::ArgAddConst(a, k) => {
                return match args[a as usize].checked_add(i64::from(k)) {
                    Some(v) => TreeEval::Ok(v),
                    None => TreeEval::Abort,
                };
            }
            FastExpr::ArgSubConst(a, k) => {
                return match args[a as usize].checked_sub(i64::from(k)) {
                    Some(v) => TreeEval::Ok(v),
                    None => TreeEval::Abort,
                };
            }
            FastExpr::ArgLtConst(a, k) => {
                return TreeEval::Ok(i64::from(args[a as usize] < i64::from(k)));
            }
            FastExpr::ArgLeConst(a, k) => {
                return TreeEval::Ok(i64::from(args[a as usize] <= i64::from(k)));
            }
            FastExpr::ArgGtConst(a, k) => {
                return TreeEval::Ok(i64::from(args[a as usize] > i64::from(k)));
            }
            FastExpr::ArgGeConst(a, k) => {
                return TreeEval::Ok(i64::from(args[a as usize] >= i64::from(k)));
            }
            FastExpr::IfArgLtConst {
                arg,
                k,
                then_id,
                else_id,
            } => {
                id = if args[arg as usize] < i64::from(k) {
                    then_id
                } else {
                    else_id
                };
            }
            FastExpr::IfArgLeConst {
                arg,
                k,
                then_id,
                else_id,
            } => {
                id = if args[arg as usize] <= i64::from(k) {
                    then_id
                } else {
                    else_id
                };
            }
            FastExpr::If {
                cond,
                then_id,
                else_id,
            } => {
                let c = try_tree!(eval_tree_n(nodes, root, cond, args, next_depth, budget));
                id = if c != 0 { then_id } else { else_id };
            }
            FastExpr::Add(l, r) => {
                let lv = try_tree!(eval_tree_n(nodes, root, l, args, next_depth, budget));
                let rv = try_tree!(eval_tree_n(nodes, root, r, args, next_depth, budget));
                return match lv.checked_add(rv) {
                    Some(v) => TreeEval::Ok(v),
                    None => TreeEval::Abort,
                };
            }
            FastExpr::Sub(l, r) => {
                let lv = try_tree!(eval_tree_n(nodes, root, l, args, next_depth, budget));
                let rv = try_tree!(eval_tree_n(nodes, root, r, args, next_depth, budget));
                return match lv.checked_sub(rv) {
                    Some(v) => TreeEval::Ok(v),
                    None => TreeEval::Abort,
                };
            }
            FastExpr::Mul(l, r) => {
                let lv = try_tree!(eval_tree_n(nodes, root, l, args, next_depth, budget));
                let rv = try_tree!(eval_tree_n(nodes, root, r, args, next_depth, budget));
                return match lv.checked_mul(rv) {
                    Some(v) => TreeEval::Ok(v),
                    None => TreeEval::Abort,
                };
            }
            FastExpr::AddConst(x, k) => {
                let v = try_tree!(eval_tree_n(nodes, root, x, args, next_depth, budget));
                return match v.checked_add(i64::from(k)) {
                    Some(r) => TreeEval::Ok(r),
                    None => TreeEval::Abort,
                };
            }
            FastExpr::SubConst(x, k) => {
                let v = try_tree!(eval_tree_n(nodes, root, x, args, next_depth, budget));
                return match v.checked_sub(i64::from(k)) {
                    Some(r) => TreeEval::Ok(r),
                    None => TreeEval::Abort,
                };
            }
            FastExpr::Lt(l, r) => {
                let lv = try_tree!(eval_tree_n(nodes, root, l, args, next_depth, budget));
                let rv = try_tree!(eval_tree_n(nodes, root, r, args, next_depth, budget));
                return TreeEval::Ok(i64::from(lv < rv));
            }
            FastExpr::Le(l, r) => {
                let lv = try_tree!(eval_tree_n(nodes, root, l, args, next_depth, budget));
                let rv = try_tree!(eval_tree_n(nodes, root, r, args, next_depth, budget));
                return TreeEval::Ok(i64::from(lv <= rv));
            }
            FastExpr::Gt(l, r) => {
                let lv = try_tree!(eval_tree_n(nodes, root, l, args, next_depth, budget));
                let rv = try_tree!(eval_tree_n(nodes, root, r, args, next_depth, budget));
                return TreeEval::Ok(i64::from(lv > rv));
            }
            FastExpr::Ge(l, r) => {
                let lv = try_tree!(eval_tree_n(nodes, root, l, args, next_depth, budget));
                let rv = try_tree!(eval_tree_n(nodes, root, r, args, next_depth, budget));
                return TreeEval::Ok(i64::from(lv >= rv));
            }
            FastExpr::Eq(l, r) => {
                let lv = try_tree!(eval_tree_n(nodes, root, l, args, next_depth, budget));
                let rv = try_tree!(eval_tree_n(nodes, root, r, args, next_depth, budget));
                return TreeEval::Ok(i64::from(lv == rv));
            }
            FastExpr::Ne(l, r) => {
                let lv = try_tree!(eval_tree_n(nodes, root, l, args, next_depth, budget));
                let rv = try_tree!(eval_tree_n(nodes, root, r, args, next_depth, budget));
                return TreeEval::Ok(i64::from(lv != rv));
            }
            FastExpr::LtConst(x, k) => {
                let v = try_tree!(eval_tree_n(nodes, root, x, args, next_depth, budget));
                return TreeEval::Ok(i64::from(v < i64::from(k)));
            }
            FastExpr::LeConst(x, k) => {
                let v = try_tree!(eval_tree_n(nodes, root, x, args, next_depth, budget));
                return TreeEval::Ok(i64::from(v <= i64::from(k)));
            }
            FastExpr::GtConst(x, k) => {
                let v = try_tree!(eval_tree_n(nodes, root, x, args, next_depth, budget));
                return TreeEval::Ok(i64::from(v > i64::from(k)));
            }
            FastExpr::GeConst(x, k) => {
                let v = try_tree!(eval_tree_n(nodes, root, x, args, next_depth, budget));
                return TreeEval::Ok(i64::from(v >= i64::from(k)));
            }
            FastExpr::CallSelf2(a0, a1) => {
                if *budget < 4 {
                    return TreeEval::Abort;
                }
                *budget -= 4;
                let next = [
                    try_tree!(eval_tree_n(nodes, root, a0, args, next_depth, budget)),
                    try_tree!(eval_tree_n(nodes, root, a1, args, next_depth, budget)),
                    0,
                    0,
                ];
                return eval_tree_n(nodes, root, root, &next, next_depth, budget);
            }
            FastExpr::CallSelf3(a0, a1, a2) => {
                if *budget < 4 {
                    return TreeEval::Abort;
                }
                *budget -= 4;
                let next = [
                    try_tree!(eval_tree_n(nodes, root, a0, args, next_depth, budget)),
                    try_tree!(eval_tree_n(nodes, root, a1, args, next_depth, budget)),
                    try_tree!(eval_tree_n(nodes, root, a2, args, next_depth, budget)),
                    0,
                ];
                return eval_tree_n(nodes, root, root, &next, next_depth, budget);
            }
            FastExpr::CallSelf4(a0, a1, a2, a3) => {
                if *budget < 4 {
                    return TreeEval::Abort;
                }
                *budget -= 4;
                let next = [
                    try_tree!(eval_tree_n(nodes, root, a0, args, next_depth, budget)),
                    try_tree!(eval_tree_n(nodes, root, a1, args, next_depth, budget)),
                    try_tree!(eval_tree_n(nodes, root, a2, args, next_depth, budget)),
                    try_tree!(eval_tree_n(nodes, root, a3, args, next_depth, budget)),
                ];
                return eval_tree_n(nodes, root, root, &next, next_depth, budget);
            }
            _ => return TreeEval::Abort,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct FastIntBlock {
    pub ops: Vec<FastIntOp>,
    pub consts: Vec<i64>,
    pub bind_sites: Vec<usize>,
    pub self_calls: Vec<SelfCallSite>,
    pub has_self_calls: bool,
    pub returns_int: bool,
    /// Minimum function arity for which this block reads zero captured locals.
    pub min_arity_no_capture: usize,
    tree: Option<FastIntTree>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ValKind {
    Int,
    Bool,
    SelfFn(usize),
}

#[inline]
pub(super) fn is_int_accepting_type(ty: &TypeCheck) -> bool {
    match ty.resolved() {
        TypeCheck::Any
        | TypeCheck::Unknown
        | TypeCheck::Int
        | TypeCheck::Number
        | TypeCheck::NonNull => true,
        TypeCheck::Alias(inner) | TypeCheck::GenericAlias(inner) => is_int_accepting_type(inner),
        TypeCheck::NamedAlias {
            arguments,
            expanded: inner,
            ..
        } if arguments.is_empty() => is_int_accepting_type(inner),
        TypeCheck::Nullable(inner) => is_int_accepting_type(inner),
        _ => false,
    }
}

fn is_bool_accepting_type(ty: &TypeCheck) -> bool {
    match ty.resolved() {
        TypeCheck::Any | TypeCheck::Unknown | TypeCheck::Boolean | TypeCheck::NonNull => true,
        TypeCheck::Alias(inner) | TypeCheck::GenericAlias(inner) => is_bool_accepting_type(inner),
        TypeCheck::NamedAlias {
            arguments,
            expanded: inner,
            ..
        } if arguments.is_empty() => is_bool_accepting_type(inner),
        TypeCheck::Nullable(inner) => is_bool_accepting_type(inner),
        _ => false,
    }
}

impl FastIntBlock {
    /// Attempts to compile `code` into a pure-`i64` `FastIntBlock`.
    /// Returns `None` if any instruction, control-flow join, or type falls outside the pure-`i64` subset.
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn compile(code: &[Instruction], arity: usize) -> Option<Self> {
        if code.is_empty() || code.len() > 4096 {
            return None;
        }
        let mut is_jump_target = vec![false; code.len() + 1];
        for instr in code {
            match instr {
                Instruction::Jump(t) | Instruction::JumpIfFalse(t) => {
                    if *t >= code.len() {
                        return None;
                    }
                    is_jump_target[*t] = true;
                }
                _ => {}
            }
        }

        // First pass: abstractly type-check reachable PCs so every jump join agrees on ValKind stack.
        let mut entry_stacks: Vec<Option<(Vec<ValKind>, usize)>> = vec![None; code.len()];
        let mut worklist: Vec<usize> = vec![0];
        entry_stacks[0] = Some((Vec::new(), 0));
        let mut returns_int: Option<bool> = None;
        let mut min_arity_no_capture: usize = 0;

        while let Some(mut pc) = worklist.pop() {
            let (mut st, mut pushed) = entry_stacks[pc].clone().unwrap();
            loop {
                let instr = code.get(pc)?;
                let mut next_pc = Some(pc + 1);
                match instr {
                    Instruction::Observe(_) | Instruction::ObserveNativeLiteral { .. } => {}
                    Instruction::LoadLocal(d) => {
                        if *d >= pushed {
                            min_arity_no_capture = min_arity_no_capture.max(*d - pushed + 1);
                        }
                        st.push(ValKind::Int);
                    }
                    Instruction::StoreLocal(d) => {
                        if *d >= pushed {
                            min_arity_no_capture = min_arity_no_capture.max(*d - pushed + 1);
                        }
                        if st.pop() != Some(ValKind::Int) {
                            return None;
                        }
                    }
                    Instruction::BindLocal { depth, ty } => {
                        if st.pop() != Some(ValKind::Int) || !is_int_accepting_type(ty) {
                            return None;
                        }
                        match depth {
                            Some(d) => {
                                if *d >= pushed {
                                    min_arity_no_capture =
                                        min_arity_no_capture.max(*d - pushed + 1);
                                }
                            }
                            None => pushed += 1,
                        }
                    }
                    Instruction::PopLocal => {
                        pushed = pushed.checked_sub(1)?;
                    }
                    Instruction::Constant(Value::Int(_)) => st.push(ValKind::Int),
                    Instruction::Constant(Value::Bool(_)) => st.push(ValKind::Bool),
                    Instruction::Pop => {
                        let popped = st.pop()?;
                        if matches!(popped, ValKind::SelfFn(_)) {
                            return None;
                        }
                    }
                    Instruction::Duplicate => {
                        let top = *st.last()?;
                        if matches!(top, ValKind::SelfFn(_)) {
                            return None;
                        }
                        st.push(top);
                    }
                    Instruction::Swap => {
                        let n = st.len();
                        if n < 2
                            || matches!(st[n - 1], ValKind::SelfFn(_))
                            || matches!(st[n - 2], ValKind::SelfFn(_))
                        {
                            return None;
                        }
                        st.swap(n - 1, n - 2);
                    }
                    Instruction::AddIntConstant(_) => {
                        if st.last() != Some(&ValKind::Int) {
                            return None;
                        }
                    }
                    Instruction::Add | Instruction::Subtract | Instruction::Multiply => {
                        if st.pop() != Some(ValKind::Int) || st.pop() != Some(ValKind::Int) {
                            return None;
                        }
                        st.push(ValKind::Int);
                    }
                    Instruction::Less
                    | Instruction::LessEqual
                    | Instruction::Greater
                    | Instruction::GreaterEqual
                    | Instruction::Equal
                    | Instruction::NotEqual => {
                        if st.pop() != Some(ValKind::Int) || st.pop() != Some(ValKind::Int) {
                            return None;
                        }
                        st.push(ValKind::Bool);
                    }
                    Instruction::CheckType(ty) => match st.last()? {
                        ValKind::Int if is_int_accepting_type(ty) => {}
                        ValKind::Bool if is_bool_accepting_type(ty) => {}
                        _ => return None,
                    },
                    Instruction::GetMethodProperty { .. } => {
                        st.push(ValKind::SelfFn(pc));
                    }
                    Instruction::PrepareMethodCall {
                        lookup,
                        arity,
                        after_arguments,
                    } => {
                        let idx = if *after_arguments {
                            st.len().checked_sub(*arity + 1)?
                        } else {
                            st.len().checked_sub(1)?
                        };
                        if st[idx] != ValKind::SelfFn(*lookup) {
                            return None;
                        }
                    }
                    Instruction::Call { arity, method } => {
                        if !*method || *arity > 4 {
                            return None;
                        }
                        for _ in 0..*arity {
                            if st.pop() != Some(ValKind::Int) {
                                return None;
                            }
                        }
                        if !matches!(st.pop(), Some(ValKind::SelfFn(_))) {
                            return None;
                        }
                        st.push(ValKind::Int);
                    }
                    Instruction::Jump(target) => {
                        if st.iter().any(|k| matches!(k, ValKind::SelfFn(_))) {
                            return None;
                        }
                        if let Some((prev, prev_pushed)) = &entry_stacks[*target] {
                            if prev != &st || *prev_pushed != pushed {
                                return None;
                            }
                        } else {
                            entry_stacks[*target] = Some((st.clone(), pushed));
                            worklist.push(*target);
                        }
                        next_pc = None;
                    }
                    Instruction::JumpIfFalse(target) => {
                        if st.pop() != Some(ValKind::Bool) {
                            return None;
                        }
                        if st.iter().any(|k| matches!(k, ValKind::SelfFn(_))) {
                            return None;
                        }
                        if let Some((prev, prev_pushed)) = &entry_stacks[*target] {
                            if prev != &st || *prev_pushed != pushed {
                                return None;
                            }
                        } else {
                            entry_stacks[*target] = Some((st.clone(), pushed));
                            worklist.push(*target);
                        }
                    }
                    Instruction::Return => {
                        if st.len() != 1 {
                            return None;
                        }
                        let is_int = match st[0] {
                            ValKind::Int => true,
                            ValKind::Bool => false,
                            ValKind::SelfFn(_) => return None,
                        };
                        if let Some(prev) = returns_int {
                            if prev != is_int {
                                return None;
                            }
                        } else {
                            returns_int = Some(is_int);
                        }
                        next_pc = None;
                    }
                    _ => return None,
                }
                let Some(n) = next_pc else {
                    break;
                };
                if n >= code.len() {
                    return None;
                }
                if let Some((prev, prev_pushed)) = &entry_stacks[n] {
                    if prev != &st || *prev_pushed != pushed {
                        return None;
                    }
                    break;
                }
                entry_stacks[n] = Some((st.clone(), pushed));
                pc = n;
            }
        }

        let returns_int = returns_int?;

        // Second pass: emit compact FastIntOps with safe peephole fusion.
        let mut ops = Vec::with_capacity(code.len());
        let mut consts = Vec::new();
        let mut bind_sites = Vec::new();
        let mut self_calls: Vec<SelfCallSite> = Vec::new();
        let mut pc_to_op: Vec<u32> = vec![u32::MAX; code.len() + 1];
        let mut patch_jumps: Vec<(usize, usize)> = Vec::new();

        let mut pc = 0usize;
        while pc < code.len() {
            pc_to_op[pc] = ops.len() as u32;
            if entry_stacks[pc].is_none() {
                pc += 1;
                continue;
            }
            // Peephole 1: `LoadLocal(d); BindLocal { depth: Some(d), .. }` is a pure no-op on verified i64 locals!
            if let Instruction::LoadLocal(d1) = &code[pc]
                && pc + 1 < code.len()
                && !is_jump_target[pc + 1]
                && let Instruction::BindLocal {
                    depth: Some(d2), ..
                } = &code[pc + 1]
                && d1 == d2
            {
                bind_sites.push(pc + 1);
                pc += 2;
                continue;
            }
            // Peephole 2: `Constant(Int(k))` (fits in i32) followed by Add/Sub/Lt/Le/Gt/Ge.
            if let Instruction::Constant(Value::Int(k)) = &code[pc]
                && let Ok(k32) = i32::try_from(k.get())
                && pc + 1 < code.len()
                && !is_jump_target[pc + 1]
            {
                let fused = match &code[pc + 1] {
                    Instruction::Add => Some(FastIntOp::AddConst(k32)),
                    Instruction::Subtract => Some(FastIntOp::SubConst(k32)),
                    Instruction::Less => Some(FastIntOp::LtConst(k32)),
                    Instruction::LessEqual => Some(FastIntOp::LeConst(k32)),
                    Instruction::Greater => Some(FastIntOp::GtConst(k32)),
                    Instruction::GreaterEqual => Some(FastIntOp::GeConst(k32)),
                    _ => None,
                };
                if let Some(op) = fused {
                    ops.push(op);
                    pc += 2;
                    continue;
                }
            }

            match &code[pc] {
                Instruction::Observe(_)
                | Instruction::ObserveNativeLiteral { .. }
                | Instruction::CheckType(_)
                | Instruction::GetMethodProperty { .. }
                | Instruction::PrepareMethodCall { .. } => {}
                Instruction::LoadLocal(d) => {
                    ops.push(FastIntOp::LoadLocal(u16::try_from(*d).ok()?));
                }
                Instruction::StoreLocal(d) => {
                    ops.push(FastIntOp::StoreLocal(u16::try_from(*d).ok()?));
                }
                Instruction::BindLocal { depth, .. } => {
                    bind_sites.push(pc);
                    match depth {
                        Some(d) => ops.push(FastIntOp::StoreLocal(u16::try_from(*d).ok()?)),
                        None => ops.push(FastIntOp::BindNewLocal),
                    }
                }
                Instruction::PopLocal => ops.push(FastIntOp::PopLocal),
                Instruction::Constant(Value::Int(k)) => {
                    let v = k.get();
                    if let Ok(k32) = i32::try_from(v) {
                        ops.push(FastIntOp::ConstSmall(k32));
                    } else {
                        let idx = u16::try_from(consts.len()).ok()?;
                        consts.push(v);
                        ops.push(FastIntOp::ConstPool(idx));
                    }
                }
                Instruction::Constant(Value::Bool(b)) => {
                    ops.push(FastIntOp::ConstSmall(i32::from(*b)));
                }
                Instruction::Pop => ops.push(FastIntOp::Pop),
                Instruction::Duplicate => ops.push(FastIntOp::Dup),
                Instruction::Swap => ops.push(FastIntOp::Swap),
                Instruction::AddIntConstant(k) => {
                    if let Ok(k32) = i32::try_from(*k) {
                        ops.push(FastIntOp::AddConst(k32));
                    } else {
                        let idx = u16::try_from(consts.len()).ok()?;
                        consts.push(*k);
                        ops.push(FastIntOp::ConstPool(idx));
                        ops.push(FastIntOp::Add);
                    }
                }
                Instruction::Add => ops.push(FastIntOp::Add),
                Instruction::Subtract => ops.push(FastIntOp::Sub),
                Instruction::Multiply => ops.push(FastIntOp::Mul),
                Instruction::Less => ops.push(FastIntOp::Lt),
                Instruction::LessEqual => ops.push(FastIntOp::Le),
                Instruction::Greater => ops.push(FastIntOp::Gt),
                Instruction::GreaterEqual => ops.push(FastIntOp::Ge),
                Instruction::Equal => ops.push(FastIntOp::Eq),
                Instruction::NotEqual => ops.push(FastIntOp::Ne),
                Instruction::Call { arity, .. } => {
                    // Find matching SelfFn lookup from abstract stack at this PC.
                    let (st, _) = entry_stacks[pc].as_ref()?;
                    let ValKind::SelfFn(lookup_pc) = st[st.len() - *arity - 1] else {
                        return None;
                    };
                    let Instruction::GetMethodProperty {
                        name,
                        owner_depth,
                        local,
                    } = &code[lookup_pc]
                    else {
                        return None;
                    };
                    self_calls.push(SelfCallSite {
                        lookup_pc,
                        name: name.clone(),
                        owner_depth: *owner_depth,
                        local: *local,
                        arity: *arity,
                    });
                    ops.push(FastIntOp::CallSelf(*arity as u8));
                }
                Instruction::Jump(target) => {
                    // Peephole 3: jumping directly to `(CheckType)*; Return` becomes `Return`!
                    let mut t = *target;
                    while let Some(
                        Instruction::CheckType(_)
                        | Instruction::Observe(_)
                        | Instruction::ObserveNativeLiteral { .. },
                    ) = code.get(t)
                    {
                        t += 1;
                    }
                    if matches!(code.get(t), Some(Instruction::Return)) {
                        ops.push(FastIntOp::Return);
                    } else {
                        let op_idx = ops.len();
                        ops.push(FastIntOp::Jump(0));
                        patch_jumps.push((op_idx, *target));
                    }
                }
                Instruction::JumpIfFalse(target) => {
                    let op_idx = ops.len();
                    ops.push(FastIntOp::JumpIfFalse(0));
                    patch_jumps.push((op_idx, *target));
                }
                Instruction::Return => ops.push(FastIntOp::Return),
                _ => return None,
            }
            pc += 1;
        }
        pc_to_op[code.len()] = ops.len() as u32;

        for (op_idx, target_pc) in patch_jumps {
            let mut t = target_pc;
            while t < code.len() && pc_to_op[t] == u32::MAX {
                t += 1;
            }
            let target_op = pc_to_op[t];
            if target_op as usize >= ops.len() {
                return None;
            }
            match &mut ops[op_idx] {
                FastIntOp::Jump(dst) | FastIntOp::JumpIfFalse(dst) => *dst = target_op,
                _ => unreachable!(),
            }
        }

        let has_self_calls = !self_calls.is_empty();
        // Self-calls in the abstract verifier assume `ValKind::Int` returns; reject recursive
        // Bool-returning functions and arity-mismatched self-calls before execution.
        if has_self_calls && (!returns_int || self_calls.iter().any(|c| c.arity != arity)) {
            return None;
        }
        // Only lower to the expression-tree IR when recursion or 2-arg fold matching
        // will amortize the lowering allocation; non-recursive 1-shot helpers execute
        // directly on the compact bytecode ops.
        let tree = if has_self_calls || arity == 2 {
            FastIntTree::lower(&ops, &consts, arity)
        } else {
            None
        };
        Some(Self {
            ops,
            consts,
            bind_sites,
            self_calls,
            has_self_calls,
            returns_int,
            min_arity_no_capture,
            tree,
        })
    }

    /// Returns `Some(0)` for `(a, b) -> a + b`, `Some(1)` for `(a, b) -> a - b`,
    /// `Some(2)` for `(a, b) -> a * b` when no locals are captured.
    #[inline]
    pub(super) fn try_fold_add_sub_mul(&self, captured: &[i64]) -> Option<u8> {
        if !captured.is_empty() {
            return None;
        }
        let tree = self.tree.as_ref()?;
        if tree.arity != 2 {
            return None;
        }
        match tree.nodes[tree.root as usize] {
            FastExpr::Add(l, r)
                if matches!(tree.nodes[l as usize], FastExpr::Arg(0))
                    && matches!(tree.nodes[r as usize], FastExpr::Arg(1)) =>
            {
                Some(0)
            }
            FastExpr::Sub(l, r)
                if matches!(tree.nodes[l as usize], FastExpr::Arg(0))
                    && matches!(tree.nodes[r as usize], FastExpr::Arg(1)) =>
            {
                Some(1)
            }
            FastExpr::Mul(l, r)
                if matches!(tree.nodes[l as usize], FastExpr::Arg(0))
                    && matches!(tree.nodes[r as usize], FastExpr::Arg(1)) =>
            {
                Some(2)
            }
            _ => None,
        }
    }

    /// Executes this `FastIntBlock` on a pure `i64` scratchpad.
    /// Returns `Ok((result_i64, steps_consumed))` on clean completion, or
    /// `Err(steps_consumed)` on overflow, recursion limit, or step-budget
    /// exhaustion (zero side effects, while still billing consumed steps so
    /// cooperative timeout/cancellation cannot be bypassed).
    #[cfg_attr(not(target_arch = "wasm32"), inline)]
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn execute(
        &self,
        captured: &[i64],
        args: &[i64],
        max_steps: usize,
        val_stack: &mut Vec<i64>,
        locals_buf: &mut Vec<i64>,
        call_stack: &mut Vec<(u32, u32)>,
    ) -> Result<(i64, usize), usize> {
        let mut prior_steps = 0usize;
        if captured.is_empty()
            && let Some(tree) = &self.tree
            && args.len() == tree.arity as usize
        {
            match tree.execute(args, max_steps) {
                TreeResult::Done(val, consumed) => return Ok((val, consumed)),
                TreeResult::Abort(consumed) => return Err(consumed),
                TreeResult::DepthExceeded(consumed) => {
                    prior_steps = consumed;
                    if prior_steps >= max_steps {
                        return Err(prior_steps);
                    }
                }
            }
        }
        let rem_steps = max_steps.saturating_sub(prior_steps);
        let mut steps: usize = 0;
        match self.execute_ops(
            captured, args, rem_steps, &mut steps, val_stack, locals_buf, call_stack,
        ) {
            Some(val) => Ok((val, prior_steps.saturating_add(steps).max(1))),
            None => Err(prior_steps.saturating_add(steps).max(1)),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_ops(
        &self,
        captured: &[i64],
        args: &[i64],
        max_steps: usize,
        steps_out: &mut usize,
        val_stack: &mut Vec<i64>,
        locals_buf: &mut Vec<i64>,
        call_stack: &mut Vec<(u32, u32)>,
    ) -> Option<i64> {
        val_stack.clear();
        locals_buf.clear();
        call_stack.clear();
        if !captured.is_empty() {
            locals_buf.extend_from_slice(captured);
        }
        locals_buf.extend_from_slice(args);

        let ops = self.ops.as_slice();
        let consts = self.consts.as_slice();
        let mut pc: usize = 0;
        let mut locals_base: usize = 0;

        while *steps_out < max_steps {
            let op = *ops.get(pc)?;
            *steps_out += 1;
            match op {
                FastIntOp::LoadLocal(depth) => {
                    let idx = locals_buf.len().checked_sub(1 + depth as usize)?;
                    val_stack.push(*locals_buf.get(idx)?);
                    pc += 1;
                }
                FastIntOp::StoreLocal(depth) => {
                    let v = val_stack.pop()?;
                    let idx = locals_buf.len().checked_sub(1 + depth as usize)?;
                    *locals_buf.get_mut(idx)? = v;
                    pc += 1;
                }
                FastIntOp::BindNewLocal => {
                    let v = val_stack.pop()?;
                    locals_buf.push(v);
                    pc += 1;
                }
                FastIntOp::PopLocal => {
                    locals_buf.pop()?;
                    pc += 1;
                }
                FastIntOp::ConstSmall(k) => {
                    val_stack.push(i64::from(k));
                    pc += 1;
                }
                FastIntOp::ConstPool(idx) => {
                    val_stack.push(*consts.get(idx as usize)?);
                    pc += 1;
                }
                FastIntOp::Add => {
                    let r = val_stack.pop()?;
                    let l = val_stack.last_mut()?;
                    *l = l.checked_add(r)?;
                    pc += 1;
                }
                FastIntOp::Sub => {
                    let r = val_stack.pop()?;
                    let l = val_stack.last_mut()?;
                    *l = l.checked_sub(r)?;
                    pc += 1;
                }
                FastIntOp::Mul => {
                    let r = val_stack.pop()?;
                    let l = val_stack.last_mut()?;
                    *l = l.checked_mul(r)?;
                    pc += 1;
                }
                FastIntOp::AddConst(k) => {
                    let l = val_stack.last_mut()?;
                    *l = l.checked_add(i64::from(k))?;
                    pc += 1;
                }
                FastIntOp::SubConst(k) => {
                    let l = val_stack.last_mut()?;
                    *l = l.checked_sub(i64::from(k))?;
                    pc += 1;
                }
                FastIntOp::Lt => {
                    let r = val_stack.pop()?;
                    let l = val_stack.last_mut()?;
                    *l = i64::from(*l < r);
                    pc += 1;
                }
                FastIntOp::Le => {
                    let r = val_stack.pop()?;
                    let l = val_stack.last_mut()?;
                    *l = i64::from(*l <= r);
                    pc += 1;
                }
                FastIntOp::Gt => {
                    let r = val_stack.pop()?;
                    let l = val_stack.last_mut()?;
                    *l = i64::from(*l > r);
                    pc += 1;
                }
                FastIntOp::Ge => {
                    let r = val_stack.pop()?;
                    let l = val_stack.last_mut()?;
                    *l = i64::from(*l >= r);
                    pc += 1;
                }
                FastIntOp::Eq => {
                    let r = val_stack.pop()?;
                    let l = val_stack.last_mut()?;
                    *l = i64::from(*l == r);
                    pc += 1;
                }
                FastIntOp::Ne => {
                    let r = val_stack.pop()?;
                    let l = val_stack.last_mut()?;
                    *l = i64::from(*l != r);
                    pc += 1;
                }
                FastIntOp::LtConst(k) => {
                    let l = val_stack.last_mut()?;
                    *l = i64::from(*l < i64::from(k));
                    pc += 1;
                }
                FastIntOp::LeConst(k) => {
                    let l = val_stack.last_mut()?;
                    *l = i64::from(*l <= i64::from(k));
                    pc += 1;
                }
                FastIntOp::GtConst(k) => {
                    let l = val_stack.last_mut()?;
                    *l = i64::from(*l > i64::from(k));
                    pc += 1;
                }
                FastIntOp::GeConst(k) => {
                    let l = val_stack.last_mut()?;
                    *l = i64::from(*l >= i64::from(k));
                    pc += 1;
                }
                FastIntOp::Jump(target) => {
                    pc = target as usize;
                }
                FastIntOp::JumpIfFalse(target) => {
                    let cond = val_stack.pop()?;
                    pc = if cond != 0 { pc + 1 } else { target as usize };
                }
                FastIntOp::Pop => {
                    val_stack.pop()?;
                    pc += 1;
                }
                FastIntOp::Dup => {
                    let v = *val_stack.last()?;
                    val_stack.push(v);
                    pc += 1;
                }
                FastIntOp::Swap => {
                    let n = val_stack.len();
                    if n < 2 {
                        return None;
                    }
                    val_stack.swap(n - 1, n - 2);
                    pc += 1;
                }
                FastIntOp::CallSelf(arity) => {
                    if call_stack.len() >= 4000 {
                        return None;
                    }
                    call_stack.push(((pc + 1) as u32, locals_base as u32));
                    locals_base = locals_buf.len();
                    if !captured.is_empty() {
                        locals_buf.extend_from_slice(captured);
                    }
                    let a = arity as usize;
                    if a == 1 {
                        let a0 = val_stack.pop()?;
                        locals_buf.push(a0);
                    } else if a == 2 {
                        let a1 = val_stack.pop()?;
                        let a0 = val_stack.pop()?;
                        locals_buf.push(a0);
                        locals_buf.push(a1);
                    } else {
                        let start = val_stack.len().checked_sub(a)?;
                        locals_buf.extend_from_slice(&val_stack[start..]);
                        val_stack.truncate(start);
                    }
                    pc = 0;
                }
                FastIntOp::Return => {
                    if let Some((ret_pc, prev_base)) = call_stack.pop() {
                        locals_buf.truncate(locals_base);
                        locals_base = prev_base as usize;
                        pc = ret_pc as usize;
                    } else {
                        let result = val_stack.pop()?;
                        return Some(result);
                    }
                }
            }
        }
        None
    }
}

/// Verifies that `fn_id` (with `object`) can safely execute its `FastIntBlock` right now:
/// - `active_type_trackers == 0`
/// - Every `BindLocal` site in `fast_block` already has `heap.local_kind` recorded (`Long | Object`)
/// - Every `SelfCallSite` resolves directly (non-open, non-abstract or local, non-deprecated, non-saturated) to `fn_id`
/// - All captured locals are `Value::Int` (copied into `captured_out`)
#[cfg_attr(not(target_arch = "wasm32"), inline)]
#[cfg_attr(target_arch = "wasm32", inline(never))]
pub(super) fn verify_fast_int_invocation(
    fast_block: &FastIntBlock,
    block_id: usize,
    fn_id: ObjectId,
    object: &Object,
    heap: &Heap,
    executable_roots: &super::ExecutableRoots,
    captured_out: &mut Vec<i64>,
) -> bool {
    captured_out.clear();
    let arity = object.function.as_ref().map_or(0, |f| f.arity);
    if fast_block.min_arity_no_capture > arity {
        for local in &object.environment.locals {
            if let Value::Int(n) = local {
                captured_out.push(n.get());
            } else {
                return false;
            }
        }
    }
    let exec_owner = object
        .environment
        .executable_owner
        .or_else(|| executable_roots.get(&block_id).copied());

    if !fast_block.bind_sites.is_empty() {
        let Some(owner) = exec_owner else {
            return false;
        };
        for &site_pc in &fast_block.bind_sites {
            match heap.local_kind(owner, (block_id, site_pc)) {
                Ok(Some(identity::SlotKind::Long | identity::SlotKind::Object)) => {}
                _ => return false,
            }
        }
    }

    if fast_block.has_self_calls {
        let self_arity = match object.function.as_ref() {
            Some(f) => f.arity,
            None => return false,
        };
        if let Some((decl_owner, decl_code)) = object.method_declaration
            && heap
                .get(decl_owner)
                .is_ok_and(|o| o.member_deprecations.contains_key(&decl_code))
        {
            return false;
        }
        for call in &fast_block.self_calls {
            if call.arity != self_arity {
                return false;
            }
            let Some(&scope) = object.environment.scopes.get(call.owner_depth) else {
                return false;
            };
            let Ok(owner_obj) = heap.get(scope.owner) else {
                return false;
            };
            let Some(class) = owner_obj.class_definition.as_ref() else {
                return false;
            };
            let direct = call.local || !(class.open || class.abstract_);
            if !direct {
                return false;
            }
            if let Some(owner) = exec_owner {
                match heap.existing_method_call_cache(owner, (block_id, call.lookup_pc)) {
                    Ok(Some(cache)) if cache.is_saturated() => return false,
                    Err(_) => return false,
                    _ => {}
                }
            }
            let Ok(recv_obj) = heap.get(scope.receiver) else {
                return false;
            };
            let key = CacheKey {
                entry: false,
                owner: call.local.then_some(scope.owner),
                name: call.name.clone(),
                method: true,
            };
            match recv_obj.cache_get_completed(&key) {
                Some(Value::Function(resolved_id)) if *resolved_id == fn_id => {}
                _ => return false,
            }
        }
    }
    true
}

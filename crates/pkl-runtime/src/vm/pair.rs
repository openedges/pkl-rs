//! Immutable Pair components are traced and evaluated by the common VM tasks.
use super::*;
pub(super) fn constructor_block(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> usize {
    let source = crate::stdlib_sources::base();
    let declaration = "external const function Pair";
    let start = source.text.find(declaration).expect("Pair constructor");
    let end = start + source.text[start..].find('\n').unwrap();
    let block = blocks.len();
    blocks.push(vec![
        Instruction::LoadLocal(1),
        Instruction::LoadLocal(0),
        Instruction::MakePair,
        Instruction::Return,
    ]);
    metadata.push(BlockMetadata {
        source: Some(source),
        member: "#Pair".into(),
        declaration: Some(Span::new(start, end)),
        spans: vec![None; 4],
        ..Default::default()
    });
    block
}
impl Vm {
    pub(super) fn make_pair(&mut self, first: Value, second: Value) -> Value {
        let parent = self
            .builtin_class(TypeCheck::Pair {
                arguments: Vec::new(),
            })
            .heap_id()
            .expect("Pair class");
        let class = parent;
        let parent = self.heap.get_class(class).expect("builtin class").prototype;
        Value::Object(self.heap.allocate(Object {
            pair: Some(Box::new([first, second])),
            parent: Some(parent),
            instance_class: Some(class),
            ..Default::default()
        }))
    }
    pub(super) fn pair_constructor(&mut self) -> Value {
        Value::Function(self.heap.allocate(Object {
            function: Some(Function {
                block: self.program.pair_constructor_block,
                arity: 2,
                method: true,
                captured_this: None,
            }),
            ..Default::default()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pair_components_and_class_survive_only_while_the_pair_is_rooted() {
        let program = Arc::new(Program::new(vec![vec![Instruction::Return]]));
        let mut vm = Vm::new(program);
        let first = vm.heap.allocate(Object::default());
        let second = vm.heap.allocate(Object::default());
        let pair = vm.make_pair(Value::Object(first), Value::Object(second));
        let owner = pair.as_object().unwrap();
        vm.collect(std::slice::from_ref(&pair));
        assert!(vm.heap.get(first).is_ok());
        assert!(vm.heap.get(second).is_ok());
        assert!(vm.heap.get(owner).is_ok());
        vm.collect(&[]);
        assert!(vm.heap.get(owner).is_err());
        assert!(vm.heap.get(first).is_err());
        assert!(vm.heap.get(second).is_err());
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }
}

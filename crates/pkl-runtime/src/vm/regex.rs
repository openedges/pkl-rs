//! Language Regex calls and ordinary typed RegexMatch factory members.
use super::*;
use crate::pkl_regex::{PklRegex, RegexError, RegexMatchData};

const MATCH_FACTORY: usize = builtins::REGEX_MATCH - 1;

#[derive(Clone, Copy, Debug)]
pub enum Operation {
    FindMatchesIn,
    MatchEntire,
}

#[derive(Clone, Copy, Debug)]
pub enum Property {
    Value,
    Start,
    End,
    Groups,
}

pub(super) fn string(regex: &PklRegex) -> PklString {
    PklString::join(
        &[
            "Regex(".into(),
            pcf_format::single_line_custom_string(regex.pattern()),
            ")".into(),
        ],
        "",
    )
}

#[cfg_attr(target_arch = "wasm32", inline(never))]
pub(super) fn blocks(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<(Arc<str>, Arc<str>), usize> {
    let source = crate::stdlib_sources::base();
    let mut methods = BTreeMap::new();
    for (name, class, signature, operation) in [
        (
            "Regex",
            "",
            "external const function Regex(pattern: String): Regex",
            None,
        ),
        (
            "findMatchesIn",
            "Regex",
            "external function findMatchesIn(input: String): List<RegexMatch>",
            Some(Operation::FindMatchesIn),
        ),
        (
            "matchEntire",
            "Regex",
            "external function matchEntire(input: String): RegexMatch?",
            Some(Operation::MatchEntire),
        ),
    ] {
        let start = source.text.find(signature).expect("Regex declaration");
        let argument = start + signature.find(": String").unwrap() + 2;
        let block = blocks.len();
        blocks.push(vec![
            Instruction::LoadLocal(0),
            Instruction::CheckType(Box::new(TypeCheck::String)),
            operation.map_or(Instruction::MakeRegex, |operation| {
                Instruction::Primitive(primitives::Operation::Regex(operation))
            }),
            Instruction::Return,
        ]);
        metadata.push(BlockMetadata {
            source: Some(source.clone()),
            member: if class.is_empty() {
                "#Regex".into()
            } else {
                format!("#Regex.{name}").into()
            },
            declaration: Some(Span::new(start, start + signature.len())),
            spans: vec![None, Some(Span::new(argument, argument + 6)), None, None],
            ..Default::default()
        });
        methods.insert((class.into(), name.into()), block);
    }
    methods
}

#[cfg_attr(target_arch = "wasm32", inline(never))]
pub(super) fn builtin_members(
    blocks: &mut Vec<Vec<Instruction>>,
    metadata: &mut Vec<BlockMetadata>,
) -> BTreeMap<usize, Vec<Member>> {
    let source = crate::stdlib_sources::base();
    let mut d = output::Definitions {
        source,
        blocks,
        metadata,
    };
    let group_start =
        d.source.text.find("groups: List<RegexMatch?>").unwrap() + "groups: List<".len();
    let groups = TypeCheck::ParameterizedCollection {
        kind: crate::heap::CollectionKind::List,
        arguments: vec![(
            TypeCheck::Nullable(Box::new(builtins::ty(builtins::REGEX_MATCH))),
            Span::new(group_start, group_start + "RegexMatch".len()),
        )],
    };
    let factory = [
        ("value", TypeCheck::String, Property::Value),
        ("start", TypeCheck::Int, Property::Start),
        ("end", TypeCheck::Int, Property::End),
        ("groups", groups, Property::Groups),
    ]
    .into_iter()
    .map(|(name, ty, property)| {
        d.member(
            "RegexMatch",
            name,
            ty,
            vec![Instruction::RegexMatchProperty(property)],
            false,
        )
    })
    .collect();
    // External Regex values use primitive dispatch. Prototype member metadata
    // still supplies Reference property types without constructing a value.
    let regex = [
        ("pattern", TypeCheck::String),
        ("groupCount", TypeCheck::Int),
    ]
    .into_iter()
    .map(|(name, ty)| {
        d.member(
            "Regex",
            name,
            ty,
            vec![Instruction::Undefined(name.into())],
            false,
        )
    })
    .collect();
    BTreeMap::from([(MATCH_FACTORY, factory), (classes::REGEX, regex)])
}

pub(super) fn backend_error(error: RegexError, pattern: &PklString) -> RuntimeError {
    match error {
        RegexError::Syntax(message) => RuntimeError::new(
            ErrorKind::Type,
            PklString::join(
                &[
                    "Syntax error in regex `".into(),
                    pattern.clone(),
                    "`: ".into(),
                    message,
                ],
                "",
            ),
        ),
        RegexError::Unsupported(message) => RuntimeError::new(ErrorKind::Unsupported, message),
        RegexError::Execution(error) => RuntimeError::new(
            ErrorKind::Unsupported,
            format!("Regex backend execution failed: {error}"),
        ),
        RegexError::InvalidBackendRange => {
            invalid("regex backend returned an invalid capture range")
        }
    }
}

impl Vm {
    pub(super) fn regex_constructor(&mut self) -> Result<Value, RuntimeError> {
        let frame = self
            .frames
            .last()
            .ok_or_else(|| invalid("Regex constructor frame"))?;
        let site = ExecutionSite::RegexConstructor {
            block: frame.block,
            instruction: frame.pc - 1,
        };
        let parent = self.current_executable()?;
        let owner = self.heap.executable_child(parent, site)?;
        Ok(Value::Function(self.heap.allocate(Object {
            environment: Environment {
                executable_owner: Some(owner),
                ..Default::default()
            },
            function: Some(Function {
                block: self.program.primitive_methods.expect("", "Regex"),
                arity: 1,
                method: true,
                captured_this: None,
            }),
            ..Default::default()
        })))
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn make_regex(&mut self) -> Result<(), RuntimeError> {
        let Value::String(pattern) = self.pop()? else {
            return Err(invalid("checked Regex pattern"));
        };
        let owner = self.current_executable()?;
        let regex = self
            .heap
            .regex_constructor_cache(owner)?
            .compile(pattern.clone())
            .map_err(|error| backend_error(error, &pattern))?;
        self.stack.push(Value::Regex(regex));
        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn regex_operation(
        &mut self,
        receiver: Value,
        operation: Operation,
    ) -> Result<(), RuntimeError> {
        let Value::Regex(regex) = receiver else {
            return Err(invalid("Regex method receiver"));
        };
        let Value::String(input) = self.pop()? else {
            return Err(invalid("checked Regex input"));
        };
        match operation {
            Operation::FindMatchesIn => {
                let matches = regex
                    .find_matches_in(&input)
                    .map_err(|error| backend_error(error, regex.pattern()))?;
                let length = matches.len();
                for matched in matches {
                    let value = self.regex_match(matched);
                    self.stack.push(value);
                }
                self.collection_instruction(Instruction::MakeList { length })?;
            }
            Operation::MatchEntire => {
                let matched = regex
                    .match_entire(&input)
                    .map_err(|error| backend_error(error, regex.pattern()))?;
                let value = matched.map_or(Value::Null, |matched| self.regex_match(matched));
                self.stack.push(value);
            }
        }
        Ok(())
    }

    pub(super) fn regex_match(&mut self, matched: RegexMatchData) -> Value {
        let class = self
            .builtin_class(builtins::ty(builtins::REGEX_MATCH))
            .heap_id()
            .expect("RegexMatch class");
        let parent = self
            .heap
            .get_class(class)
            .expect("RegexMatch class")
            .prototype;
        Value::Object(self.heap.allocate(Object {
            parent: Some(parent),
            instance_class: Some(class),
            regex_match: Some(Arc::new(matched)),
            members: self.program.builtin_members[&MATCH_FACTORY].clone().into(),
            ..Default::default()
        }))
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn regex_match_property(&mut self, property: Property) -> Result<(), RuntimeError> {
        let owner = self
            .frames
            .last()
            .and_then(|frame| frame.force.as_ref())
            .map(|force| force.owner)
            .ok_or_else(|| invalid("RegexMatch property without declaration owner"))?;
        let data = self
            .heap
            .get(owner)?
            .regex_match
            .clone()
            .ok_or_else(|| invalid("RegexMatch factory payload missing"))?;
        let value = match property {
            Property::Value => Value::String(data.value()),
            Property::Start => Value::Int((data.start() as i64).into()),
            Property::End => Value::Int((data.end() as i64).into()),
            Property::Groups => {
                let mut length = 0;
                for group in data.groups() {
                    let value = group.map_or(Value::Null, |group| self.regex_match(group));
                    self.stack.push(value);
                    length += 1;
                }
                self.collection_instruction(Instruction::MakeList { length })?;
                return Ok(());
            }
        };
        self.stack.push(value);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructor_state_lives_with_executable_without_retaining_heap_receivers() {
        let program = Arc::new(Program::new(vec![vec![Instruction::Return]]));
        let mut vm = Vm::new(program);
        let owner = vm.heap.allocate_executable();
        let value = vm
            .heap
            .regex_constructor_cache(owner)
            .unwrap()
            .compile("(a)".into())
            .unwrap();
        vm.collect(&[owner.root()]);
        assert!(vm.heap.executable_is_live(owner));
        assert_eq!(
            value.reference(),
            vm.heap
                .regex_constructor_cache(owner)
                .unwrap()
                .compile("(a)".into())
                .unwrap()
                .reference()
        );
        vm.collect(&[Value::Regex(value.clone())]);
        assert!(!vm.heap.executable_is_live(owner));
        assert_eq!(value.group_count(), 1);
        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }
}

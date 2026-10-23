//! Ordered renderer converter selection and source-backed path grammar.
use super::*;
#[derive(Clone)]
pub(super) enum PathPart {
    Root,
    Property(PklString),
    Entry(Value),
}
#[derive(Clone)]
enum SpecPart {
    Root,
    Property(PklString),
    Entry(PklString),
    AnyProperty,
    AnyEntry,
}
#[derive(Default)]
pub(super) struct Converters {
    types: Vec<(ObjectId, Value)>,
    paths: Vec<(Vec<SpecPart>, Value)>,
    pub(super) transformers: Vec<(ObjectId, Value)>,
}
impl Converters {
    pub(super) fn roots(&self, out: &mut Vec<Value>) {
        for (class, f) in self.types.iter().chain(&self.transformers) {
            out.push(Value::Class(*class));
            out.push(f.clone());
        }
        out.extend(self.paths.iter().map(|(_, f)| f.clone()));
    }
    pub(super) fn load(
        heap: &Heap,
        mapping: ObjectId,
        transformers: ObjectId,
    ) -> Result<Self, RuntimeError> {
        let mut result = Self::default();
        let mut paths = Vec::new();
        for member in collections::ordered_members(heap, mapping)? {
            let value = collections::cached_render_value(heap, mapping, &member)?
                .ok_or_else(|| invalid("converter not cached"))?;
            match member.key {
                Value::Class(class) => result.types.push((class, value)),
                Value::String(spec) => paths.push((spec, value)),
                _ => {
                    return Err(RuntimeError::new(
                        ErrorKind::Type,
                        "Renderer converter keys must be classes or strings.",
                    ));
                }
            }
        }
        for member in collections::ordered_members(heap, transformers)? {
            let Value::Class(class) = member.key else {
                return Err(RuntimeError::new(
                    ErrorKind::Type,
                    "Property transformer keys must be classes.",
                ));
            };
            let value = collections::cached_render_value(heap, transformers, &member)?
                .ok_or_else(|| invalid("transformer not cached"))?;
            result.transformers.push((class, value));
        }
        for (spec, value) in paths {
            result.paths.push((parse(&spec)?, value));
        }
        Ok(result)
    }
    pub(super) fn transformer(
        &self,
        vm: &Vm,
        annotation: ObjectId,
    ) -> Result<Option<Value>, RuntimeError> {
        let mut current = Some(annotation);
        let mut seen = BTreeSet::new();
        while let Some(id) = current {
            if !seen.insert(id) {
                return Err(RuntimeError::new(
                    ErrorKind::Cycle,
                    "cyclic class hierarchy",
                ));
            }
            if let Some((_, f)) = self.transformers.iter().find(|(class, _)| *class == id) {
                return Ok(Some(f.clone()));
            }
            current = vm.heap.get(id)?.parent;
        }
        Ok(None)
    }
    pub(super) fn select(
        &self,
        vm: &mut Vm,
        value: &Value,
        path: &[PathPart],
    ) -> Result<Option<Value>, RuntimeError> {
        for (spec, f) in &self.paths {
            if spec.len() <= path.len()
                && spec
                    .iter()
                    .rev()
                    .zip(path.iter().rev())
                    .all(|(s, p)| match (s, p) {
                        (SpecPart::Root, PathPart::Root) => true,
                        (SpecPart::Property(a), PathPart::Property(b)) => a == b,
                        (SpecPart::Entry(a), PathPart::Entry(Value::String(b))) => a == b,
                        (SpecPart::AnyProperty, PathPart::Property(_))
                        | (SpecPart::AnyEntry, PathPart::Entry(_)) => true,
                        _ => false,
                    })
            {
                return Ok(Some(f.clone()));
            }
        }
        let Value::Class(class) = vm.reference_get_class(value)? else {
            return Err(invalid("value class"));
        };
        let typed = matches!(value,Value::Object(id) if vm.heap.get(*id)?.pair.is_none() && vm.heap.get(*id)?.reference.is_none() && collections::typed_class(&vm.heap,*id)?.is_some());
        let mut current = Some(class);
        let mut seen = BTreeSet::new();
        while let Some(class) = current {
            if !seen.insert(class) {
                return Err(RuntimeError::new(
                    ErrorKind::Cycle,
                    "cyclic class hierarchy",
                ));
            }
            if let Some((_, f)) = self.types.iter().find(|(id, _)| *id == class) {
                return Ok(Some(f.clone()));
            }
            if !typed {
                break;
            }
            current = vm.heap.get(class)?.parent;
        }
        Ok(None)
    }
}
fn parse(spec: &PklString) -> Result<Vec<SpecPart>, RuntimeError> {
    let units: Vec<_> = spec.encode_utf16().collect();
    let mut state = 0;
    let mut start = 0;
    let mut result = Vec::new();
    let invalid = || {
        RuntimeError::new(
            ErrorKind::Type,
            PklString::from("Converter path `")
                .concat(spec)
                .concat(&"` has invalid syntax.".into()),
        )
    };
    for (i, &u) in units.iter().enumerate() {
        match u {
            94 => {
                if i != 0 {
                    return Err(invalid());
                }
                result.push(SpecPart::Root);
                start = 1;
            }
            46 => {
                match state {
                    1 => {
                        if start == i {
                            return Err(invalid());
                        }
                        result.push(SpecPart::Property(PklString::from_utf16(
                            units[start..i].to_vec(),
                        )));
                    }
                    3 | 4 => {}
                    _ => return Err(invalid()),
                }
                start = i + 1;
                state = 1;
            }
            91 => {
                match state {
                    1 => {
                        if start == i {
                            return Err(invalid());
                        }
                        result.push(SpecPart::Property(PklString::from_utf16(
                            units[start..i].to_vec(),
                        )));
                    }
                    0 | 3 | 4 => {}
                    _ => return Err(invalid()),
                }
                start = i + 1;
                state = 2;
            }
            93 => {
                match state {
                    2 => {
                        if start == i {
                            return Err(invalid());
                        }
                        result.push(SpecPart::Entry(PklString::from_utf16(
                            units[start..i].to_vec(),
                        )));
                    }
                    5 => {}
                    _ => return Err(invalid()),
                }
                state = 3;
            }
            42 => {
                if start != i {
                    return Err(invalid());
                }
                state = match state {
                    0 | 1 => {
                        result.push(SpecPart::AnyProperty);
                        4
                    }
                    2 => {
                        result.push(SpecPart::AnyEntry);
                        5
                    }
                    _ => return Err(invalid()),
                };
            }
            _ => {
                if state > 2 {
                    return Err(invalid());
                }
                if state == 0 {
                    state = 1;
                }
            }
        }
    }
    match state {
        0 => {
            if result.is_empty() {
                result.push(SpecPart::Root);
            }
        }
        1 => {
            if start == units.len() {
                return Err(invalid());
            }
            result.push(SpecPart::Property(PklString::from_utf16(
                units[start..].to_vec(),
            )));
        }
        3 | 4 => {}
        _ => return Err(invalid()),
    }
    Ok(result)
}

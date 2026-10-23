//! Identity and inheritance for source-backed ordinary standard-library classes.
use super::*;

pub(crate) fn id(name: &str) -> Option<usize> {
    annotations::builtin_id(name).or_else(|| output::class_id(name))
}
pub(crate) fn name(id: usize) -> Option<&'static str> {
    annotations::builtin_name(id).or_else(|| output::class_name(id))
}
pub(super) fn parent(id: usize) -> Option<usize> {
    if annotations::builtin_name(id).is_some() && id != annotations::ANNOTATION_ID {
        Some(annotations::ANNOTATION_ID)
    } else {
        output::class_parent(id)
    }
}
pub(super) fn abstract_class(id: usize) -> bool {
    id == annotations::ANNOTATION_ID || output::abstract_class(id)
}
pub(super) fn open_class(id: usize) -> bool {
    id == annotations::ANNOTATION_ID || output::open_class(id)
}
pub(super) fn ty(id: usize) -> TypeCheck {
    TypeCheck::Class {
        module: None,
        id,
        name: name(id).expect("registered builtin class").into(),
    }
}

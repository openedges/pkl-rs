//! Pinned MinPklVersionChecker's partial-parser path, separate from evaluation.
use crate::diagnostic::Diagnostic;
use crate::pkl_string::PklStringBuilder;
use crate::source::{SourceSlice, SourceText};
use crate::syntax::{ExprKind, ModuleDeclaration, ObjectMember, TypeKind};
use crate::value::{ErrorKind, PklString, RuntimeError};

/// Retain the real parser commit point and its original source/diagnostic.
/// The completed AST retains annotation order, syntactic type wrappers, method
/// productions and amendment-only property metadata before any VM lowering.
#[derive(Clone, Debug)]
pub(crate) struct ModuleParseFailure {
    pub(crate) diagnostic: Diagnostic,
    pub(crate) source: SourceText,
    pub(crate) completed_declaration: Option<ModuleDeclaration>,
}

impl ModuleParseFailure {
    pub(super) fn new(
        diagnostic: Diagnostic,
        source: SourceSlice<'_>,
        completed_declaration: Option<ModuleDeclaration>,
    ) -> Box<Self> {
        Box::new(Self {
            diagnostic,
            source: SourceText::from(source.to_pkl_string()),
            completed_declaration,
        })
    }

    /// Run only after ModuleKey name inference and only on a parser failure.
    /// Returning None preserves the complete original diagnostic unchanged.
    pub(crate) fn minimum_version_error(&self, module_name: &PklString) -> Option<RuntimeError> {
        let declaration = self.completed_declaration.as_ref()?;
        for annotation in &declaration.annotations {
            let ExprKind::Annotation { ty, members, .. } = &annotation.kind else {
                continue;
            };
            let TypeKind::Named { path, .. } = &ty.kind else {
                continue;
            };
            if ty.parenthesized || !path.last().is_some_and(|name| name == "ModuleInfo") {
                continue;
            }
            for member in members {
                let ObjectMember::Property(member) = member else {
                    continue;
                };
                // Object methods share Member storage but retain their authored
                // production flag; a property whose value is a lambda does not.
                if matches!(&member.expr.kind, ExprKind::Function { method: true, .. })
                    || member.name != "minPklVersion"
                {
                    continue;
                }
                // An amendment body has no ObjectProperty.getExpr() upstream.
                // Its lowered synthetic super read is never a version value.
                if member.amendment_body_span.is_some() {
                    return None;
                }
                let Some(expression) = self.source.slice(member.expr.span) else {
                    return Some(RuntimeError::new(
                        ErrorKind::InvalidBytecode,
                        "Partial module expression span is outside its original source.",
                    ));
                };
                let units: Vec<_> = expression.to_pkl_string().encode_utf16().collect();
                if units.len() < 2 {
                    // Java substring(1, length - 1) escapes its version parser's
                    // IllegalArgumentException catch. Its API exception model
                    // is still required/unimplemented, never an ordinary pass.
                    return Some(RuntimeError::new(
                        ErrorKind::Unsupported,
                        "The Java partial ModuleInfo checker fails internally for this expression; its API exception contract is unresolved.",
                    ));
                }
                let required = RequiredVersion::parse(&units[1..units.len() - 1])?;
                return required.requires_newer_release().then(|| {
                    let mut message = PklStringBuilder::default();
                    message.push_str("Module `");
                    message.push_value(module_name);
                    message.push_str("` requires Pkl version ");
                    message.push_value(&required.display());
                    message.push_str(" or higher, but your Pkl version is 0.32.0.");
                    RuntimeError::new(ErrorKind::Type, message.finish())
                });
            }
        }
        None
    }
}

/// Version.parse syntax and formatting, restricted to comparison with the
/// pinned current release's toNormal() value. No evaluated ModuleInfo constraint
/// or general-purpose semver library is substituted for the pinned regex.
struct RequiredVersion {
    core: [i32; 3],
    suffix: PklString,
}

impl RequiredVersion {
    fn parse(units: &[u16]) -> Option<Self> {
        fn component(units: &[u16], cursor: &mut usize) -> Option<i32> {
            let start = *cursor;
            let mut result = 0_i32;
            while let Some(&unit) = units.get(*cursor) {
                if !(48..=57).contains(&unit) {
                    break;
                }
                result = result.checked_mul(10)?.checked_add(i32::from(unit - 48))?;
                *cursor += 1;
            }
            (*cursor > start).then_some(result)
        }

        fn build_is_valid(units: &[u16]) -> bool {
            // Java Pattern's unflagged dot excludes these line terminators.
            !units.is_empty()
                && !units
                    .iter()
                    .any(|unit| matches!(unit, 10 | 13 | 0x85 | 0x2028 | 0x2029))
        }

        let mut cursor = 0;
        let mut core = [0; 3];
        for (index, value) in core.iter_mut().enumerate() {
            *value = component(units, &mut cursor)?;
            if index < 2 {
                if units.get(cursor) != Some(&46) {
                    return None;
                }
                cursor += 1;
            }
        }
        let suffix = &units[cursor..];
        match suffix.first().copied() {
            None => (),
            Some(45) => {
                // [^+]+ accepts line terminators; the optional build's .+ does
                // not. Plus signs inside build metadata remain ordinary text.
                let end = suffix[1..]
                    .iter()
                    .position(|unit| *unit == 43)
                    .map_or(suffix.len(), |index| index + 1);
                if end == 1 || (end < suffix.len() && !build_is_valid(&suffix[end + 1..])) {
                    return None;
                }
            }
            Some(43) if build_is_valid(&suffix[1..]) => (),
            _ => return None,
        }
        Some(Self {
            core,
            suffix: PklString::from_utf16(suffix.to_vec()),
        })
    }

    fn requires_newer_release(&self) -> bool {
        // Current 0.32.0 is normalized and therefore has no prerelease. Equal
        // core components compare >= any required prerelease; build is ignored.
        // Version's prerelease-identifier comparator is never reached here.
        self.core > [0, 32, 0]
    }

    fn display(&self) -> PklString {
        let mut text = PklStringBuilder::default();
        text.push_str(&format!(
            "{}.{}.{}",
            self.core[0], self.core[1], self.core[2]
        ));
        text.push_value(&self.suffix);
        text.finish()
    }
}

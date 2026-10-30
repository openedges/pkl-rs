//! Resumable GlobResolver traversal. Host responses contain names and flags,
//! never matching decisions or evaluated Pkl values.
use super::*;
use crate::glob::PathElement;
use crate::host::resources::glob as uri;
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub(super) struct Element {
    pub path: PklString,
    pub uri: PklString,
    pub directory: bool,
}

pub(super) enum Poll {
    Continue,
    FragmentPaths,
    HasElement(PklString),
    ListElements(PklString),
    Done(Vec<Element>),
}

struct Resolve {
    part: usize,
    uri: PklString,
    path: PklString,
}

struct Scan {
    uri: PklString,
    path: PklString,
    elements: Option<Vec<PathElement>>,
    next: usize,
}

struct Expansion {
    part: usize,
    pattern: super::glob_pattern::Pattern,
    globstar: bool,
    frames: Vec<Scan>,
    matched: Vec<Element>,
}

struct Opaque {
    uri: PklString,
    pattern: super::glob_pattern::Pattern,
    elements: Option<Vec<PathElement>>,
    next: usize,
}

enum ResolveAction {
    Initial,
    Matched {
        path: PklString,
        directory: bool,
        recurse: bool,
    },
    Recurse {
        path: PklString,
    },
    Regular {
        path: PklString,
        next_part: Option<usize>,
    },
}

struct ReaderResolve {
    base: PklString,
    relative: PklString,
    action: ResolveAction,
}

enum Pending {
    Resolve(ReaderResolve),
    Has(Element),
    HierarchicalList,
    OpaqueList,
}

pub(super) struct Traversal {
    authored: PklString,
    parts: Vec<PklString>,
    absolute: bool,
    reader_resolve: Option<ReaderResolve>,
    tasks: Vec<Resolve>,
    expansion: Option<Expansion>,
    opaque: Option<Opaque>,
    pending: Option<Pending>,
    initial: Option<Element>,
    result: Vec<Element>,
    positions: BTreeMap<PklString, usize>,
    list_calls: u32,
    list_threshold: u32,
}

impl Traversal {
    /// The caller resolves the base through the original declaration ModuleKey
    /// before constructing a hierarchical traversal. `base_path` is the exact
    /// unnormalized mapping-key prefix from splitGlobPatternIntoBaseAndWildcards.
    pub(super) fn hierarchical(
        authored: PklString,
        base_uri: PklString,
        base_path: PklString,
        parts: Vec<PklString>,
        initial_base: Option<PklString>,
        list_threshold: u32,
    ) -> Result<Self, RuntimeError> {
        let absolute = uri::absolute_pattern(&authored);
        let initial = if parts.is_empty() && initial_base.is_none() {
            Some(Element {
                path: authored.clone(),
                uri: base_uri.clone(),
                directory: true,
            })
        } else {
            None
        };
        let tasks = if !parts.is_empty() {
            vec![Resolve {
                part: 0,
                uri: base_uri,
                path: base_path,
            }]
        } else {
            Vec::new()
        };
        let reader_resolve = initial_base.map(|base| ReaderResolve {
            base,
            relative: authored.clone(),
            action: ResolveAction::Initial,
        });
        Ok(Self {
            authored,
            parts,
            absolute,
            reader_resolve,
            tasks,
            initial,
            expansion: None,
            opaque: None,
            pending: None,
            result: Vec::new(),
            positions: BTreeMap::new(),
            list_calls: 0,
            list_threshold,
        })
    }

    pub(super) fn opaque(authored: PklString, list_uri: PklString) -> Result<Self, RuntimeError> {
        let pattern = super::glob_pattern::Pattern::compile(&authored, &authored)?;
        uri::validate_opaque_list_uri(&list_uri)?;
        Ok(Self {
            authored,
            parts: Vec::new(),
            absolute: false,
            reader_resolve: None,
            tasks: Vec::new(),
            expansion: None,
            opaque: Some(Opaque {
                uri: list_uri,
                pattern,
                elements: None,
                next: 0,
            }),
            pending: None,
            initial: None,
            result: Vec::new(),
            positions: BTreeMap::new(),
            list_calls: 0,
            list_threshold: 0,
        })
    }

    fn insert(&mut self, element: Element) -> Result<(), RuntimeError> {
        if let Some(&position) = self.positions.get(&element.path) {
            self.result[position] = element;
        } else {
            self.positions
                .insert(element.path.clone(), self.result.len());
            self.result.push(element);
        }
        Ok(())
    }

    /// One bounded transition. VM fuel is charged between transitions. A Host
    /// response is consumed exactly once before the next transition can run.
    pub(super) fn poll(&mut self) -> Result<Poll, RuntimeError> {
        if self.pending.is_some() {
            return Err(invalid(
                "glob traversal polled with an outstanding Host request",
            ));
        }
        if let Some(resolve) = self.reader_resolve.take() {
            self.pending = Some(Pending::Resolve(resolve));
            return Ok(Poll::FragmentPaths);
        }
        if let Some(element) = self.initial.take() {
            let requested = element.uri.clone();
            self.pending = Some(Pending::Has(element));
            return Ok(Poll::HasElement(requested));
        }
        if let Some(opaque) = &mut self.opaque {
            let Some(elements) = &opaque.elements else {
                self.pending = Some(Pending::OpaqueList);
                return Ok(Poll::ListElements(opaque.uri.clone()));
            };
            let Some(element) = elements.get(opaque.next) else {
                self.opaque = None;
                return Ok(Poll::Continue);
            };
            opaque.next += 1;
            let scheme = uri::scheme(&opaque.uri).ok_or_else(|| invalid("opaque glob scheme"))?;
            let resolved = uri::opaque_element(&scheme, &element.name)?;
            let path = resolved.clone();
            if opaque.pattern.matches(&path)? {
                self.insert(Element {
                    path,
                    uri: resolved,
                    directory: false,
                })?;
            }
            return Ok(Poll::Continue);
        }
        if let Some(expansion) = &mut self.expansion {
            let Some(frame) = expansion.frames.last_mut() else {
                let expansion = self.expansion.take().unwrap();
                if expansion.part + 1 == self.parts.len() {
                    for element in expansion.matched {
                        self.insert(element)?;
                    }
                } else {
                    // GlobResolver fully expands the current part before it
                    // resolves the next segment for each matched directory.
                    for element in expansion.matched.into_iter().rev() {
                        if element.directory {
                            self.tasks.push(Resolve {
                                part: expansion.part + 1,
                                uri: element.uri,
                                path: element.path,
                            });
                        }
                    }
                }
                return Ok(Poll::Continue);
            };
            let Some(elements) = &frame.elements else {
                // Preserve Java getAndIncrement() > threshold, including the
                // threshold+1 successful calls before the failing attempt.
                let previous = self.list_calls;
                self.list_calls = self.list_calls.saturating_add(1);
                if previous > self.list_threshold {
                    return Err(glob::invalid_pattern(
                        &self.authored,
                        "The glob pattern is too complex.".into(),
                    ));
                }
                self.pending = Some(Pending::HierarchicalList);
                return Ok(Poll::ListElements(frame.uri.clone()));
            };
            let Some(element) = elements.get(frame.next) else {
                expansion.frames.pop();
                return Ok(Poll::Continue);
            };
            let element = element.clone();
            frame.next += 1;
            let matched = expansion.pattern.matches(&element.name)?;
            let recurse = element.is_directory && expansion.globstar;
            if matched || recurse {
                let path = uri::path(&frame.path, &element.name, self.absolute);
                let relative = if element.is_directory {
                    element.name.concat(&"/".into())
                } else {
                    element.name.clone()
                };
                self.reader_resolve = Some(ReaderResolve {
                    base: frame.uri.clone(),
                    relative,
                    action: if matched {
                        ResolveAction::Matched {
                            path,
                            directory: element.is_directory,
                            recurse,
                        }
                    } else {
                        ResolveAction::Recurse { path }
                    },
                });
            }
            return Ok(Poll::Continue);
        }
        let Some(task) = self.tasks.pop() else {
            return Ok(Poll::Done(std::mem::take(&mut self.result)));
        };
        let part = &self.parts[task.part];
        if super::glob_pattern::regular(part) {
            let path = uri::path(&task.path, part, self.absolute);
            let leaf = task.part + 1 == self.parts.len();
            self.reader_resolve = Some(ReaderResolve {
                base: task.uri,
                relative: if leaf {
                    part.clone()
                } else {
                    part.concat(&"/".into())
                },
                action: ResolveAction::Regular {
                    path,
                    next_part: (!leaf).then_some(task.part + 1),
                },
            });
            Ok(Poll::Continue)
        } else {
            let pattern = super::glob_pattern::Pattern::compile(part, &self.authored)?;
            self.expansion = Some(Expansion {
                part: task.part,
                pattern,
                globstar: part.contains("**"),
                frames: vec![Scan {
                    uri: task.uri,
                    path: task.path,
                    elements: None,
                    next: 0,
                }],
                matched: Vec::new(),
            });
            Ok(Poll::Continue)
        }
    }

    /// IoUtils.resolve queries the target reader each time, including separate
    /// matched-directory and recursive-directory resolutions. The getter may
    /// fail, so neither the result nor its position in the effect order is cached.
    pub(super) fn complete_fragments(&mut self, fragments: bool) -> Result<(), RuntimeError> {
        let Some(Pending::Resolve(resolve)) = self.pending.take() else {
            return Err(invalid(
                "glob URI resolution has no matching fragment request",
            ));
        };
        let resolved: PklString = uri::resolve(&resolve.base, &resolve.relative, fragments)?;
        match resolve.action {
            ResolveAction::Initial => {
                self.initial = Some(Element {
                    path: self.authored.clone(),
                    uri: resolved,
                    directory: true,
                });
            }
            ResolveAction::Regular { path, next_part } => {
                if let Some(part) = next_part {
                    self.tasks.push(Resolve {
                        part,
                        uri: resolved,
                        path,
                    });
                } else {
                    self.initial = Some(Element {
                        path,
                        uri: resolved,
                        directory: false,
                    });
                }
            }
            ResolveAction::Matched {
                path,
                directory,
                recurse,
            } => {
                self.expansion
                    .as_mut()
                    .ok_or_else(|| invalid("glob matched continuation"))?
                    .matched
                    .push(Element {
                        path: path.clone(),
                        uri: resolved,
                        directory,
                    });
                if recurse {
                    self.reader_resolve = Some(ReaderResolve {
                        base: resolve.base,
                        relative: resolve.relative,
                        action: ResolveAction::Recurse { path },
                    });
                }
            }
            ResolveAction::Recurse { path } => {
                self.expansion
                    .as_mut()
                    .ok_or_else(|| invalid("glob recursion continuation"))?
                    .frames
                    .push(Scan {
                        uri: resolved,
                        path,
                        elements: None,
                        next: 0,
                    });
            }
        }
        Ok(())
    }

    pub(super) fn complete_has(&mut self, present: bool) -> Result<(), RuntimeError> {
        if !matches!(self.pending, Some(Pending::Has(_))) {
            return Err(invalid(
                "glob hasElement completion has no matching request",
            ));
        }
        let Some(Pending::Has(element)) = self.pending.take() else {
            unreachable!()
        };
        if present {
            self.insert(element)?;
        }
        Ok(())
    }

    pub(super) fn complete_list(
        &mut self,
        mut elements: Vec<PathElement>,
    ) -> Result<(), RuntimeError> {
        if !matches!(
            self.pending,
            Some(Pending::HierarchicalList | Pending::OpaqueList)
        ) {
            return Err(invalid(
                "glob listElements completion has no matching request",
            ));
        }
        // Stable Java stream sorting: files first, then directory flag ties
        // broken by String.compareTo's UTF-16 unit order. Never normalize names.
        elements.sort_by(|a, b| {
            a.is_directory
                .cmp(&b.is_directory)
                .then_with(|| a.name.cmp(&b.name))
        });
        match self.pending.take().unwrap() {
            Pending::HierarchicalList => {
                let frame = self
                    .expansion
                    .as_mut()
                    .and_then(|run| run.frames.last_mut())
                    .ok_or_else(|| invalid("missing glob directory continuation"))?;
                frame.elements = Some(elements);
            }
            Pending::OpaqueList => {
                self.opaque
                    .as_mut()
                    .ok_or_else(|| invalid("missing opaque glob continuation"))?
                    .elements = Some(elements);
            }
            Pending::Has(_) | Pending::Resolve(_) => unreachable!(),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opaque_raw_uri_failure_precedes_listing_and_preserves_cause() {
        // Captured Java 0.32.0 oracle: Illegal character in opaque part at
        // index 6: gdata:\*. Repeating construction must not leave a request.
        for _ in 0..2 {
            let error = match Traversal::opaque("gdata:\\*".into(), "gdata:\\*".into()) {
                Ok(_) => panic!("invalid URI reached traversal"),
                Err(error) => error,
            };
            assert_eq!(error.kind, ErrorKind::Unsupported);
            let cause = error.uri_create_cause.as_ref().unwrap();
            assert_eq!(cause.input, "gdata:\\*");
            assert_eq!(cause.reason, "Illegal character in opaque part");
            assert_eq!(cause.index_utf16, Some(6));
        }
    }

    #[test]
    fn opaque_regex_failure_precedes_raw_uri_validation() {
        let error = match Traversal::opaque("gdata:\\".into(), "gdata:\\".into()) {
            Ok(_) => panic!("invalid glob reached traversal"),
            Err(error) => error,
        };
        assert!(error.uri_create_cause.is_none());
        assert_eq!(error.kind, ErrorKind::Type);
    }

    #[test]
    fn opaque_uri_cause_keeps_utf16_input_and_index() {
        let pattern = PklString::from_utf16(vec![
            103, 100, 97, 116, 97, 58, 0xd83d, 0xde00, 0xd800, 92, 42,
        ]);
        let error = match Traversal::opaque(pattern.clone(), pattern.clone()) {
            Ok(_) => panic!("invalid URI reached traversal"),
            Err(error) => error,
        };
        let cause = error.uri_create_cause.as_ref().unwrap();
        assert_eq!(
            cause.input.encode_utf16().collect::<Vec<_>>(),
            pattern.encode_utf16().collect::<Vec<_>>()
        );
        assert_eq!(cause.reason, "Illegal character in opaque part");
        assert_eq!(cause.index_utf16, Some(9));
    }

    #[test]
    fn opaque_relative_dummy_and_lossless_valid_patterns_reach_listing() {
        let lone_surrogate = PklString::from_utf16(vec![103, 100, 97, 116, 97, 58, 0xd800, 42]);
        for (pattern, list_uri) in [
            (PklString::from("\\*"), PklString::from("gdata:dummy")),
            (PklString::from("gdata:é*"), PklString::from("gdata:é*")),
            (lone_surrogate.clone(), lone_surrogate),
        ] {
            let mut traversal = Traversal::opaque(pattern, list_uri.clone()).unwrap();
            assert!(
                matches!(traversal.poll().unwrap(), Poll::ListElements(uri) if uri == list_uri)
            );
        }
    }
}

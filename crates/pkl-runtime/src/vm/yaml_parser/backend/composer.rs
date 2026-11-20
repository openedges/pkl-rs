// Copyright (c) 2018, SnakeYAML. Apache-2.0. See PROVENANCE.md.
//! Document-at-a-time composition with identity-preserving alias edges and
//! cooperative step-budgeted yielding across scanning, parsing, composition,
//! and explicit merge expansion.
use super::diagnostic::ErrorKind;
use super::nodes::tag;
use super::parser::{Event, EventId, Parser};
use super::{Document, Node, NodeId, NodeKind, Settings, Text, YamlError};
use std::collections::{BTreeMap, BTreeSet};

/// Outcome of one cooperative step in [`Backend::step`].
#[derive(Debug)]
pub enum StepOutcome {
    /// Budget exhausted mid-document or mid-merge; call [`Backend::step`] again to resume.
    Yielded,
    /// Document composition completed (`None` at end of stream).
    Document(Option<Document>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Consumption {
    Fresh,
    Stream,
    Single,
    Finished,
}

struct Collection {
    id: NodeId,
    key: Option<NodeId>,
}

struct ComposeState {
    nodes: Vec<Node>,
    anchors: BTreeMap<Text, NodeId>,
    recursive: BTreeSet<NodeId>,
    merges: BTreeSet<NodeId>,
    aliases: usize,
    stack: Vec<Collection>,
}

impl ComposeState {
    fn new() -> Self {
        Self {
            nodes: Vec::new(),
            anchors: BTreeMap::new(),
            recursive: BTreeSet::new(),
            merges: BTreeSet::new(),
            aliases: 0,
            stack: Vec::new(),
        }
    }
}

struct FlattenState {
    target_id: NodeId,
    to_process: Vec<(NodeId, NodeId)>,
    seen: BTreeSet<Vec<(NodeId, NodeId)>>,
    started_round: bool,
    process: bool,
    updated: Vec<(NodeId, NodeId)>,
    keys: BTreeSet<Text>,
    merge_tuples: Vec<(NodeId, NodeId)>,
    merge_idx: usize,
    refs: Vec<NodeId>,
    ref_idx: usize,
}

impl FlattenState {
    fn new(target_id: NodeId, entries: Vec<(NodeId, NodeId)>) -> Self {
        Self {
            target_id,
            to_process: entries,
            seen: BTreeSet::new(),
            started_round: false,
            process: false,
            updated: Vec::new(),
            keys: BTreeSet::new(),
            merge_tuples: Vec::new(),
            merge_idx: 0,
            refs: Vec::new(),
            ref_idx: 0,
        }
    }
}

enum ComposePhase {
    Idle,
    StartSingle,
    StartSingleCheck,
    StartStream,
    StartStreamCheck,
    ExpectDocStart {
        single: bool,
    },
    Composing {
        single: bool,
        state: ComposeState,
    },
    Flattening {
        single: bool,
        state: ComposeState,
        flatten: FlattenState,
    },
    ExpectDocEnd {
        single: bool,
        root: NodeId,
        nodes: Vec<Node>,
    },
    VerifySingleEnd {
        document: Option<Document>,
    },
    ConsumeSingleEnd {
        document: Option<Document>,
    },
}

/// Pure backend. Supports both synchronous convenience calls (`single_document` /
/// `next_document`) and cooperative budget-bounded stepping (`start_single_document` /
/// `start_next_document` + `step`).
pub struct Backend {
    parser: Parser,
    settings: Settings,
    consumption: Consumption,
    phase: ComposePhase,
}

impl Backend {
    pub fn new(input: Text, settings: Settings) -> Self {
        Self {
            parser: Parser::new(input, settings.label.clone()),
            settings,
            consumption: Consumption::Fresh,
            phase: ComposePhase::Idle,
        }
    }

    pub fn start_single_document(&mut self) -> Result<(), YamlError> {
        if self.consumption != Consumption::Fresh || !matches!(self.phase, ComposePhase::Idle) {
            return Err(YamlError::plain(
                ErrorKind::Unsupported,
                "single_document requires a fresh backend",
            ));
        }
        self.consumption = Consumption::Single;
        self.phase = ComposePhase::StartSingle;
        Ok(())
    }

    pub fn start_next_document(&mut self) -> Result<(), YamlError> {
        if !matches!(self.phase, ComposePhase::Idle) {
            return Err(YamlError::plain(
                ErrorKind::Unsupported,
                "backend already has an in-progress document",
            ));
        }
        if self.consumption == Consumption::Finished {
            return Ok(());
        }
        if self.consumption == Consumption::Single {
            return Err(YamlError::plain(
                ErrorKind::Unsupported,
                "cannot resume single-document backend as a stream",
            ));
        }
        if self.consumption == Consumption::Fresh {
            self.consumption = Consumption::Stream;
            self.phase = ComposePhase::StartStream;
        } else {
            self.phase = ComposePhase::StartStreamCheck;
        }
        Ok(())
    }

    /// Advances scanning, parsing, composition, and explicit merge expansion
    /// up to `budget` cooperative work units.
    pub fn step(&mut self, budget: usize) -> Result<StepOutcome, YamlError> {
        let mut remaining = budget.max(1);
        loop {
            if matches!(self.phase, ComposePhase::Idle) {
                return Ok(StepOutcome::Document(None));
            }
            if remaining == 0 {
                return Ok(StepOutcome::Yielded);
            }
            let phase = std::mem::replace(&mut self.phase, ComposePhase::Idle);
            match phase {
                ComposePhase::Idle => return Ok(StepOutcome::Document(None)),
                ComposePhase::StartSingle => {
                    if self.parser.step_next(&mut remaining)?.is_none() {
                        self.phase = ComposePhase::StartSingle;
                        return Ok(StepOutcome::Yielded);
                    }
                    self.phase = ComposePhase::StartSingleCheck;
                }
                ComposePhase::StartSingleCheck => {
                    let Some(is_end) =
                        self.parser.step_check(EventId::StreamEnd, &mut remaining)?
                    else {
                        self.phase = ComposePhase::StartSingleCheck;
                        return Ok(StepOutcome::Yielded);
                    };
                    if is_end {
                        self.phase = ComposePhase::VerifySingleEnd { document: None };
                    } else {
                        self.phase = ComposePhase::ExpectDocStart { single: true };
                    }
                }
                ComposePhase::StartStream => {
                    if self.parser.step_next(&mut remaining)?.is_none() {
                        self.phase = ComposePhase::StartStream;
                        return Ok(StepOutcome::Yielded);
                    }
                    self.phase = ComposePhase::StartStreamCheck;
                }
                ComposePhase::StartStreamCheck => {
                    let Some(is_end) =
                        self.parser.step_check(EventId::StreamEnd, &mut remaining)?
                    else {
                        self.phase = ComposePhase::StartStreamCheck;
                        return Ok(StepOutcome::Yielded);
                    };
                    if is_end {
                        self.consumption = Consumption::Finished;
                        self.phase = ComposePhase::Idle;
                        return Ok(StepOutcome::Document(None));
                    }
                    self.phase = ComposePhase::ExpectDocStart { single: false };
                }
                ComposePhase::ExpectDocStart { single } => {
                    let Some(start) = self.parser.step_next(&mut remaining)? else {
                        self.phase = ComposePhase::ExpectDocStart { single };
                        return Ok(StepOutcome::Yielded);
                    };
                    if start.id != EventId::DocumentStart {
                        return Err(YamlError::plain(
                            ErrorKind::Engine,
                            "Expected document start event",
                        ));
                    }
                    self.phase = ComposePhase::Composing {
                        single,
                        state: ComposeState::new(),
                    };
                }
                ComposePhase::Composing { single, mut state } => {
                    let Some(event) = self.parser.step_peek(&mut remaining)? else {
                        self.phase = ComposePhase::Composing { single, state };
                        return Ok(StepOutcome::Yielded);
                    };
                    if remaining == 0 {
                        self.phase = ComposePhase::Composing { single, state };
                        return Ok(StepOutcome::Yielded);
                    }
                    remaining -= 1;

                    if matches!(event.id, EventId::SequenceEnd | EventId::MappingEnd) {
                        let event = self.parser.take_peeked();
                        let frame = state.stack.pop().expect("collection end has start");
                        state.nodes[frame.id.0].end = event.end;
                        if state.merges.contains(&frame.id) {
                            let NodeKind::Mapping { entries, .. } = &state.nodes[frame.id.0].kind
                            else {
                                unreachable!()
                            };
                            let flatten = FlattenState::new(frame.id, entries.clone());
                            self.phase = ComposePhase::Flattening {
                                single,
                                state,
                                flatten,
                            };
                            continue;
                        }
                        if let Some(parent) = state.stack.last_mut() {
                            attach(&mut state.nodes, parent, frame.id, &mut state.merges);
                            state.recursive.remove(&parent.id);
                            self.phase = ComposePhase::Composing { single, state };
                        } else {
                            self.phase = ComposePhase::ExpectDocEnd {
                                single,
                                root: frame.id,
                                nodes: state.nodes,
                            };
                        }
                        continue;
                    }

                    if let Some(parent) = state.stack.last() {
                        state.recursive.insert(parent.id);
                    }
                    let event = self.parser.take_peeked();
                    let id = if event.id == EventId::Alias {
                        let anchor = event.anchor.as_ref().expect("alias anchor");
                        let Some(&id) = state.anchors.get(anchor) else {
                            let mut problem =
                                "found undefined alias ".encode_utf16().collect::<Vec<_>>();
                            problem.extend(anchor.units());
                            return Err(YamlError::marked(
                                ErrorKind::Composer,
                                None,
                                None,
                                Text::from_units(problem),
                                Some(event.start),
                            ));
                        };
                        if !matches!(state.nodes[id.0].kind, NodeKind::Scalar { .. }) {
                            state.aliases += 1;
                            if state.aliases > self.settings.max_collection_aliases {
                                return Err(YamlError::plain(
                                    ErrorKind::Engine,
                                    format!(
                                        "Number of aliases for non-scalar nodes exceeds the specified max={}",
                                        self.settings.max_collection_aliases
                                    ),
                                ));
                            }
                        }
                        if state.recursive.remove(&id) {
                            state.nodes[id.0].recursive = true;
                        }
                        id
                    } else {
                        let id = NodeId(state.nodes.len());
                        let node = make_node(event, &self.settings)?;
                        if let Some(anchor) = &node.anchor {
                            state.anchors.insert(anchor.clone(), id);
                        }
                        let collection = !matches!(node.kind, NodeKind::Scalar { .. });
                        state.nodes.push(node);
                        if collection {
                            state.stack.push(Collection { id, key: None });
                            self.phase = ComposePhase::Composing { single, state };
                            continue;
                        }
                        id
                    };

                    if let Some(parent) = state.stack.last_mut() {
                        attach(&mut state.nodes, parent, id, &mut state.merges);
                        state.recursive.remove(&parent.id);
                        self.phase = ComposePhase::Composing { single, state };
                    } else {
                        self.phase = ComposePhase::ExpectDocEnd {
                            single,
                            root: id,
                            nodes: state.nodes,
                        };
                    }
                }
                ComposePhase::Flattening {
                    single,
                    mut state,
                    mut flatten,
                } => {
                    remaining -= 1;
                    if !flatten.started_round {
                        if !flatten.seen.insert(flatten.to_process.clone()) {
                            return Err(YamlError::plain(
                                ErrorKind::Unsupported,
                                "Nonterminating explicit merge expansion requires the pinned Java lifecycle outcome",
                            ));
                        }
                        flatten.process = false;
                        flatten.updated.clear();
                        flatten.keys.clear();
                        flatten.merge_tuples.clear();
                        flatten.merge_idx = 0;
                        flatten.refs.clear();
                        flatten.ref_idx = 0;
                        for &(key, value) in &flatten.to_process {
                            if state.nodes[key.0].tag == tag("merge") {
                                flatten.merge_tuples.push((key, value));
                            } else {
                                flatten.updated.push((key, value));
                                if let NodeKind::Scalar { value, .. } = &state.nodes[key.0].kind {
                                    flatten.keys.insert(value.clone());
                                }
                            }
                        }
                        flatten.started_round = true;
                        self.phase = ComposePhase::Flattening {
                            single,
                            state,
                            flatten,
                        };
                    } else if flatten.ref_idx < flatten.refs.len() {
                        let reference = flatten.refs[flatten.ref_idx];
                        let Some(mapped) = self.step_mapping_id(
                            reference,
                            &state.nodes,
                            &state.anchors,
                            &mut remaining,
                        )?
                        else {
                            self.phase = ComposePhase::Flattening {
                                single,
                                state,
                                flatten,
                            };
                            return Ok(StepOutcome::Yielded);
                        };
                        flatten.ref_idx += 1;
                        flatten.process |= state.merges.contains(&mapped);
                        let NodeKind::Mapping { entries, .. } = &state.nodes[mapped.0].kind else {
                            unreachable!()
                        };
                        let mut found = BTreeSet::new();
                        for &(key, value) in entries {
                            if let NodeKind::Scalar { value: scalar, .. } = &state.nodes[key.0].kind
                            {
                                if !flatten.keys.contains(scalar) {
                                    flatten.updated.push((key, value));
                                    found.insert(scalar.clone());
                                }
                            } else {
                                flatten.updated.push((key, value));
                            }
                        }
                        flatten.keys.extend(found);
                        self.phase = ComposePhase::Flattening {
                            single,
                            state,
                            flatten,
                        };
                    } else if flatten.merge_idx < flatten.merge_tuples.len() {
                        let (_, value) = flatten.merge_tuples[flatten.merge_idx];
                        flatten.merge_idx += 1;
                        flatten.refs = match &state.nodes[value.0].kind {
                            NodeKind::Sequence { children, .. } => children.clone(),
                            _ => vec![value],
                        };
                        flatten.ref_idx = 0;
                        self.phase = ComposePhase::Flattening {
                            single,
                            state,
                            flatten,
                        };
                    } else if flatten.process {
                        flatten.to_process = std::mem::take(&mut flatten.updated);
                        flatten.started_round = false;
                        self.phase = ComposePhase::Flattening {
                            single,
                            state,
                            flatten,
                        };
                    } else {
                        let target_id = flatten.target_id;
                        let NodeKind::Mapping { entries, .. } = &mut state.nodes[target_id.0].kind
                        else {
                            unreachable!()
                        };
                        *entries = flatten.updated;
                        state.merges.remove(&target_id);
                        if let Some(parent) = state.stack.last_mut() {
                            attach(&mut state.nodes, parent, target_id, &mut state.merges);
                            state.recursive.remove(&parent.id);
                            self.phase = ComposePhase::Composing { single, state };
                        } else {
                            self.phase = ComposePhase::ExpectDocEnd {
                                single,
                                root: target_id,
                                nodes: state.nodes,
                            };
                        }
                    }
                }
                ComposePhase::ExpectDocEnd {
                    single,
                    root,
                    nodes,
                } => {
                    let Some(end) = self.parser.step_next(&mut remaining)? else {
                        self.phase = ComposePhase::ExpectDocEnd {
                            single,
                            root,
                            nodes,
                        };
                        return Ok(StepOutcome::Yielded);
                    };
                    if end.id != EventId::DocumentEnd {
                        return Err(YamlError::plain(
                            ErrorKind::Engine,
                            "Expected document end event",
                        ));
                    }
                    let document = Document { root, nodes };
                    if single {
                        self.phase = ComposePhase::VerifySingleEnd {
                            document: Some(document),
                        };
                    } else {
                        self.phase = ComposePhase::Idle;
                        return Ok(StepOutcome::Document(Some(document)));
                    }
                }
                ComposePhase::VerifySingleEnd { document } => {
                    let Some(is_end) =
                        self.parser.step_check(EventId::StreamEnd, &mut remaining)?
                    else {
                        self.phase = ComposePhase::VerifySingleEnd { document };
                        return Ok(StepOutcome::Yielded);
                    };
                    if !is_end {
                        let event = self.parser.take_peeked();
                        let previous = document.as_ref().map(|d| d.node(d.root()).start.clone());
                        return Err(YamlError::marked(
                            ErrorKind::Composer,
                            Some("expected a single document in the stream"),
                            previous,
                            "but found another document",
                            Some(event.start),
                        ));
                    }
                    self.phase = ComposePhase::ConsumeSingleEnd { document };
                }
                ComposePhase::ConsumeSingleEnd { document } => {
                    if self.parser.step_next(&mut remaining)?.is_none() {
                        self.phase = ComposePhase::ConsumeSingleEnd { document };
                        return Ok(StepOutcome::Yielded);
                    }
                    self.consumption = Consumption::Finished;
                    self.phase = ComposePhase::Idle;
                    return Ok(StepOutcome::Document(document));
                }
            }
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub fn single_document(&mut self) -> Result<Option<Document>, YamlError> {
        self.start_single_document()?;
        loop {
            if let StepOutcome::Document(doc) = self.step(usize::MAX)? {
                return Ok(doc);
            }
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub fn next_document(&mut self) -> Result<Option<Document>, YamlError> {
        self.start_next_document()?;
        loop {
            if let StepOutcome::Document(doc) = self.step(usize::MAX)? {
                return Ok(doc);
            }
        }
    }

    fn step_mapping_id(
        &mut self,
        id: NodeId,
        nodes: &[Node],
        anchors: &BTreeMap<Text, NodeId>,
        budget: &mut usize,
    ) -> Result<Option<NodeId>, YamlError> {
        if matches!(nodes[id.0].kind, NodeKind::Mapping { .. }) {
            return Ok(Some(id));
        }
        if let Some(anchor) = &nodes[id.0].anchor
            && let Some(&other) = anchors.get(anchor)
            && matches!(nodes[other.0].kind, NodeKind::Mapping { .. })
        {
            return Ok(Some(other));
        }
        let Some(event) = self.parser.step_peek(budget)? else {
            return Ok(None);
        };
        Err(YamlError::marked(
            ErrorKind::Composer,
            None,
            None,
            "Expected mapping node or an anchor referencing mapping",
            Some(event.start),
        ))
    }
}

fn attach(
    nodes: &mut [Node],
    parent: &mut Collection,
    child: NodeId,
    merges: &mut BTreeSet<NodeId>,
) {
    let merge = nodes[child.0].tag == tag("merge");
    match &mut nodes[parent.id.0].kind {
        NodeKind::Sequence { children, .. } => children.push(child),
        NodeKind::Mapping { entries, .. } => {
            if let Some(key) = parent.key.take() {
                entries.push((key, child));
            } else {
                parent.key = Some(child);
                if merge {
                    merges.insert(parent.id);
                }
            }
        }
        NodeKind::Scalar { .. } => unreachable!(),
    }
}

fn make_node(event: Event, settings: &Settings) -> Result<Node, YamlError> {
    let resolve = event.tag.as_ref().is_none_or(|t| t.is_ascii("!"));
    let tag_value = if resolve {
        match event.id {
            EventId::Scalar => super::resolve::resolve(&event.value, event.implicit, settings.mode),
            EventId::SequenceStart => tag("seq"),
            EventId::MappingStart => tag("map"),
            _ => return Err(YamlError::plain(ErrorKind::Engine, "Expected node event")),
        }
    } else {
        encode_tag(event.tag.as_ref().unwrap())?
    };
    let kind = match event.id {
        EventId::Scalar => NodeKind::Scalar {
            value: event.value,
            style: event.style,
        },
        EventId::SequenceStart => NodeKind::Sequence {
            children: Vec::new(),
            flow: event.flow,
        },
        EventId::MappingStart => NodeKind::Mapping {
            entries: Vec::new(),
            flow: event.flow,
        },
        _ => return Err(YamlError::plain(ErrorKind::Engine, "Expected node event")),
    };
    Ok(Node {
        tag: tag_value,
        resolved: resolve,
        kind,
        start: event.start,
        end: event.end,
        anchor: event.anchor,
        recursive: false,
    })
}

fn encode_tag(value: &Text) -> Result<Text, YamlError> {
    if value.units().is_empty() {
        return Err(YamlError::foreign(
            "java.lang.IllegalArgumentException",
            "Tag must not be empty.",
        ));
    }
    if value.units().first().is_some_and(|u| *u <= 32)
        || value.units().last().is_some_and(|u| *u <= 32)
    {
        return Err(YamlError::foreign(
            "java.lang.IllegalArgumentException",
            "Tag must not contain leading or trailing spaces.",
        ));
    }
    let text = String::from_utf16(value.units()).map_err(|_| {
        YamlError::plain(
            ErrorKind::Unsupported,
            "Exact PercentEscaper invalid UTF-16 tag failure is not ported",
        )
    })?;
    let mut out = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() || "-_.!~*'()@:$&,;=[]/".contains(c) {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).as_bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
    }
    Ok(Text::from(out))
}

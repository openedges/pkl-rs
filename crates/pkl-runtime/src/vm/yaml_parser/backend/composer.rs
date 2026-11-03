// Copyright (c) 2018, SnakeYAML. Apache-2.0. See PROVENANCE.md.
//! Document-at-a-time composition with identity-preserving alias edges.
use super::diagnostic::ErrorKind;
use super::nodes::tag;
use super::parser::{Event, EventId, Parser};
use super::{Document, Node, NodeId, NodeKind, Settings, Text, YamlError};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Consumption {
    Fresh,
    Stream,
    Single,
    Finished,
}
/// Pure backend. `single_document` verifies stream exhaustion before returning a
/// graph. `next_document` leaves later-document construction to its caller.
pub struct Backend {
    parser: Parser,
    settings: Settings,
    consumption: Consumption,
}
struct Collection {
    id: NodeId,
    key: Option<NodeId>,
}
impl Backend {
    pub fn new(input: Text, settings: Settings) -> Self {
        Self {
            parser: Parser::new(input, settings.label.clone()),
            settings,
            consumption: Consumption::Fresh,
        }
    }
    pub fn single_document(&mut self) -> Result<Option<Document>, YamlError> {
        if self.consumption != Consumption::Fresh {
            return Err(YamlError::plain(
                ErrorKind::Unsupported,
                "single_document requires a fresh backend",
            ));
        }
        self.consumption = Consumption::Single;
        self.parser.next()?;
        let document = if !self.parser.check(EventId::StreamEnd)? {
            Some(self.compose()?)
        } else {
            None
        };
        if !self.parser.check(EventId::StreamEnd)? {
            let event = self.parser.next()?;
            let previous = document.as_ref().map(|d| d.node(d.root()).start.clone());
            return Err(YamlError::marked(
                ErrorKind::Composer,
                Some("expected a single document in the stream"),
                previous,
                "but found another document",
                Some(event.start),
            ));
        }
        self.parser.next()?;
        self.consumption = Consumption::Finished;
        Ok(document)
    }
    pub fn next_document(&mut self) -> Result<Option<Document>, YamlError> {
        if self.consumption == Consumption::Finished {
            return Ok(None);
        }
        if self.consumption == Consumption::Single {
            return Err(YamlError::plain(
                ErrorKind::Unsupported,
                "cannot resume single-document backend as a stream",
            ));
        }
        if self.consumption == Consumption::Fresh {
            self.parser.next()?;
            self.consumption = Consumption::Stream;
        }
        if self.parser.check(EventId::StreamEnd)? {
            self.consumption = Consumption::Finished;
            return Ok(None);
        }
        self.compose().map(Some)
    }
    fn compose(&mut self) -> Result<Document, YamlError> {
        let start = self.parser.next()?;
        if start.id != EventId::DocumentStart {
            return Err(YamlError::plain(
                ErrorKind::Engine,
                "Expected document start event",
            ));
        }
        let mut nodes: Vec<Node> = Vec::new();
        let mut anchors: BTreeMap<Text, NodeId> = BTreeMap::new();
        let mut recursive = BTreeSet::new();
        let mut merges = BTreeSet::new();
        let mut aliases = 0usize;
        let mut stack: Vec<Collection> = Vec::new();
        let root = loop {
            let event = self.parser.peek()?;
            if matches!(event.id, EventId::SequenceEnd | EventId::MappingEnd) {
                let event = self.parser.next()?;
                let frame = stack.pop().expect("collection end has start");
                nodes[frame.id.0].end = event.end;
                if merges.contains(&frame.id) {
                    self.flatten(frame.id, &mut nodes, &anchors, &merges)?;
                    merges.remove(&frame.id);
                }
                if let Some(parent) = stack.last_mut() {
                    attach(&mut nodes, parent, frame.id, &mut merges);
                    recursive.remove(&parent.id);
                } else {
                    break frame.id;
                }
                continue;
            }
            if let Some(parent) = stack.last() {
                recursive.insert(parent.id);
            }
            let event = self.parser.next()?;
            let id = if event.id == EventId::Alias {
                let anchor = event.anchor.as_ref().expect("alias anchor");
                let Some(&id) = anchors.get(anchor) else {
                    let mut problem = "found undefined alias ".encode_utf16().collect::<Vec<_>>();
                    problem.extend(anchor.units());
                    return Err(YamlError::marked(
                        ErrorKind::Composer,
                        None,
                        None,
                        Text::from_units(problem),
                        Some(event.start),
                    ));
                };
                if !matches!(nodes[id.0].kind, NodeKind::Scalar { .. }) {
                    aliases += 1;
                    if aliases > self.settings.max_collection_aliases {
                        return Err(YamlError::plain(
                            ErrorKind::Engine,
                            format!(
                                "Number of aliases for non-scalar nodes exceeds the specified max={}",
                                self.settings.max_collection_aliases
                            ),
                        ));
                    }
                }
                if recursive.remove(&id) {
                    nodes[id.0].recursive = true;
                }
                id
            } else {
                let id = NodeId(nodes.len());
                let node = make_node(event, &self.settings)?;
                if let Some(anchor) = &node.anchor {
                    anchors.insert(anchor.clone(), id);
                }
                let collection = !matches!(node.kind, NodeKind::Scalar { .. });
                nodes.push(node);
                if collection {
                    stack.push(Collection { id, key: None });
                    continue;
                }
                id
            };
            if let Some(parent) = stack.last_mut() {
                attach(&mut nodes, parent, id, &mut merges);
                recursive.remove(&parent.id);
            } else {
                break id;
            }
        };
        let end = self.parser.next()?;
        if end.id != EventId::DocumentEnd {
            return Err(YamlError::plain(
                ErrorKind::Engine,
                "Expected document end event",
            ));
        }
        Ok(Document { root, nodes })
    }
    fn mapping_id(
        &mut self,
        id: NodeId,
        nodes: &[Node],
        anchors: &BTreeMap<Text, NodeId>,
    ) -> Result<NodeId, YamlError> {
        if matches!(nodes[id.0].kind, NodeKind::Mapping { .. }) {
            return Ok(id);
        }
        if let Some(anchor) = &nodes[id.0].anchor
            && let Some(&other) = anchors.get(anchor)
            && matches!(nodes[other.0].kind, NodeKind::Mapping { .. })
        {
            return Ok(other);
        }
        let event = self.parser.peek()?;
        Err(YamlError::marked(
            ErrorKind::Composer,
            None,
            None,
            "Expected mapping node or an anchor referencing mapping",
            Some(event.start),
        ))
    }
    fn flatten(
        &mut self,
        id: NodeId,
        nodes: &mut [Node],
        anchors: &BTreeMap<Text, NodeId>,
        merges: &BTreeSet<NodeId>,
    ) -> Result<(), YamlError> {
        let NodeKind::Mapping { entries, .. } = &nodes[id.0].kind else {
            unreachable!()
        };
        let mut to_process = entries.clone();
        let mut seen = BTreeSet::new();
        loop {
            if !seen.insert(to_process.clone()) {
                return Err(YamlError::plain(
                    ErrorKind::Unsupported,
                    "Nonterminating explicit merge expansion requires the pinned Java lifecycle outcome",
                ));
            }
            let mut process = false;
            let mut updated = Vec::new();
            let mut keys = BTreeSet::new();
            let mut merge_tuples = Vec::new();
            for &(key, value) in &to_process {
                if nodes[key.0].tag == tag("merge") {
                    merge_tuples.push((key, value));
                } else {
                    updated.push((key, value));
                    if let NodeKind::Scalar { value, .. } = &nodes[key.0].kind {
                        keys.insert(value.clone());
                    }
                }
            }
            for (_, value) in merge_tuples {
                let refs = match &nodes[value.0].kind {
                    NodeKind::Sequence { children, .. } => children.clone(),
                    _ => vec![value],
                };
                for reference in refs {
                    let mapped = self.mapping_id(reference, nodes, anchors)?;
                    process |= merges.contains(&mapped);
                    let NodeKind::Mapping { entries, .. } = &nodes[mapped.0].kind else {
                        unreachable!()
                    };
                    let mut found = BTreeSet::new();
                    for &(key, value) in entries {
                        if let NodeKind::Scalar { value: scalar, .. } = &nodes[key.0].kind {
                            if !keys.contains(scalar) {
                                updated.push((key, value));
                                found.insert(scalar.clone());
                            }
                        } else {
                            updated.push((key, value));
                        }
                    }
                    keys.extend(found);
                }
            }
            if process {
                to_process = updated;
            } else {
                let NodeKind::Mapping { entries, .. } = &mut nodes[id.0].kind else {
                    unreachable!()
                };
                *entries = updated;
                return Ok(());
            }
        }
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

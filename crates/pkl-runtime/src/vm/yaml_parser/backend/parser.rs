// Copyright (c) 2018, SnakeYAML. Apache-2.0. See PROVENANCE.md.
//! Explicit Engine ParserImpl production stack; comments are disabled by Pkl.
use super::diagnostic::ErrorKind;
use super::scanner::{Data, Directive, Id, Scanner, Token};
use super::{Mark, ScalarStyle, Text, YamlError};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EventId {
    StreamStart,
    StreamEnd,
    DocumentStart,
    DocumentEnd,
    Alias,
    Scalar,
    SequenceStart,
    SequenceEnd,
    MappingStart,
    MappingEnd,
}
#[derive(Clone, Debug)]
pub(crate) struct Event {
    pub id: EventId,
    pub start: Mark,
    pub end: Mark,
    pub anchor: Option<Text>,
    pub tag: Option<Text>,
    pub implicit: bool,
    pub value: Text,
    pub style: ScalarStyle,
    pub flow: bool,
}
impl Event {
    fn new(id: EventId, start: Mark, end: Mark) -> Self {
        Self {
            id,
            start,
            end,
            anchor: None,
            tag: None,
            implicit: true,
            value: Text::default(),
            style: ScalarStyle::Plain,
            flow: false,
        }
    }
    fn token(id: EventId, t: Token) -> Self {
        Self::new(id, t.start, t.end)
    }
    fn empty(mark: Mark) -> Self {
        Self::new(EventId::Scalar, mark.clone(), mark)
    }
}
#[derive(Clone, Copy, Debug)]
enum State {
    StreamStart,
    ImplicitDocumentStart,
    DocumentStart,
    DocumentEnd,
    DocumentContent,
    Node(bool, bool),
    BlockSequenceFirst,
    BlockSequenceEntry,
    IndentlessSequenceEntry,
    BlockMappingFirst,
    BlockMappingKey,
    BlockMappingValue,
    FlowSequenceFirst,
    FlowSequenceEntry(bool),
    FlowSequenceMappingKey,
    FlowSequenceMappingValue,
    FlowSequenceMappingEnd,
    FlowMappingFirst,
    FlowMappingKey(bool),
    FlowMappingValue,
    FlowMappingEmptyValue,
    End,
}
pub(crate) struct Parser {
    scanner: Scanner,
    state: State,
    states: Vec<State>,
    marks: Vec<Mark>,
    tags: BTreeMap<Text, Text>,
    event: Option<Event>,
}
impl Parser {
    pub fn new(input: Text, label: Text) -> Self {
        Self {
            scanner: Scanner::new(input, label),
            state: State::StreamStart,
            states: Vec::new(),
            marks: Vec::new(),
            tags: default_tags(),
            event: None,
        }
    }
    pub fn peek(&mut self) -> Result<Event, YamlError> {
        if self.event.is_none() {
            self.event = Some(self.produce()?);
        }
        Ok(self.event.as_ref().unwrap().clone())
    }
    pub fn next(&mut self) -> Result<Event, YamlError> {
        let event = self.peek()?;
        self.event = None;
        Ok(event)
    }
    pub fn check(&mut self, id: EventId) -> Result<bool, YamlError> {
        Ok(self.peek()?.id == id)
    }
    fn resume(&mut self) {
        self.state = self.states.pop().expect("parser production stack");
    }
    fn fail(
        context: Option<&str>,
        context_mark: Option<Mark>,
        problem: impl Into<Text>,
        problem_mark: Mark,
    ) -> YamlError {
        YamlError::marked(
            ErrorKind::Parser,
            context,
            context_mark,
            problem,
            Some(problem_mark),
        )
    }
    fn directives(&mut self) -> Result<(), YamlError> {
        let mut version = false;
        let mut tags = BTreeMap::new();
        while self.scanner.check(&[Id::Directive])? {
            let token = self.scanner.next()?;
            match token.data {
                Data::Directive(_, Some(Directive::Yaml(major, minor))) => {
                    if version {
                        return Err(Self::fail(
                            None,
                            None,
                            "found duplicate YAML directive",
                            token.start,
                        ));
                    }
                    if major != 1 {
                        return Err(YamlError::plain(
                            ErrorKind::Version,
                            format!("Version{{major={major}, minor={minor}}}"),
                        ));
                    }
                    version = true;
                }
                Data::Directive(_, Some(Directive::Tag(handle, prefix))) => {
                    if tags.contains_key(&handle) {
                        let mut msg = "duplicate tag handle ".encode_utf16().collect::<Vec<_>>();
                        msg.extend(handle.units());
                        return Err(Self::fail(None, None, Text::from_units(msg), token.start));
                    }
                    tags.insert(handle, prefix);
                }
                _ => {}
            }
        }
        for (k, v) in default_tags() {
            tags.entry(k).or_insert(v);
        }
        self.tags = tags;
        Ok(())
    }
    fn produce(&mut self) -> Result<Event, YamlError> {
        loop {
            match self.state {
                State::StreamStart => {
                    let t = self.scanner.next()?;
                    self.state = State::ImplicitDocumentStart;
                    return Ok(Event::token(EventId::StreamStart, t));
                }
                State::ImplicitDocumentStart => {
                    if !self
                        .scanner
                        .check(&[Id::Directive, Id::DocumentStart, Id::StreamEnd])?
                    {
                        let t = self.scanner.peek()?;
                        self.states.push(State::DocumentEnd);
                        self.state = State::Node(true, false);
                        return Ok(Event::new(EventId::DocumentStart, t.start.clone(), t.start));
                    }
                    self.state = State::DocumentStart;
                }
                State::DocumentStart => {
                    while self.scanner.check(&[Id::DocumentEnd])? {
                        self.scanner.next()?;
                    }
                    if !self.scanner.check(&[Id::StreamEnd])? {
                        self.scanner.reset_document_index();
                        let start = self.scanner.peek()?.start;
                        self.directives()?;
                        if !self.scanner.check(&[Id::DocumentStart])? {
                            let t = self.scanner.peek()?;
                            return Err(Self::fail(
                                None,
                                None,
                                format!(
                                    "expected '<document start>', but found '{}'",
                                    t.id.description()
                                ),
                                t.start,
                            ));
                        }
                        let t = self.scanner.next()?;
                        self.states.push(State::DocumentEnd);
                        self.state = State::DocumentContent;
                        return Ok(Event::new(EventId::DocumentStart, start, t.end));
                    }
                    let t = self.scanner.next()?;
                    if !self.states.is_empty() || !self.marks.is_empty() {
                        return Err(YamlError::plain(
                            ErrorKind::Unsupported,
                            "Unexpected parser stack state at stream end; exact Java state-list diagnostic is not ported",
                        ));
                    }
                    self.state = State::End;
                    return Ok(Event::token(EventId::StreamEnd, t));
                }
                State::DocumentEnd => {
                    let mut t = self.scanner.peek()?;
                    let start = t.start.clone();
                    let mut end = start.clone();
                    if t.id == Id::DocumentEnd {
                        t = self.scanner.next()?;
                        end = t.end;
                    } else if t.id == Id::Directive {
                        return Err(Self::fail(
                            None,
                            None,
                            format!(
                                "expected '<document end>' before directives, but found '{}'",
                                t.id.description()
                            ),
                            t.start,
                        ));
                    }
                    self.tags.clear();
                    self.state = State::DocumentStart;
                    return Ok(Event::new(EventId::DocumentEnd, start, end));
                }
                State::DocumentContent => {
                    if self.scanner.check(&[
                        Id::Directive,
                        Id::DocumentStart,
                        Id::DocumentEnd,
                        Id::StreamEnd,
                    ])? {
                        let mark = self.scanner.peek()?.start;
                        self.resume();
                        return Ok(Event::empty(mark));
                    }
                    self.state = State::Node(true, false);
                }
                State::Node(block, indentless) => return self.node(block, indentless),
                State::BlockSequenceFirst => {
                    let t = self.scanner.next()?;
                    self.marks.push(t.start);
                    self.state = State::BlockSequenceEntry;
                }
                State::BlockSequenceEntry => {
                    if self.scanner.check(&[Id::BlockEntry])? {
                        let t = self.scanner.next()?;
                        if !self.scanner.check(&[Id::BlockEntry, Id::BlockEnd])? {
                            self.states.push(State::BlockSequenceEntry);
                            self.state = State::Node(true, false);
                        } else {
                            self.state = State::BlockSequenceEntry;
                            return Ok(Event::empty(t.end));
                        }
                    } else {
                        let t = self.scanner.peek()?;
                        if t.id != Id::BlockEnd {
                            return Err(Self::fail(
                                Some("while parsing a block collection"),
                                self.marks.pop(),
                                format!("expected <block end>, but found '{}'", t.id.description()),
                                t.start,
                            ));
                        }
                        let t = self.scanner.next()?;
                        self.resume();
                        self.marks.pop();
                        return Ok(Event::token(EventId::SequenceEnd, t));
                    }
                }
                State::IndentlessSequenceEntry => {
                    if self.scanner.check(&[Id::BlockEntry])? {
                        let t = self.scanner.next()?;
                        if !self.scanner.check(&[
                            Id::BlockEntry,
                            Id::Key,
                            Id::Value,
                            Id::BlockEnd,
                        ])? {
                            self.states.push(State::IndentlessSequenceEntry);
                            self.state = State::Node(true, false);
                        } else {
                            return Ok(Event::empty(t.end));
                        }
                    } else {
                        let t = self.scanner.peek()?;
                        self.resume();
                        return Ok(Event::token(EventId::SequenceEnd, t));
                    }
                }
                State::BlockMappingFirst => {
                    let t = self.scanner.next()?;
                    self.marks.push(t.start);
                    self.state = State::BlockMappingKey;
                }
                State::BlockMappingKey => {
                    if self.scanner.check(&[Id::Key])? {
                        let t = self.scanner.next()?;
                        if !self.scanner.check(&[Id::Key, Id::Value, Id::BlockEnd])? {
                            self.states.push(State::BlockMappingValue);
                            self.state = State::Node(true, true);
                        } else {
                            self.state = State::BlockMappingValue;
                            return Ok(Event::empty(t.end));
                        }
                    } else {
                        let t = self.scanner.peek()?;
                        if t.id != Id::BlockEnd {
                            return Err(Self::fail(
                                Some("while parsing a block mapping"),
                                self.marks.pop(),
                                format!("expected <block end>, but found '{}'", t.id.description()),
                                t.start,
                            ));
                        }
                        let t = self.scanner.next()?;
                        self.resume();
                        self.marks.pop();
                        return Ok(Event::token(EventId::MappingEnd, t));
                    }
                }
                State::BlockMappingValue => {
                    if self.scanner.check(&[Id::Value])? {
                        let t = self.scanner.next()?;
                        if !self.scanner.check(&[Id::Key, Id::Value, Id::BlockEnd])? {
                            self.states.push(State::BlockMappingKey);
                            self.state = State::Node(true, true);
                        } else {
                            self.state = State::BlockMappingKey;
                            return Ok(Event::empty(t.end));
                        }
                    } else if self.scanner.check(&[Id::Scalar])? {
                        self.states.push(State::BlockMappingKey);
                        self.state = State::Node(true, true);
                    } else {
                        self.state = State::BlockMappingKey;
                        return Ok(Event::empty(self.scanner.peek()?.start));
                    }
                }
                State::FlowSequenceFirst => {
                    let t = self.scanner.next()?;
                    self.marks.push(t.start);
                    self.state = State::FlowSequenceEntry(true);
                }
                State::FlowSequenceEntry(first) => {
                    if !self.scanner.check(&[Id::FlowSequenceEnd])? {
                        if !first {
                            if self.scanner.check(&[Id::FlowEntry])? {
                                self.scanner.next()?;
                            } else {
                                let t = self.scanner.peek()?;
                                return Err(Self::fail(
                                    Some("while parsing a flow sequence"),
                                    self.marks.pop(),
                                    format!("expected ',' or ']', but got {}", t.id.description()),
                                    t.start,
                                ));
                            }
                        }
                        if self.scanner.check(&[Id::Key])? {
                            let t = self.scanner.peek()?;
                            let mut event = Event::token(EventId::MappingStart, t);
                            event.flow = true;
                            self.state = State::FlowSequenceMappingKey;
                            return Ok(event);
                        }
                        if !self.scanner.check(&[Id::FlowSequenceEnd])? {
                            self.states.push(State::FlowSequenceEntry(false));
                            self.state = State::Node(false, false);
                            continue;
                        }
                    }
                    let t = self.scanner.next()?;
                    let _ = self.scanner.peek()?;
                    self.resume();
                    self.marks.pop();
                    return Ok(Event::token(EventId::SequenceEnd, t));
                }
                State::FlowSequenceMappingKey => {
                    let t = self.scanner.next()?;
                    if !self
                        .scanner
                        .check(&[Id::Value, Id::FlowEntry, Id::FlowSequenceEnd])?
                    {
                        self.states.push(State::FlowSequenceMappingValue);
                        self.state = State::Node(false, false);
                    } else {
                        self.state = State::FlowSequenceMappingValue;
                        return Ok(Event::empty(t.end));
                    }
                }
                State::FlowSequenceMappingValue => {
                    if self.scanner.check(&[Id::Value])? {
                        let t = self.scanner.next()?;
                        if !self.scanner.check(&[Id::FlowEntry, Id::FlowSequenceEnd])? {
                            self.states.push(State::FlowSequenceMappingEnd);
                            self.state = State::Node(false, false);
                        } else {
                            self.state = State::FlowSequenceMappingEnd;
                            return Ok(Event::empty(t.end));
                        }
                    } else {
                        self.state = State::FlowSequenceMappingEnd;
                        return Ok(Event::empty(self.scanner.peek()?.start));
                    }
                }
                State::FlowSequenceMappingEnd => {
                    self.state = State::FlowSequenceEntry(false);
                    return Ok(Event::token(EventId::MappingEnd, self.scanner.peek()?));
                }
                State::FlowMappingFirst => {
                    let t = self.scanner.next()?;
                    self.marks.push(t.start);
                    self.state = State::FlowMappingKey(true);
                }
                State::FlowMappingKey(first) => {
                    if !self.scanner.check(&[Id::FlowMappingEnd])? {
                        if !first {
                            if self.scanner.check(&[Id::FlowEntry])? {
                                self.scanner.next()?;
                            } else {
                                let t = self.scanner.peek()?;
                                return Err(Self::fail(
                                    Some("while parsing a flow mapping"),
                                    self.marks.pop(),
                                    format!("expected ',' or '}}', but got {}", t.id.description()),
                                    t.start,
                                ));
                            }
                        }
                        if self.scanner.check(&[Id::Key])? {
                            let t = self.scanner.next()?;
                            if !self.scanner.check(&[
                                Id::Value,
                                Id::FlowEntry,
                                Id::FlowMappingEnd,
                            ])? {
                                self.states.push(State::FlowMappingValue);
                                self.state = State::Node(false, false);
                                continue;
                            }
                            self.state = State::FlowMappingValue;
                            return Ok(Event::empty(t.end));
                        } else if !self.scanner.check(&[Id::FlowMappingEnd])? {
                            self.states.push(State::FlowMappingEmptyValue);
                            self.state = State::Node(false, false);
                            continue;
                        }
                    }
                    let t = self.scanner.next()?;
                    self.marks.pop();
                    let _ = self.scanner.peek()?;
                    self.resume();
                    return Ok(Event::token(EventId::MappingEnd, t));
                }
                State::FlowMappingValue => {
                    if self.scanner.check(&[Id::Value])? {
                        let t = self.scanner.next()?;
                        if !self.scanner.check(&[Id::FlowEntry, Id::FlowMappingEnd])? {
                            self.states.push(State::FlowMappingKey(false));
                            self.state = State::Node(false, false);
                        } else {
                            self.state = State::FlowMappingKey(false);
                            return Ok(Event::empty(t.end));
                        }
                    } else {
                        self.state = State::FlowMappingKey(false);
                        return Ok(Event::empty(self.scanner.peek()?.start));
                    }
                }
                State::FlowMappingEmptyValue => {
                    self.state = State::FlowMappingKey(false);
                    return Ok(Event::empty(self.scanner.peek()?.start));
                }
                State::End => {
                    return Err(YamlError::plain(ErrorKind::Engine, "No more Events found."));
                }
            }
        }
    }
    fn node(&mut self, block: bool, indentless: bool) -> Result<Event, YamlError> {
        if self.scanner.check(&[Id::Alias])? {
            let t = self.scanner.next()?;
            let Data::Name(name) = t.data else {
                unreachable!()
            };
            let mut e = Event::new(EventId::Alias, t.start, t.end);
            e.anchor = Some(name);
            self.resume();
            return Ok(e);
        }
        let mut start = None;
        let mut end = None;
        let mut tag_mark = None;
        let mut anchor = None;
        let mut tag_tuple = None;
        if self.scanner.check(&[Id::Anchor])? {
            let t = self.scanner.next()?;
            start = Some(t.start);
            end = Some(t.end);
            let Data::Name(name) = t.data else {
                unreachable!()
            };
            anchor = Some(name);
            if self.scanner.check(&[Id::Tag])? {
                let t = self.scanner.next()?;
                tag_mark = Some(t.start);
                end = Some(t.end);
                let Data::Tag(h, s) = t.data else {
                    unreachable!()
                };
                tag_tuple = Some((h, s));
            }
        } else if self.scanner.check(&[Id::Tag])? {
            let t = self.scanner.next()?;
            start = Some(t.start.clone());
            tag_mark = Some(t.start);
            end = Some(t.end);
            let Data::Tag(h, s) = t.data else {
                unreachable!()
            };
            tag_tuple = Some((h, s));
            if self.scanner.check(&[Id::Anchor])? {
                let t = self.scanner.next()?;
                end = Some(t.end);
                let Data::Name(name) = t.data else {
                    unreachable!()
                };
                anchor = Some(name);
            }
        }
        let mut tag = None;
        if let Some((handle, suffix)) = tag_tuple {
            tag = Some(if let Some(handle) = handle {
                let Some(prefix) = self.tags.get(&handle) else {
                    let mut msg = "found undefined tag handle "
                        .encode_utf16()
                        .collect::<Vec<_>>();
                    msg.extend(handle.units());
                    return Err(Self::fail(
                        Some("while parsing a node"),
                        start,
                        Text::from_units(msg),
                        tag_mark.unwrap(),
                    ));
                };
                let mut units = prefix.units().to_vec();
                units.extend(suffix.units());
                Text::from_units(units)
            } else {
                suffix
            });
        }
        if start.is_none() {
            let mark = self.scanner.peek()?.start;
            start = Some(mark.clone());
            end = Some(mark);
        }
        let start = start.unwrap();
        let t = self.scanner.peek()?;
        let mut event = Event::new(EventId::Scalar, start.clone(), end.unwrap());
        event.anchor = anchor;
        event.tag = tag;
        event.implicit = event.tag.is_none();
        if indentless && t.id == Id::BlockEntry {
            event.id = EventId::SequenceStart;
            event.end = t.end;
            self.state = State::IndentlessSequenceEntry;
        } else {
            match t.id {
                Id::Scalar => {
                    let t = self.scanner.next()?;
                    let Data::Scalar(value, style) = t.data else {
                        unreachable!()
                    };
                    event.end = t.end;
                    event.value = value;
                    event.style = style;
                    event.implicit = style == ScalarStyle::Plain && event.tag.is_none();
                    self.resume();
                }
                Id::FlowSequenceStart => {
                    event.id = EventId::SequenceStart;
                    event.end = t.end;
                    event.flow = true;
                    self.state = State::FlowSequenceFirst;
                }
                Id::FlowMappingStart => {
                    event.id = EventId::MappingStart;
                    event.end = t.end;
                    event.flow = true;
                    self.state = State::FlowMappingFirst;
                }
                Id::BlockSequenceStart if block => {
                    event.id = EventId::SequenceStart;
                    event.end = t.start;
                    self.state = State::BlockSequenceFirst;
                }
                Id::BlockMappingStart if block => {
                    event.id = EventId::MappingStart;
                    event.end = t.start;
                    self.state = State::BlockMappingFirst;
                }
                _ if event.anchor.is_some() || event.tag.is_some() => {
                    self.resume();
                }
                _ => {
                    return Err(Self::fail(
                        Some(if block {
                            "while parsing a block node"
                        } else {
                            "while parsing a flow node"
                        }),
                        Some(start),
                        format!(
                            "expected the node content, but found '{}'",
                            t.id.description()
                        ),
                        t.start,
                    ));
                }
            }
        }
        Ok(event)
    }
}
fn default_tags() -> BTreeMap<Text, Text> {
    [
        (Text::from("!"), Text::from("!")),
        (Text::from("!!"), Text::from("tag:yaml.org,2002:")),
    ]
    .into()
}

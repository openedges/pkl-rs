//! Representation graph. Aliases are edges to the same NodeId, not copied nodes.
use super::diagnostic::Mark;
use std::sync::Arc;
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Text(pub Arc<[u16]>);
impl Text {
    pub fn units(&self) -> &[u16] {
        &self.0
    }
    pub fn from_units(units: impl Into<Arc<[u16]>>) -> Self {
        Self(units.into())
    }
    pub fn is_ascii(&self, text: &str) -> bool {
        self.0.iter().copied().eq(text.encode_utf16())
    }
    pub(crate) fn from_points(points: &[u32]) -> Self {
        let mut units = Vec::new();
        for &cp in points {
            push_point(&mut units, cp);
        }
        Self(units.into())
    }
}
impl From<&str> for Text {
    fn from(s: &str) -> Self {
        Self(s.encode_utf16().collect())
    }
}
impl From<String> for Text {
    fn from(s: String) -> Self {
        Self::from(s.as_str())
    }
}
pub(crate) fn push_point(units: &mut Vec<u16>, cp: u32) {
    if cp <= 0xffff {
        units.push(cp as u16);
    } else {
        let n = cp - 0x10000;
        units.push((0xd800 + (n >> 10)) as u16);
        units.push((0xdc00 + (n & 1023)) as u16);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NodeId(pub usize);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalarStyle {
    Plain,
    SingleQuoted,
    DoubleQuoted,
    Literal,
    Folded,
}
#[derive(Clone, Debug)]
pub enum NodeKind {
    Scalar {
        value: Text,
        #[allow(
            dead_code,
            reason = "Preserve scalar style in the composed representation graph; VM construction uses the resolved tag."
        )]
        style: ScalarStyle,
    },
    Sequence {
        children: Vec<NodeId>,
        #[allow(
            dead_code,
            reason = "Preserve collection presentation in the composed graph; VM construction is independent of flow style."
        )]
        flow: bool,
    },
    Mapping {
        entries: Vec<(NodeId, NodeId)>,
        #[allow(
            dead_code,
            reason = "Preserve collection presentation in the composed graph; VM construction is independent of flow style."
        )]
        flow: bool,
    },
}
#[derive(Clone, Debug)]
pub struct Node {
    pub tag: Text,
    #[allow(
        dead_code,
        reason = "Preserve implicit-tag resolution provenance for graph comparison; construction consumes the final tag."
    )]
    pub resolved: bool,
    pub kind: NodeKind,
    pub start: Mark,
    pub end: Mark,
    pub anchor: Option<Text>,
    pub recursive: bool,
}
#[derive(Clone, Debug)]
pub struct Document {
    pub(crate) root: NodeId,
    pub(crate) nodes: Vec<Node>,
}
impl Document {
    pub fn root(&self) -> NodeId {
        self.root
    }
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.0]
    }
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }
}
pub(crate) fn tag(name: &str) -> Text {
    Text::from(format!("tag:yaml.org,2002:{name}"))
}

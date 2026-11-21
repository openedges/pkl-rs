// Compression and loop rendering adapted from Pkl 0.32.0 StackTraceRenderer.java.
// Copyright © 2024-2026 Apple Inc. and the Pkl project authors. All rights reserved.
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at https://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.

use super::RenderFrame;
use super::ansi::{AnsiStringBuilder, theme};
use crate::pkl_string::PklString;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Group {
    Frame(usize),
    Loop { frames: Vec<Group>, count: usize },
}

/// Equality follows org.pkl.core.StackFrame, before display whitespace stripping
/// and highlighting. Neither Rust source identity nor formatted output is a key.
#[derive(PartialEq, Eq)]
struct FrameKey<'a> {
    uri: &'a PklString,
    member: &'a crate::value::PklString,
    source_lines: Vec<PklString>,
    start_line: usize,
    start_column: usize,
    end_line: usize,
    end_column: usize,
}

fn key<'a>(frame: &'a RenderFrame<'_>) -> Option<FrameKey<'a>> {
    key_with_offset(frame, 0)
}

fn key_with_offset<'a>(frame: &'a RenderFrame<'_>, offset_utf16: usize) -> Option<FrameKey<'a>> {
    let mut start = frame.source.position(frame.span.start)?;
    if offset_utf16 != 0 {
        let scalar = frame
            .source
            .text()
            .entire()
            .get(frame.span.start..)?
            .first()?;
        // This override represents only an interior surrogate-unit boundary;
        // ordinary source positions and newline handling retain their path.
        if offset_utf16 >= scalar.len_utf16() {
            return None;
        }
        start.utf16 += offset_utf16;
        start.column_utf16 += offset_utf16;
    }
    let (end_line, end_column) = if let Some(width) = frame.highlight_length_utf16 {
        let source_length = frame
            .source
            .position(frame.source.text().len_bytes())?
            .utf16;
        if source_length - start.utf16 < width {
            // Pinned Truffle SourceSectionLoaded exposes 1:1 coordinates for
            // an invalid section. A parser insertion at exact EOF has width
            // one and is invalid; a following space or newline makes it valid.
            // Preserve the parser's authored span and apply the source-section
            // rule only when converting it to observable frame coordinates.
            start = frame.source.position(0)?;
            (1, 1)
        } else {
            (start.line, start.column_utf16 + width.saturating_sub(1))
        }
    } else if let Some((offset, ch)) = frame.source.slice(frame.span)?.characters().next_back() {
        let end = frame.source.position(frame.span.start + offset)?;
        (end.line, end.column_utf16 + ch.len_utf16() - 1)
    } else {
        (start.line, start.column_utf16)
    };
    let source_lines = frame
        .source
        .text()
        .lines()
        .skip(start.line - 1)
        .take(end_line - start.line + 1)
        .map(crate::source::SourceSlice::to_pkl_string)
        .collect::<Vec<_>>();
    if source_lines.len() != end_line - start.line + 1 {
        return None;
    }
    Some(FrameKey {
        uri: &frame.uri,
        member: frame.member,
        source_lines,
        start_line: start.line,
        start_column: start.column_utf16,
        end_line,
        end_column,
    })
}

fn compress<T: PartialEq>(frames: &[T]) -> Vec<Group> {
    let mut result = Vec::new();
    compress_range(
        frames,
        &mut vec![0; frames.len()],
        &mut result,
        0,
        frames.len(),
    );
    result
}

fn compress_range<T: PartialEq>(
    frames: &[T],
    lpps: &mut [usize],
    result: &mut Vec<Group>,
    beginning: usize,
    ending: usize,
) {
    let total_size = ending - beginning;
    let mut max_length = 0;
    let mut pattern_start = 0;
    let mut match_end = 0;
    'search: for i in beginning..ending {
        let mut best = i;
        let mut len = 0;
        lpps[i] = 0;
        let mut j = i + 1;
        while j < ending {
            let matches = frames[frames.len() - 1 - j] == frames[frames.len() - 1 - (len + i)];
            if !matches && len != 0 {
                len = lpps[len - 1];
            } else {
                len += usize::from(matches);
                lpps[j] = len;
                if len > lpps[best] {
                    best = j;
                } else if len > 0 && len == lpps[j - 1] {
                    continue 'search;
                }
                j += 1;
            }
        }
        let length = best - i + 1;
        if length > 1 && max_length < length {
            max_length = length;
            match_end = best;
            pattern_start = i;
        }
        if max_length > ending - i || max_length * 2 > total_size {
            break;
        }
    }
    if max_length > 1 {
        // Preserve the pinned algorithm's partial-pattern/count-one behavior.
        let width = match_end - lpps[match_end] - pattern_start + 1;
        compress_range(frames, lpps, result, match_end + 1, ending);
        let mut nested = Vec::new();
        compress_range(
            frames,
            lpps,
            &mut nested,
            pattern_start,
            pattern_start + width,
        );
        result.push(Group::Loop {
            frames: nested,
            count: max_length / width,
        });
        compress_range(frames, lpps, result, beginning, pattern_start);
    } else {
        result.extend(
            (beginning..ending)
                .rev()
                .map(|i| Group::Frame(frames.len() - 1 - i)),
        );
    }
}

/// Compression uses the original complete frames. Transforming a leaf cannot
/// alter the selected loop layout or cause additional callback invocations.
#[derive(Clone, Debug)]
pub(super) struct DiagnosticFrames {
    frames: Vec<Option<crate::logging::LogFrame>>,
    groups: Vec<Group>,
    order: Vec<usize>,
    next: usize,
}
impl DiagnosticFrames {
    pub(super) fn new(frames: &[RenderFrame<'_>]) -> Self {
        Self::new_with_first_offset(frames, 0)
    }
    pub(super) fn new_with_first_offset(frames: &[RenderFrame<'_>], offset_utf16: usize) -> Self {
        let keys: Vec<_> = frames
            .iter()
            .enumerate()
            .map(|(index, frame)| {
                if index == 0 && offset_utf16 != 0 {
                    key_with_offset(frame, offset_utf16).ok_or(index)
                } else {
                    key(frame).ok_or(index)
                }
            })
            .collect();
        let groups = compress(&keys);
        fn visit(groups: &[Group], order: &mut Vec<usize>) {
            for group in groups {
                match group {
                    Group::Frame(index) => order.push(*index),
                    Group::Loop { frames, .. } => visit(frames, order),
                }
            }
        }
        let frames: Vec<_> = keys
            .into_iter()
            .map(|key| {
                key.ok().map(|key| crate::logging::LogFrame {
                    module_uri: key.uri.clone(),
                    member_name: Some(key.member.clone()),
                    source_lines: key.source_lines.into(),
                    start_line: key.start_line,
                    start_column: key.start_column,
                    end_line: key.end_line,
                    end_column: key.end_column,
                })
            })
            .collect();
        let mut order = Vec::new();
        visit(&groups, &mut order);
        order.retain(|index| frames[*index].is_some());
        Self {
            frames,
            groups,
            order,
            next: 0,
        }
    }
    pub(super) fn next(&self) -> Option<&crate::logging::LogFrame> {
        self.order
            .get(self.next)
            .and_then(|index| self.frames[*index].as_ref())
    }
    pub(super) fn replace(&mut self, frame: crate::logging::LogFrame) {
        let index = self.order[self.next];
        self.frames[index] = Some(frame);
        self.next += 1;
    }
    pub(super) fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }
    pub(super) fn render_ansi(&self, out: &mut AnsiStringBuilder, hint: Option<&PklString>) {
        render_groups_ansi(out, &self.frames, &self.groups, hint, false, "", true);
    }

    pub(super) fn render_ansi_precolored_hint(
        &self,
        out: &mut AnsiStringBuilder,
        hint: &PklString,
    ) {
        render_groups_ansi(out, &self.frames, &self.groups, Some(hint), true, "", true);
    }
}

fn render_hint_ansi(
    out: &mut AnsiStringBuilder,
    hint: Option<&PklString>,
    precolored: bool,
    margin: &str,
) {
    if let Some(hint) = hint {
        out.append_char('\n')
            .append_styled(theme::STACK_TRACE_MARGIN, margin);
        if precolored {
            out.append_pkl(hint);
        } else {
            out.append_styled_pkl(theme::ERROR_MESSAGE_HINT, hint);
        }
        out.append_char('\n');
    }
}

fn render_groups_ansi(
    out: &mut AnsiStringBuilder,
    frames: &[Option<crate::logging::LogFrame>],
    groups: &[Group],
    hint: Option<&PklString>,
    precolored_hint: bool,
    margin: &str,
    mut first: bool,
) {
    for group in groups {
        match group {
            Group::Loop {
                frames: nested,
                count: 1,
            } => render_groups_ansi(out, frames, nested, None, false, margin, first),
            Group::Loop {
                frames: nested,
                count,
            } => {
                if !first {
                    out.append_styled(theme::STACK_TRACE_MARGIN, margin)
                        .append_char('\n');
                }
                out.append_styled(theme::STACK_TRACE_MARGIN, margin)
                    .append_styled(theme::STACK_TRACE_MARGIN, "┌─ ")
                    .append_styled_usize(theme::STACK_TRACE_LOOP_COUNT, *count)
                    .append_str(" repetitions of:\n");
                let nested_margin = format!("{margin}│ ");
                render_groups_ansi(out, frames, nested, None, false, &nested_margin, first);
                if first {
                    render_hint_ansi(out, hint, precolored_hint, &nested_margin);
                    first = false;
                }
                out.append_styled(theme::STACK_TRACE_MARGIN, &format!("{margin}└─"))
                    .append_char('\n');
            }
            Group::Frame(index) => {
                if !first {
                    out.append_styled(theme::STACK_TRACE_MARGIN, margin)
                        .append_char('\n');
                }
                if let Some(frame) = &frames[*index] {
                    super::render_log_frame_ansi(out, frame, margin);
                }
            }
        }
        if first {
            render_hint_ansi(out, hint, precolored_hint, margin);
            first = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_prefix_suffix_and_thirty_frame_loop() {
        // The sizes and layout are from upstream StackTraceRendererTest.
        let frames: Vec<_> = (0..10)
            .chain((10..40).cycle().take(120))
            .chain(40..45)
            .collect();
        let compressed = compress(&frames);
        assert_eq!(compressed.len(), 16);
        assert_eq!(
            &compressed[..10],
            &(0..10).map(Group::Frame).collect::<Vec<_>>()
        );
        assert_eq!(
            compressed[10],
            Group::Loop {
                frames: (100..130).map(Group::Frame).collect(),
                count: 4,
            }
        );
        assert_eq!(
            &compressed[11..],
            &(130..135).map(Group::Frame).collect::<Vec<_>>()
        );
    }

    #[test]
    fn pinned_partial_pattern_keeps_count_one_loop() {
        assert_eq!(
            compress(&['A', 'B', 'A']),
            vec![Group::Loop {
                frames: vec![Group::Frame(1), Group::Frame(2)],
                count: 1,
            }]
        );
    }

    #[test]
    fn repeated_frame_rendering_keeps_loop_margins_and_first_hint() {
        use crate::source::{SourceMap, Span};
        let mut sources = SourceMap::default();
        let id = sources.add("source", "foo");
        let member = "f".into();
        let frames: Vec<_> = (0..3)
            .map(|_| RenderFrame {
                source: sources.get(id).unwrap(),
                uri: "repl:text".into(),
                member: &member,
                span: Span::new(0, 3),
                highlight_length_utf16: None,
            })
            .collect();
        assert_eq!(
            super::super::render_error("failure", &frames, Some("hint\ncontinued")),
            "–– Pkl Error ––\nfailure\n\n┌─ 3 repetitions of:\n│ 1 | foo\n│     ^^^\n│ at f (repl:text)\n\n│ hint\ncontinued\n└─\n"
        );
    }

    #[test]
    fn count_one_rendering_keeps_partial_pattern_and_hint_position() {
        use crate::source::{SourceMap, Span};
        let mut sources = SourceMap::default();
        let id = sources.add("source", "foo");
        let members: Vec<crate::value::PklString> = ["A", "B", "A"].map(Into::into).into();
        let frames: Vec<_> = members
            .iter()
            .map(|member| RenderFrame {
                source: sources.get(id).unwrap(),
                uri: "repl:text".into(),
                member,
                span: Span::new(0, 3),
                highlight_length_utf16: None,
            })
            .collect();
        assert_eq!(
            super::super::render_error("failure", &frames, Some("hint")),
            "–– Pkl Error ––\nfailure\n\n1 | foo\n    ^^^\nat B (repl:text)\n\n1 | foo\n    ^^^\nat A (repl:text)\n\nhint\n"
        );
    }

    #[test]
    fn compression_compares_all_source_lines_before_display_transforms() {
        use crate::source::{SourceMap, Span};
        let mut sources = SourceMap::default();
        let first = sources.add("first", "foo\nbar");
        let same = sources.add("second", "foo\r\nbar");
        let different = sources.add("third", "foo\nbaz");
        let member = "f".into();
        let frame = |id, end| RenderFrame {
            source: sources.get(id).unwrap(),
            uri: "repl:text".into(),
            member: &member,
            span: Span::new(0, end),
            highlight_length_utf16: None,
        };
        assert!(key(&frame(first, 7)) == key(&frame(same, 8)));
        assert!(key(&frame(first, 7)) != key(&frame(different, 7)));
        let mut other_uri = frame(first, 7);
        other_uri.uri = "repl:other".into();
        assert!(key(&frame(first, 7)) != key(&other_uri));
    }

    #[test]
    fn authored_surrogate_source_lines_are_distinct_before_writer_and_compression() {
        use crate::source::{SourceMap, Span};
        let mut sources = SourceMap::default();
        let high = sources.add("high", PklString::from_utf16(vec![0xd800]));
        let low = sources.add("low", PklString::from_utf16(vec![0xdc00]));
        let question = sources.add("question", "?");
        let member = PklString::from("f");
        let frames = [(high, 3), (low, 3), (question, 1)].map(|(id, end)| RenderFrame {
            source: sources.get(id).unwrap(),
            uri: "repl:text".into(),
            member: &member,
            span: Span::new(0, end),
            highlight_length_utf16: None,
        });
        assert_eq!(
            compress(&frames.iter().map(key).collect::<Vec<_>>()),
            vec![Group::Frame(0), Group::Frame(1), Group::Frame(2)]
        );
        for (frame, unit) in frames.iter().zip([0xd800, 0xdc00, 0x3f]) {
            let key = key(frame).unwrap();
            assert_eq!(
                key.source_lines[0].encode_utf16().collect::<Vec<_>>(),
                vec![unit]
            );
            assert_eq!((key.start_column, key.end_column), (1, 1));
            let text = super::super::render_error_text(
                &"failure".into(),
                std::slice::from_ref(frame),
                None,
            );
            let mut expected: Vec<_> = "–– Pkl Error ––\nfailure\n\n1 | ".encode_utf16().collect();
            expected.push(unit);
            expected.extend("\n    ^\nat f (repl:text)\n".encode_utf16());
            assert_eq!(text.encode_utf16().collect::<Vec<_>>(), expected);
        }
    }

    #[test]
    fn stage120_interior_pair_offset_and_lone_unit_positions_remain_distinct() {
        use crate::source::{SourceMap, Span};
        let mut sources = SourceMap::default();
        let id = sources.add(
            "mixed",
            PklString::from_utf16(vec![0xd800, 0xd83d, 0xde00, 13, 10, 0xdc00]),
        );
        let member = PklString::from("f");
        let mut frame = RenderFrame {
            source: sources.get(id).unwrap(),
            uri: "repl:text".into(),
            member: &member,
            span: Span::new(3, 3),
            highlight_length_utf16: Some(1),
        };
        let paired = key_with_offset(&frame, 1).unwrap();
        assert_eq!(
            (paired.start_line, paired.start_column, paired.end_column),
            (1, 3, 3)
        );
        frame.span = Span::new(0, 3);
        frame.highlight_length_utf16 = None;
        assert!(key_with_offset(&frame, 1).is_none());
        frame.span = Span::new(9, 12);
        let lone = key(&frame).unwrap();
        assert_eq!(
            (lone.start_line, lone.start_column, lone.end_column),
            (2, 1, 1)
        );
        assert_eq!(
            lone.source_lines[0].encode_utf16().collect::<Vec<_>>(),
            vec![0xdc00]
        );
    }

    #[test]
    fn compression_compares_utf16_member_names_before_writer_replacement() {
        use crate::source::{SourceMap, Span};
        use crate::value::PklString;
        let mut sources = SourceMap::default();
        let id = sources.add("source", "foo");
        // Java StackFrame.equals compares String code units. These three
        // distinct names all become the same text only at the UTF-8 writer.
        let members = [
            PklString::from_utf16(vec![0xd800]),
            PklString::from_utf16(vec![0xdc00]),
            "?".into(),
        ];
        let frames: Vec<_> = members
            .iter()
            .map(|member| RenderFrame {
                source: sources.get(id).unwrap(),
                uri: "repl:text".into(),
                member,
                span: Span::new(0, 3),
                highlight_length_utf16: None,
            })
            .collect();
        assert_eq!(
            compress(&frames.iter().map(key).collect::<Vec<_>>()),
            vec![Group::Frame(0), Group::Frame(1), Group::Frame(2)]
        );
        assert_eq!(
            super::super::render_error("failure", &frames, None),
            "–– Pkl Error ––\nfailure\n\n1 | foo\n    ^^^\nat ? (repl:text)\n\n1 | foo\n    ^^^\nat ? (repl:text)\n\n1 | foo\n    ^^^\nat ? (repl:text)\n"
        );
        // Pinned StackTraceRenderer keeps a two-frame repeated pattern distinct
        // from six identical names, despite the identical UTF-8 frame text.
        let alternating = [0, 2, 0, 2, 0, 2].map(|index| key(&frames[index]));
        assert_eq!(
            compress(&alternating),
            vec![Group::Loop {
                frames: vec![Group::Frame(4), Group::Frame(5)],
                count: 3,
            }]
        );
        let collapsed = [2; 6].map(|index| key(&frames[index]));
        assert_eq!(
            compress(&collapsed),
            vec![Group::Loop {
                frames: vec![Group::Frame(5)],
                count: 6,
            }]
        );
        let paired = [
            PklString::from_utf16(vec![0xd800, 0xdc00]),
            "\u{10000}".into(),
        ];
        assert_eq!(
            compress(&paired),
            vec![Group::Loop {
                frames: vec![Group::Frame(1)],
                count: 2,
            }]
        );
    }
}

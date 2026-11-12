// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::fmt::Write;

use p4kx_audio::Loop;

use crate::audio::music_clip_features::MusicClipFeatures;

use super::TxtpApproximation;
use super::tree::{Combine, Group, Leaf, Node};

/// The longest text of a comment line, in characters.
const MAX_COMMENT_LEN: usize = 160;

/// Spaces of indentation per nesting level.
const INDENT: usize = 2;

/// The file a WEM was exported as, as the TXTP names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct SoundFile<'a> {
    /// The file name in the export folder.
    pub(super) name: &'a str,
    /// The loop of the WEM in frames, written as `#I`.
    pub(super) loop_points: Option<Loop>,
    /// Whether the file holds only the prefetched start of a streamed WEM.
    pub(super) prefetch_only: bool,
}

/// Returns `text` as one comment line: control characters become `_`, and an overlong text is cut,
/// so a name from the Archive cannot start a line of its own.
pub(super) fn comment_text(text: &str) -> String {
    text.chars()
        .take(MAX_COMMENT_LEN)
        .map(|c| if c.is_control() { '_' } else { c })
        .collect()
}

pub(super) struct RenderedTxtp {
    pub(super) text: String,
    pub(super) approximations: Vec<TxtpApproximation>,
}

impl RenderedTxtp {
    fn note(&mut self, approximation: TxtpApproximation) {
        if !self.approximations.contains(&approximation) {
            self.approximations.push(approximation);
        }
    }

    fn note_music_clips(&mut self, clips: MusicClipFeatures) {
        if clips.timing {
            self.note(TxtpApproximation::OmitMusicClipTiming);
        }
        if clips.posted_events {
            self.note(TxtpApproximation::OmitMusicClipEvents);
        }
        if clips.nonfinite_timing {
            self.note(TxtpApproximation::NonfiniteMusicClipTiming);
        }
    }

    fn insert_notes(&mut self, offset: usize) {
        if self.approximations.is_empty() {
            return;
        }
        let mut notes = String::new();
        for approximation in &self.approximations {
            notes.push_str("# approximation: ");
            notes.push_str(approximation.code());
            notes.push('\n');
        }
        self.text.insert_str(offset, &notes);
    }
}

/// Writes the TXTP of `tree` under the comment lines `header`: each leaf is a file line, each
/// group with several children a `group` line after them, nested as the objects are.
///
/// # Errors
///
/// Returns why the TXTP cannot be written: a sound has no usable file in `files`.
pub(super) fn render(
    tree: &Group,
    header: &[String],
    files: &HashMap<u32, SoundFile<'_>>,
) -> Result<RenderedTxtp, String> {
    let mut rendered = RenderedTxtp {
        text: String::new(),
        approximations: Vec::new(),
    };
    for line in header {
        let _ = writeln!(rendered.text, "# {}", comment_text(line));
    }
    let header_end = rendered.text.len();
    group_lines(&mut rendered, tree, 0, files)?;
    rendered.insert_notes(header_end);
    Ok(rendered)
}

fn group_lines(
    rendered: &mut RenderedTxtp,
    group: &Group,
    depth: usize,
    files: &HashMap<u32, SoundFile<'_>>,
) -> Result<(), String> {
    let [only] = group.children.as_slice() else {
        for child in &group.children {
            match child {
                Node::Leaf(leaf) => leaf_line(rendered, leaf, depth + 1, files)?,
                Node::Group(inner) => group_lines(rendered, inner, depth + 1, files)?,
            }
        }
        group_line(rendered, group, depth);
        return Ok(());
    };
    match only {
        Node::Leaf(leaf) => leaf_line(rendered, leaf, depth, files),
        Node::Group(inner) => group_lines(rendered, inner, depth, files),
    }
}

fn leaf_line(
    rendered: &mut RenderedTxtp,
    leaf: &Leaf,
    depth: usize,
    files: &HashMap<u32, SoundFile<'_>>,
) -> Result<(), String> {
    let file = leaf
        .wem_id
        .and_then(|id| files.get(&id))
        .ok_or_else(|| format!("{} {} has no file", leaf.kind.label(), leaf.source_id))?;
    if file.prefetch_only {
        return Err(format!(
            "{} {}: only its start was written, the rest is streamed",
            leaf.kind.label(),
            leaf.source_id
        ));
    }
    rendered.note_music_clips(leaf.music_clip_features);
    let text = &mut rendered.text;
    let _ = write!(text, "{:indent$}{}", "", file.name, indent = depth * INDENT);
    if let Some(points) = file.loop_points {
        let _ = write!(text, " #I {} {}", points.start, points.end);
    }
    text.push('\n');
    Ok(())
}

fn group_line(rendered: &mut RenderedTxtp, group: &Group, depth: usize) {
    let count = group.children.len();
    let (kind, selection, mix) = match group.combine {
        Combine::Layers => ('L', "", " #@layer-v"),
        Combine::Segments => ('S', "", ""),
        Combine::Select => {
            rendered.note(TxtpApproximation::SelectFirstAlternative);
            ('R', ">1", "")
        }
    };
    let text = &mut rendered.text;
    let indent = depth * INDENT;
    let _ = write!(
        text,
        "{:indent$}# {} {}",
        "",
        group.step.kind.name(),
        group.step.id
    );
    if group.combine == Combine::Select {
        let _ = write!(
            text,
            ": plays the first of {count}, change >1 to pick another"
        );
    }
    let _ = writeln!(
        text,
        "\n{:indent$}group = -{kind}{count}{selection}{mix}",
        ""
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::{NodeKind, PathStep, SourceKind};

    fn render_text(
        tree: &Group,
        header: &[String],
        files: &HashMap<u32, SoundFile<'_>>,
    ) -> Result<String, String> {
        render(tree, header, files).map(|rendered| rendered.text)
    }

    fn leaf(wem_id: u32) -> Node {
        Node::Leaf(Leaf {
            wem_id: Some(wem_id),
            source_id: wem_id,
            kind: SourceKind::Embedded,
            music_clip_features: MusicClipFeatures::default(),
        })
    }

    fn group(kind: NodeKind, id: u32, children: Vec<Node>) -> Group {
        Group {
            step: PathStep { kind, id },
            combine: match kind {
                NodeKind::Sequence => Combine::Segments,
                NodeKind::Random => Combine::Select,
                _ => Combine::Layers,
            },
            children,
        }
    }

    fn files() -> HashMap<u32, SoundFile<'static>> {
        HashMap::from([
            (
                1,
                SoundFile {
                    name: "a.ogg",
                    loop_points: None,
                    prefetch_only: false,
                },
            ),
            (
                2,
                SoundFile {
                    name: "b.ogg",
                    loop_points: Some(Loop { start: 10, end: 90 }),
                    prefetch_only: false,
                },
            ),
        ])
    }

    #[test]
    fn a_group_of_one_child_is_the_child() {
        let tree = group(
            NodeKind::Event,
            5,
            vec![Node::Group(group(NodeKind::Action, 6, vec![leaf(1)]))],
        );

        assert_eq!(render_text(&tree, &[], &files()).as_deref(), Ok("a.ogg\n"));
    }

    #[test]
    fn groups_nest_after_their_children() {
        let inner = group(NodeKind::Random, 7, vec![leaf(1), leaf(2)]);
        let tree = group(NodeKind::Event, 5, vec![leaf(1), Node::Group(inner)]);

        let text = render_text(&tree, &["x".to_owned()], &files());

        assert_eq!(
            text.as_deref(),
            Ok("# x
# approximation: select-first-alternative
  a.ogg
    a.ogg
    b.ogg #I 10 90
  # random 7: plays the first of 2, change >1 to pick another
  group = -R2>1
# event 5
group = -L2 #@layer-v
")
        );
    }

    #[test]
    fn a_sequence_is_a_segment_group() {
        let tree = group(NodeKind::Sequence, 9, vec![leaf(1), leaf(2)]);

        let text = render_text(&tree, &[], &files()).expect("renders");

        assert!(
            text.contains(
                "# sequence 9
group = -S2
"
            ),
            "{text}"
        );
    }

    #[test]
    fn a_sound_without_a_file_fails_the_render() {
        let tree = group(NodeKind::Event, 5, vec![leaf(1), leaf(3)]);

        assert_eq!(
            render_text(&tree, &[], &files()),
            Err("embedded source 3 has no file".to_owned())
        );
    }

    #[test]
    fn a_name_cannot_start_a_line_of_its_own() {
        assert_eq!(comment_text("a\nb\r c"), "a_b_ c");
        assert_eq!(comment_text(&"x".repeat(500)).len(), MAX_COMMENT_LEN);
    }
}

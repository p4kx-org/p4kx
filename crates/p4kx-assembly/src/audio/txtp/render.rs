// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::fmt::Write;
use std::ops::Range;
use std::slice;

use p4kx_audio::Loop;

use crate::audio::music_clip_features::MusicClipFeatures;
use crate::audio::playback_features::{PlaybackFeature, PlaybackFeatures};
use crate::audio::{ExportLoopOrigin, ExportedWemOrigin};

use super::TxtpApproximation;
use super::tree::{Combine, Group, Leaf, Node, Tree};

/// The longest text of a comment line, in characters.
const MAX_COMMENT_LEN: usize = 160;

/// Spaces of indentation per nesting level.
const INDENT: usize = 2;

/// The file a WEM was exported as, as the TXTP names it.
#[derive(Debug, Clone, Copy)]
pub(super) struct SoundFile<'a> {
    /// The file name in the export folder.
    pub(super) name: &'a str,
    /// The loop of the WEM in frames, written as `#I`.
    pub(super) loop_points: Option<Loop>,
    pub(super) loop_origin: Option<&'a ExportLoopOrigin>,
    pub(super) media_origin: Option<&'a ExportedWemOrigin>,
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

pub(super) struct RenderedLoop<'a> {
    pub(super) command: usize,
    pub(super) points: Loop,
    pub(super) start: Range<usize>,
    pub(super) end: Range<usize>,
    pub(super) source: Option<&'a ExportLoopOrigin>,
}

pub(super) struct RenderedCommand<'a> {
    pub(super) variant: usize,
    pub(super) media: Option<&'a ExportedWemOrigin>,
}

pub(super) struct RenderedTxtp<'a> {
    pub(super) text: String,
    pub(super) approximations: Vec<TxtpApproximation>,
    pub(super) approximation_ranges: Vec<Range<usize>>,
    pub(super) loops: Vec<RenderedLoop<'a>>,
    pub(super) selection: Option<Range<usize>>,
    pub(super) commands: Vec<RenderedCommand<'a>>,
}

impl RenderedTxtp<'_> {
    pub(super) fn command_loops(&self) -> Option<Vec<Option<Loop>>> {
        if self.loops.is_empty() {
            return None;
        }
        let mut markers = self.loops.iter().peekable();
        Some(
            (0..self.commands.len())
                .map(|command| {
                    markers
                        .next_if(|entry| entry.command == command)
                        .map(|entry| entry.points)
                })
                .collect(),
        )
    }
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

    fn note_playback_features(&mut self, features: PlaybackFeatures) {
        for (feature, code) in [
            (
                PlaybackFeature::MusicFades,
                TxtpApproximation::OmitMusicTransitionFades,
            ),
            (
                PlaybackFeature::MusicTransitionSegments,
                TxtpApproximation::OmitMusicTransitionSegments,
            ),
            (
                PlaybackFeature::MusicTransitionScheduling,
                TxtpApproximation::OmitMusicTransitionScheduling,
            ),
            (
                PlaybackFeature::MusicStingerScheduling,
                TxtpApproximation::OmitMusicStingerScheduling,
            ),
            (
                PlaybackFeature::Loops,
                TxtpApproximation::OmitContainerLoops,
            ),
            (
                PlaybackFeature::Conditions,
                TxtpApproximation::OmitContainerConditions,
            ),
            (
                PlaybackFeature::Automation,
                TxtpApproximation::OmitMusicTrackAutomation,
            ),
            (
                PlaybackFeature::ContainerTransitions,
                TxtpApproximation::OmitContainerTransitions,
            ),
            (
                PlaybackFeature::BlendCurves,
                TxtpApproximation::OmitBlendCurves,
            ),
        ] {
            if features.contains(feature) {
                self.note(code);
            }
        }
    }

    fn insert_notes(&mut self, offset: usize) {
        if self.approximations.is_empty() {
            return;
        }
        let mut notes = String::new();
        for approximation in &self.approximations {
            notes.push_str("# approximation: ");
            let start = offset + notes.len();
            notes.push_str(approximation.code());
            self.approximation_ranges.push(start..offset + notes.len());
            notes.push('\n');
        }
        self.text.insert_str(offset, &notes);
        for markers in &mut self.loops {
            markers.start = markers.start.start + notes.len()..markers.start.end + notes.len();
            markers.end = markers.end.start + notes.len()..markers.end.end + notes.len();
        }
    }
}

/// Writes the TXTP of `tree` under the comment lines `header`: each leaf is a file line, each
/// group with several children a `group` line after them, nested as the objects are.
///
/// # Errors
///
/// Returns why the TXTP cannot be written: a sound has no usable file in `files`.
pub(super) fn render<'a>(
    tree: &Tree,
    header: &[String],
    files: &HashMap<u32, SoundFile<'a>>,
) -> Result<RenderedTxtp<'a>, String> {
    let mut rendered = RenderedTxtp {
        text: String::new(),
        approximations: Vec::new(),
        approximation_ranges: Vec::new(),
        loops: Vec::new(),
        selection: None,
        commands: Vec::new(),
    };
    for line in header {
        let _ = writeln!(rendered.text, "# {}", comment_text(line));
    }
    let header_end = rendered.text.len();
    group_lines(&mut rendered, tree, files)?;
    rendered.insert_notes(header_end);
    Ok(rendered)
}

struct GroupCursor<'a> {
    group: &'a Group,
    children: slice::Iter<'a, Node>,
    depth: usize,
    first_command: usize,
}

impl<'a> GroupCursor<'a> {
    fn new(group: &'a Group, depth: usize, first_command: usize) -> Self {
        Self {
            group,
            children: group.children.iter(),
            depth,
            first_command,
        }
    }

    const fn emits_group(&self) -> bool {
        self.group.children.len() != 1
    }
}

fn group_lines<'a>(
    rendered: &mut RenderedTxtp<'a>,
    tree: &Tree,
    files: &HashMap<u32, SoundFile<'a>>,
) -> Result<(), String> {
    let root = tree
        .group(Tree::ROOT)
        .ok_or("the TXTP root group is missing")?;
    let mut groups = vec![GroupCursor::new(root, 0, 0)];
    while let Some(cursor) = groups.last_mut() {
        let depth = cursor.depth + usize::from(cursor.emits_group());
        match cursor.children.next() {
            Some(Node::Leaf(leaf)) => leaf_line(rendered, leaf, depth, files)?,
            Some(Node::Group(id)) => {
                let group = tree.group(*id).ok_or("a TXTP child group is missing")?;
                groups.push(GroupCursor::new(group, depth, rendered.commands.len()));
            }
            None => {
                if cursor.emits_group() {
                    group_line(rendered, cursor.group, cursor.depth, cursor.first_command);
                }
                groups.pop();
            }
        }
    }
    Ok(())
}

fn leaf_line<'a>(
    rendered: &mut RenderedTxtp<'a>,
    leaf: &Leaf,
    depth: usize,
    files: &HashMap<u32, SoundFile<'a>>,
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
    rendered.note_playback_features(leaf.playback_features);
    let text = &mut rendered.text;
    let _ = write!(text, "{:indent$}{}", "", file.name, indent = depth * INDENT);
    if let Some(points) = file.loop_points {
        text.push_str(" #I ");
        let start = number(text, points.start);
        text.push(' ');
        let end = number(text, points.end);
        rendered.loops.push(RenderedLoop {
            command: rendered.commands.len(),
            points,
            start,
            end,
            source: file.loop_origin,
        });
    }
    rendered.commands.push(RenderedCommand {
        variant: leaf.variant,
        media: file.media_origin,
    });
    text.push('\n');
    Ok(())
}

fn number(text: &mut String, value: u64) -> Range<usize> {
    let start = text.len();
    let _ = write!(text, "{value}");
    start..text.len()
}

fn group_line(rendered: &mut RenderedTxtp<'_>, group: &Group, depth: usize, first_command: usize) {
    let count = group.children.len();
    let (kind, selection, mix) = match group.combine {
        Combine::Layers => ('L', "", " #@layer-v"),
        Combine::Segments => ('S', "", ""),
        Combine::Select => {
            if rendered.selection.is_none() {
                rendered.selection = Some(first_command..rendered.commands.len());
            }
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
#[path = "render_tests.rs"]
mod tests;

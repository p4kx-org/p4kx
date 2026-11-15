// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_audio::Loop;

use super::super::tree::{Combine, Group, Leaf, Node};
use super::{MAX_COMMENT_LEN, SoundFile, comment_text, render};
use crate::audio::music_clip_features::MusicClipFeatures;
use crate::audio::playback_features::PlaybackFeatures;
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
        playback_features: PlaybackFeatures::default(),
        source: None,
    })
}

fn group(kind: NodeKind, id: u32, children: Vec<Node>) -> Group {
    Group {
        step: PathStep { kind, id },
        source: None,
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
                loop_origin: None,
                media_origin: None,
                prefetch_only: false,
            },
        ),
        (
            2,
            SoundFile {
                name: "b.ogg",
                loop_points: Some(Loop { start: 10, end: 90 }),
                loop_origin: None,
                media_origin: None,
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

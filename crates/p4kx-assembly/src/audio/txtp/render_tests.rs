// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::Loop;

use super::super::tree::{GroupId, Leaf, Node, Tree};
use super::{MAX_COMMENT_LEN, SoundFile, comment_text, render};
use crate::audio::music_clip_features::MusicClipFeatures;
use crate::audio::playback_features::PlaybackFeatures;
use crate::audio::{NodeKind, PathStep, SourceKind};

fn render_text(
    tree: &Tree,
    header: &[String],
    files: &[Option<SoundFile<'_>>],
) -> Result<String, String> {
    render(tree, header, files)
        .map(|rendered| rendered.text)
        .map_err(|error| error.reason)
}

fn sound(wem_id: u32) -> Leaf {
    Leaf {
        source_id: wem_id,
        kind: SourceKind::Embedded,
        music_clip_features: MusicClipFeatures::default(),
        playback_features: PlaybackFeatures::default(),
        variant: usize::try_from(wem_id).expect("a fixture position"),
    }
}

fn leaf(wem_id: u32) -> Node {
    Node::Leaf(sound(wem_id))
}

fn add_children(tree: &mut Tree, group: GroupId, children: Vec<Node>) {
    for child in children {
        tree.add_child(group, child)
            .expect("a defined fixture group");
    }
}

fn group(tree: &mut Tree, kind: NodeKind, id: u32, children: Vec<Node>) -> Node {
    let id = tree.add_group(PathStep { kind, id });
    add_children(tree, id, children);
    Node::Group(id)
}

fn tree(kind: NodeKind, id: u32, children: Vec<Node>) -> Tree {
    let mut tree = Tree::new(PathStep { kind, id });
    add_children(&mut tree, Tree::ROOT, children);
    tree
}

fn files() -> [Option<SoundFile<'static>>; 3] {
    [
        None,
        Some(SoundFile {
            name: "a.ogg",
            loop_points: None,
            loop_origin: None,
            media_origin: None,
            prefetch_only: false,
        }),
        Some(SoundFile {
            name: "b.ogg",
            loop_points: Some(Loop { start: 10, end: 90 }),
            loop_origin: None,
            media_origin: None,
            prefetch_only: false,
        }),
    ]
}

#[test]
fn a_group_of_one_child_is_the_child() {
    let mut tree = tree(NodeKind::Event, 5, Vec::new());
    let action = group(&mut tree, NodeKind::Action, 6, vec![leaf(1)]);
    add_children(&mut tree, Tree::ROOT, vec![action]);

    assert_eq!(render_text(&tree, &[], &files()).as_deref(), Ok("a.ogg\n"));
}

#[test]
fn groups_nest_after_their_children() {
    let mut tree = tree(NodeKind::Event, 5, Vec::new());
    let inner = group(&mut tree, NodeKind::Random, 7, vec![leaf(1), leaf(2)]);
    add_children(&mut tree, Tree::ROOT, vec![leaf(1), inner]);

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
    let tree = tree(NodeKind::Sequence, 9, vec![leaf(1), leaf(2)]);

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
    let tree = tree(NodeKind::Event, 5, vec![leaf(1), leaf(3)]);

    assert_eq!(
        render_text(&tree, &[], &files()),
        Err("embedded source 3 has no file".to_owned())
    );
}

#[test]
fn a_deep_single_child_path_builds_renders_and_drops() {
    let mut tree = tree(NodeKind::Event, 0, Vec::new());
    let path: Vec<_> = (1..=8_192)
        .map(|id| PathStep {
            kind: NodeKind::Sequence,
            id,
        })
        .collect();
    tree.insert(Tree::ROOT, &path, 0, sound(1))
        .expect("the complete fixture path");
    assert_eq!(render_text(&tree, &[], &files()).as_deref(), Ok("a.ogg\n"));
    drop(tree);
}

#[test]
fn a_name_cannot_start_a_line_of_its_own() {
    assert_eq!(comment_text("a\nb\r c"), "a_b_ c");
    assert_eq!(comment_text(&"x".repeat(500)).len(), MAX_COMMENT_LEN);
}

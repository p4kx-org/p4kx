// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestContainer, TestElement, TestEntry};

use super::{child, placed, read, root, system};
use crate::starmap_containers::Limits;

const TILE: &str = "Data/ObjectContainers/PU/Starchitect/Tile/example.socpak";

#[test]
fn a_declared_external_true_reaches_the_same_tile_as_external_one() {
    reaches_tile(
        "TrUe",
        "ObjectContainers/PU/Starchitect/Tile/example.socpak",
    );
}

#[test]
fn an_extensionless_object_container_relative_path_reaches_the_tile() {
    reaches_tile("1", "PU/Starchitect/Tile/example");
}

fn reaches_tile(external: &str, name: &str) {
    let dir = tempfile::tempdir().expect("a synthetic directory");
    let child_link = TestElement::new("Child")
        .attribute("external", external)
        .attribute("name", name)
        .attribute("pos", "10,0,0");
    let mut entries = system(&root(vec![child_link]), &[]);
    let xml = root(vec![child("3,0,0", 1)]);
    let tile = TestContainer::new()
        .entry(TestEntry::stored("example.xml", xml.into_bytes()))
        .entry(TestEntry::stored("example.soc", Vec::new()))
        .build();
    entries.push(TestEntry::stored(TILE, tile));
    let placements = read(dir.path(), entries, Limits::default());
    assert_eq!(
        placed(&placements, 1),
        Some([13.0, 0.0, 0.0]),
        "the declared tile is placed"
    );
    assert_eq!(
        placements.counts.missing, 0,
        "the target is present in the Archive"
    );
}

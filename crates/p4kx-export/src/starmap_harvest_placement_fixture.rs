// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, TestContainer, TestEntry};

const FOLDER: &str = "Data\\ObjectContainers\\PU\\system\\t\\";
pub(super) const PLANET: &str = "10000000-0000-4000-8000-000000000001";
pub(super) const CLAIM: &str = "10000000-0000-4000-8000-000000000002";
const LOCATION: &str = "10000000-0000-4000-8000-000000000003";
pub(super) const PRESET: &str = "10000000-0000-4000-8000-000000000004";

pub(super) fn fixture(directory: &Path) -> Result<Archive, Box<dyn std::error::Error>> {
    let path = directory.join("Data.p4k");
    let tile = format!("{FOLDER}tile.socpak");
    let unselected = format!("{FOLDER}unselected.socpak");
    let unreadable = format!("{FOLDER}unreadable-reached.socpak");
    let root = format!(
        r#"<ObjectContainer><ChildObjectContainers>
        <Child starMapRecord="{PLANET}" pos="10,0,0"><ChildObjectContainers>
            <Child starMapRecord="{CLAIM}" pos="1,0,0"><ChildObjectContainers>
                <Child name="{tile}" external="1" pos="1,0,0"/>
                <Child name="{tile}" external="1" pos="2,0,0"/>
                <Child name="{unselected}" external="1" pos="3,0,0"/>
                <Child name="{unreadable}" external="1" pos="4,0,0"/>
            </ChildObjectContainers></Child>
        </ChildObjectContainers></Child>
        <Child locationType="3" guid="{LOCATION}" hasHierarchicalContextData="1"/>
        </ChildObjectContainers></ObjectContainer>"#,
    );
    let hcd = format!(
        r#"<HierarchicalContextData __version="2" preset="{PRESET}" presetRecordName="Synthetic"/>"#
    );
    let entity = format!(r#"<Entity EntityCryGUID="{LOCATION}"/>"#);
    let marker = format!("\0preset\0{PRESET}\0");
    let unselected_marker = ["\0preset\0", "10000000-0000-4000-8000-000000000009", "\0"].concat();
    let broken = || TestEntry::zstd("bad.soc", "expected source").with_payload(b"bad".to_vec());
    ArchiveBuilder::new()
        .entry(container(
            "tsystem",
            &root,
            [
                TestEntry::stored(format!("hcd/{LOCATION}.xml"), hcd.as_bytes()),
                TestEntry::stored("entdata/target.entxml", entity.as_bytes()),
            ],
        ))
        .entry(container(
            "tile",
            "<ObjectContainer/>",
            [TestEntry::stored("tile.soc", marker.as_bytes())],
        ))
        .entry(container(
            "unreached",
            "<ObjectContainer/>",
            [TestEntry::stored("tile.soc", marker.as_bytes())],
        ))
        .entry(container(
            "unselected",
            "<ObjectContainer/>",
            [TestEntry::stored(
                "unselected.soc",
                unselected_marker.as_bytes(),
            )],
        ))
        .entry(container(
            "unreadable-reached",
            "<ObjectContainer/>",
            [broken()],
        ))
        .entry(container(
            "unreadable-detached",
            "<ObjectContainer/>",
            [broken()],
        ))
        .write_to(&path)?;
    Ok(Archive::open(path)?)
}

fn container(name: &str, xml: &str, members: impl IntoIterator<Item = TestEntry>) -> TestEntry {
    let archive =
        TestContainer::new().entry(TestEntry::stored(format!("{name}.xml"), xml.as_bytes()));
    let archive = members.into_iter().fold(archive, TestContainer::entry);
    TestEntry::stored(format!("{FOLDER}{name}.socpak"), archive.build())
}

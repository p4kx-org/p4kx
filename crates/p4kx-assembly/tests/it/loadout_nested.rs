// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Nested Loadouts of empty ports: what is reported, and how deep and wide they may grow.

use std::collections::HashMap;
use std::error::Error;

use p4kx_assembly::{
    Attachment, EntityNode, LoadoutError, LoadoutFileError, LoadoutFiles, LoadoutOrigin,
    LoadoutResolver, MAX_LOADOUT_DEPTH, PortNode, PortSource,
};
use p4kx_dataforge::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestLoadout, TestLoadoutEntry, entity_schema};

type TestResult = Result<(), Box<dyn Error>>;

struct Files(HashMap<String, Vec<u8>>);

impl LoadoutFiles for Files {
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        self.0
            .get(&path.to_ascii_lowercase())
            .cloned()
            .ok_or(LoadoutFileError::NotFound)
    }
}

fn files(entries: &[(&str, &[u8])]) -> Files {
    Files(
        entries
            .iter()
            .map(|(path, bytes)| (path.to_ascii_lowercase(), bytes.to_vec()))
            .collect(),
    )
}

fn resolve(
    entities: &[TestEntity],
    files: Option<&Files>,
    class_name: &str,
) -> Result<EntityNode, Box<dyn Error>> {
    let builder = entities
        .iter()
        .fold(entity_schema(DataForgeBuilder::new()), |builder, entity| {
            builder.record(entity.record())
        });
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let mut resolver = LoadoutResolver::new(&forge);
    if let Some(files) = files {
        resolver = resolver.with_files(files);
    }
    let entity = resolver.entities().find(class_name)?;
    Ok(resolver.resolve(&entity)?)
}

fn entity(guid: u128, class_name: &str) -> TestEntity {
    TestEntity::new(guid, class_name, format!("entities/{class_name}.xml"))
}

fn door(loadout: Vec<TestLoadoutEntry>) -> TestEntity {
    entity(0x1, "Door").loadout(TestLoadout::Manual(loadout))
}

fn decal() -> TestEntity {
    entity(0x2, "Decal_Red")
}

fn nest(levels: usize) -> TestLoadout {
    if levels == 0 {
        return TestLoadout::Manual(vec![TestLoadoutEntry::class("leaf", "Decal_Red")]);
    }
    TestLoadout::Manual(vec![TestLoadoutEntry::empty("n").loadout(nest(levels - 1))])
}

fn issue_texts(node: &EntityNode) -> Vec<String> {
    node.issues().iter().map(ToString::to_string).collect()
}

fn deepest(node: &EntityNode) -> usize {
    node.walk()
        .iter()
        .map(|(path, _)| path.split('/').count())
        .max()
        .unwrap_or(0)
}

fn nested_depth(port: &PortNode) -> usize {
    1 + port.nested().iter().map(nested_depth).max().unwrap_or(0)
}

#[test]
fn an_unreadable_nested_xml_loadout_is_an_issue_of_the_entity() -> TestResult {
    let entities = [
        door(vec![
            TestLoadoutEntry::empty("body")
                .loadout(TestLoadout::Xml("Loadouts/Gone.xml".to_owned())),
        ]),
        decal(),
    ];

    let missing = resolve(&entities, Some(&files(&[])), "Door")?;
    let without_files = resolve(&entities, None, "Door")?;

    assert_eq!(
        issue_texts(&missing),
        ["the XML Loadout Loadouts/Gone.xml cannot be read: there is no such file"]
    );
    assert_eq!(
        issue_texts(&without_files),
        ["the XML Loadout Loadouts/Gone.xml is not read without its Archive"]
    );
    assert!(
        missing
            .ports()
            .first()
            .is_some_and(|p| p.nested().is_empty())
    );
    Ok(())
}

#[test]
fn the_issues_of_a_nested_xml_loadout_are_reported_and_its_items_kept() -> TestResult {
    let entities = [
        door(vec![
            TestLoadoutEntry::empty("body")
                .loadout(TestLoadout::Xml("Loadouts/Body.xml".to_owned())),
        ]),
        decal(),
    ];
    let body = b"<Loadout><Loadouts><Loadout loadout=\"Loadouts/Gone.xml\"/></Loadouts>\
        <Items><Item portName=\"decal\" itemName=\"Decal_Red\"/></Items></Loadout>";
    let files = files(&[("loadouts/body.xml", body)]);

    let tree = resolve(&entities, Some(&files), "Door")?;

    let port = tree.ports().first().ok_or("no port")?;
    assert_eq!(port.nested().len(), 1, "the nested item is kept");
    assert_eq!(
        issue_texts(&tree),
        ["the XML Loadout Loadouts/Gone.xml cannot be read: there is no such file"]
    );
    Ok(())
}

#[test]
fn a_wide_empty_port_retains_all_nested_loadout_entries() -> TestResult {
    const NESTED_PORTS: usize = 65_537;
    let nested = (0..NESTED_PORTS)
        .map(|index| TestLoadoutEntry::empty(format!("p{index}")))
        .collect();
    let entities = [
        door(vec![
            TestLoadoutEntry::empty("body").loadout(TestLoadout::Manual(nested)),
        ]),
        decal(),
    ];

    let tree = resolve(&entities, None, "Door")?;

    assert!(tree.issues().is_empty(), "the nested loadout is complete");
    let port = tree.ports().first().ok_or("the original parent port")?;
    assert_eq!(
        port.nested().len(),
        NESTED_PORTS,
        "all nested ports survive"
    );
    let last = port.nested().last().ok_or("the last nested port")?;
    assert_eq!(last.name(), "p65536", "the last original nested slot");
    assert_eq!(
        last.source(),
        PortSource::Override,
        "original nested source"
    );
    assert!(
        matches!(last.origin(), Some(LoadoutOrigin::Record(path)) if path.as_ref() == tree.record_path()),
        "the last nested port keeps the original owner Record",
    );
    Ok(())
}

#[test]
fn a_nested_loadout_deeper_than_the_limit_is_cut_and_reported() -> TestResult {
    let levels = MAX_LOADOUT_DEPTH;
    let mut body = String::from("<Loadout>");
    for _ in 0..levels {
        body.push_str("<Items><Item portName=\"n\" itemName=\"\">");
    }
    for _ in 0..levels {
        body.push_str("</Item></Items>");
    }
    body.push_str("</Loadout>");
    let entities = [
        entity(0x3, "Outer").loadout(TestLoadout::Manual(vec![TestLoadoutEntry::class(
            "in", "Door",
        )])),
        door(vec![
            TestLoadoutEntry::empty("body")
                .loadout(TestLoadout::Xml("Loadouts/Deep.xml".to_owned())),
        ]),
        decal(),
    ];
    let files = files(&[("loadouts/deep.xml", body.as_bytes())]);

    let tree = resolve(&entities, Some(&files), "Outer")?;

    let inner = match tree.ports().first().map(PortNode::attachment) {
        Some(Attachment::Entity(inner)) => inner,
        other => return Err(format!("{other:?}").into()),
    };
    assert_eq!(inner.issues(), [LoadoutError::TooDeep], "reported");
    let body = inner.ports().first().ok_or("no port")?;
    assert!(nested_depth(body) <= MAX_LOADOUT_DEPTH, "cut at the limit");
    Ok(())
}

#[test]
fn attached_entities_and_nested_levels_share_one_depth_budget() -> TestResult {
    let links = MAX_LOADOUT_DEPTH - 2;
    let mut entities = vec![decal()];
    for index in 0..links {
        let guid = 0x10_0000 + u128::try_from(index)?;
        entities.push(
            entity(guid, &format!("Link{index}")).loadout(TestLoadout::Manual(vec![
                TestLoadoutEntry::class("next", format!("Link{}", index + 1)),
                TestLoadoutEntry::empty("body").loadout(nest(MAX_LOADOUT_DEPTH - 2)),
            ])),
        );
    }
    entities.push(entity(0x20_0000, &format!("Link{links}")));

    let tree = resolve(&entities, None, "Link0")?;

    assert!(
        deepest(&tree) <= MAX_LOADOUT_DEPTH + 1,
        "{} levels",
        deepest(&tree)
    );
    Ok(())
}

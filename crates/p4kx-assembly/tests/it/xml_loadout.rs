// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! XML Loadouts: files a Record names instead of entries, read through `LoadoutFiles`.

use std::collections::HashMap;
use std::error::Error;
use std::fmt::Write as _;

use p4kx_assembly::{
    Attachment, EntityNode, LoadoutError, LoadoutFileError, LoadoutFiles, LoadoutResolver,
    PortNode, PortSource,
};
use p4kx_dataforge::DataForge;
use p4kx_testkit::{
    CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, TestElement, TestEntity, TestLoadout,
    TestLoadoutEntry, entity_schema,
};

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

fn files(entries: &[(&str, Vec<u8>)]) -> Files {
    Files(
        entries
            .iter()
            .map(|(path, bytes)| (path.to_ascii_lowercase(), bytes.clone()))
            .collect(),
    )
}

fn database(loadout: TestLoadout) -> DataForgeBuilder {
    let entities = [
        TestEntity::new(0x1, "Door", "entities/door.xml").loadout(loadout),
        TestEntity::new(0x2, "Decal_Red", "entities/decal_red.xml"),
        TestEntity::new(0x3, "Decal_Blue", "entities/decal_blue.xml"),
        TestEntity::new(0x4, "Panel", "entities/panel.xml"),
    ];
    entities
        .iter()
        .fold(entity_schema(DataForgeBuilder::new()), |builder, entity| {
            builder.record(entity.record())
        })
}

fn resolve(
    builder: &DataForgeBuilder,
    files: Option<&Files>,
) -> Result<EntityNode, Box<dyn Error>> {
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let mut resolver = LoadoutResolver::new(&forge);
    if let Some(files) = files {
        resolver = resolver.with_files(files);
    }
    let door = resolver.entities().find("Door")?;
    Ok(resolver.resolve(&door)?)
}

fn item(port: &str, class_name: &str) -> TestElement {
    TestElement::new("Item")
        .attribute("portName", port)
        .attribute("itemName", class_name)
}

fn cryxml(element: &TestElement) -> Vec<u8> {
    element.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}

fn lines(node: &EntityNode) -> Vec<String> {
    node.walk()
        .into_iter()
        .map(|(path, port)| match port.attachment() {
            Attachment::Entity(child) => format!("{path} {}", child.class_name()),
            Attachment::Empty => format!("{path} -"),
            Attachment::Unresolved(unresolved) => format!("{path} ! {}", unresolved.error().kind()),
        })
        .collect()
}

#[test]
fn a_cryxml_loadout_with_nested_items_is_read_from_the_files() -> TestResult {
    let builder = database(TestLoadout::Xml("Scripts/Loadouts/Door.xml".to_owned()));
    let door = TestElement::new("Loadout").child(
        TestElement::new("Items").child(
            item("hardpoint_decal", "decal_red")
                .child(TestElement::new("Items").child(item("hardpoint_panel", "Panel"))),
        ),
    );
    let files = files(&[("scripts/loadouts/door.xml", cryxml(&door))]);
    let tree = resolve(&builder, Some(&files))?;
    assert!(tree.issues().is_empty(), "issues: {:?}", tree.issues());
    assert_eq!(
        lines(&tree),
        [
            "hardpoint_decal Decal_Red",
            "hardpoint_decal/hardpoint_panel Panel"
        ],
        "class names ignore case; nested items are the child's Loadout"
    );
    let decal = tree.ports().first().ok_or("no port")?;
    assert_eq!(
        decal.source(),
        PortSource::Loadout,
        "the Entity's own Loadout"
    );
    Ok(())
}

#[test]
fn an_item_without_item_name_keeps_its_port_empty_and_lists_the_nested_items() -> TestResult {
    let builder = database(TestLoadout::Xml("Loadouts/Door.xml".to_owned()));
    let door = b"<Loadout><Items>\
        <Item portName=\"body\" itemName=\"\"><Items>\
        <Item portName=\"decal\" itemName=\"Decal_Red\"/>\
        <Item portName=\"slot\" itemName=\"\"><Items>\
        <Item portName=\"panel\" itemName=\"Panel\"/></Items></Item>\
        </Items></Item></Items></Loadout>"
        .to_vec();
    let files = files(&[("loadouts/door.xml", door)]);

    let tree = resolve(&builder, Some(&files))?;

    assert_eq!(
        lines(&tree),
        [
            "body -",
            "body/decal Decal_Red",
            "body/slot -",
            "body/slot/panel Panel"
        ],
        "the nested Loadout is kept below the empty port"
    );
    let body = tree.ports().first().ok_or("no port")?;
    assert_eq!(body.nested().len(), 2, "two nested ports");
    assert_eq!(
        body.nested().first().map(PortNode::source),
        Some(PortSource::Override),
        "source"
    );
    Ok(())
}

#[test]
fn a_manual_entry_without_target_keeps_its_nested_entries() -> TestResult {
    let builder = database(TestLoadout::Manual(vec![
        TestLoadoutEntry::empty("body").loadout(TestLoadout::Manual(vec![
            TestLoadoutEntry::class("decal", "Decal_Red"),
        ])),
    ]));

    let tree = resolve(&builder, None)?;

    assert_eq!(lines(&tree), ["body -", "body/decal Decal_Red"]);
    Ok(())
}

#[test]
fn included_loadouts_are_overridden_per_port_by_the_including_one() -> TestResult {
    let builder = database(TestLoadout::Xml("Loadouts/Door.xml".to_owned()));
    let base = b"\xef\xbb\xbf<Loadout><Items>\
        <Item portName=\"left\" itemName=\"Decal_Red\"/>\
        <Item portName=\"right\" itemName=\"Decal_Red\"/>\
        </Items></Loadout>"
        .to_vec();
    let door = b"<Loadout>\
        <Loadouts><Loadout loadout=\"Loadouts/Base.xml\"/></Loadouts>\
        <Items><Item portName=\"right\" itemName=\"Decal_Blue\"/>\
        <Item portName=\"top\" itemName=\"\"/></Items>\
        </Loadout>"
        .to_vec();
    let files = files(&[("loadouts/base.xml", base), ("loadouts/door.xml", door)]);
    let tree = resolve(&builder, Some(&files))?;
    assert_eq!(
        lines(&tree),
        ["left Decal_Red", "right Decal_Blue", "top -"],
        "text XML with a byte order mark, included first"
    );
    Ok(())
}

#[test]
fn an_unreadable_xml_loadout_is_an_issue_of_its_entity() -> TestResult {
    let builder = database(TestLoadout::Xml("Loadouts/Door.xml".to_owned()));
    let looping =
        b"<Loadout><Loadouts><Loadout loadout=\"loadouts/door.xml\"/></Loadouts></Loadout>";
    let cases: [(Option<Files>, &str); 4] = [
        (
            None,
            "the XML Loadout Loadouts/Door.xml is not read without its Archive",
        ),
        (
            Some(files(&[])),
            "the XML Loadout Loadouts/Door.xml cannot be read: there is no such file",
        ),
        (
            Some(files(&[("loadouts/door.xml", looping.to_vec())])),
            "the XML Loadout loadouts/door.xml cannot be read: it includes itself",
        ),
        (
            Some(files(&[("loadouts/door.xml", b"<Other/>".to_vec())])),
            "the XML Loadout Loadouts/Door.xml cannot be read: its root element is Other, not Loadout",
        ),
    ];
    for (files, expected) in cases {
        let tree = resolve(&builder, files.as_ref())?;
        let issues: Vec<String> = tree.issues().iter().map(ToString::to_string).collect();
        assert_eq!(issues, [expected], "one issue");
        assert!(tree.ports().is_empty(), "no ports: {:?}", tree.ports());
    }
    Ok(())
}

#[test]
fn an_xml_loadout_of_an_entry_overrides_the_attached_entity_s_own() -> TestResult {
    let builder = DataForgeBuilder::new();
    let entities = [
        TestEntity::new(0x1, "Door", "entities/door.xml").loadout(TestLoadout::Manual(vec![
            TestLoadoutEntry::class("frame", "Panel")
                .loadout(TestLoadout::Xml("Loadouts/Frame.xml".to_owned())),
        ])),
        TestEntity::new(0x4, "Panel", "entities/panel.xml").loadout(TestLoadout::Manual(vec![
            TestLoadoutEntry::class("sticker", "Decal_Red"),
        ])),
        TestEntity::new(0x2, "Decal_Red", "entities/decal_red.xml"),
        TestEntity::new(0x3, "Decal_Blue", "entities/decal_blue.xml"),
    ];
    let builder = entities
        .iter()
        .fold(entity_schema(builder), |builder, entity| {
            builder.record(entity.record())
        });
    let frame = TestElement::new("Loadout")
        .child(TestElement::new("Items").child(item("sticker", "Decal_Blue")));
    let files = files(&[("loadouts/frame.xml", cryxml(&frame))]);
    let tree = resolve(&builder, Some(&files))?;
    assert_eq!(
        lines(&tree),
        ["frame Panel", "frame/sticker Decal_Blue"],
        "the entry's XML Loadout wins"
    );
    let without = resolve(&builder, None)?;
    let panel = match without.ports().first().map(PortNode::attachment) {
        Some(Attachment::Entity(panel)) => panel.clone(),
        other => return Err(format!("no panel: {other:?}").into()),
    };
    assert!(
        matches!(panel.issues(), [LoadoutError::NoLoadoutFiles { .. }]),
        "the override cannot be read: {:?}",
        panel.issues()
    );
    assert_eq!(
        lines(&panel),
        ["sticker Decal_Red"],
        "the own Loadout stays"
    );
    Ok(())
}

#[test]
fn a_missing_include_is_an_issue_and_the_rest_still_loads() -> TestResult {
    let builder = database(TestLoadout::Xml("Loadouts/Door.xml".to_owned()));
    let door = b"<Loadout>\
        <Loadouts><Loadout loadout=\"Loadouts/Gone.xml\"/></Loadouts>\
        <Items><Item portName=\"left\" itemName=\"Decal_Red\"/></Items>\
        </Loadout>"
        .to_vec();
    let files = files(&[("loadouts/door.xml", door)]);
    let tree = resolve(&builder, Some(&files))?;
    assert_eq!(lines(&tree), ["left Decal_Red"], "the own items load");
    let issues: Vec<String> = tree.issues().iter().map(ToString::to_string).collect();
    assert_eq!(
        issues,
        ["the XML Loadout Loadouts/Gone.xml cannot be read: there is no such file"],
        "the include is reported"
    );
    Ok(())
}

#[test]
fn a_huge_xml_loadout_merges_its_includes_in_linear_time() -> TestResult {
    let count = 200_000;
    let items = |class_name: &str| {
        let mut text = String::from("<Items>");
        for index in 0..count {
            let _infallible = write!(
                text,
                "<Item portName=\"port_{index}\" itemName=\"{class_name}\"/>"
            );
        }
        text.push_str("</Items>");
        text
    };
    let base = format!("<Loadout>{}</Loadout>", items("Decal_Red"));
    let door = format!(
        "<Loadout><Loadouts><Loadout loadout=\"Loadouts/Base.xml\"/></Loadouts>{}</Loadout>",
        items("")
    );
    let files = files(&[
        ("loadouts/base.xml", base.into_bytes()),
        ("loadouts/door.xml", door.into_bytes()),
    ]);
    let builder = database(TestLoadout::Xml("Loadouts/Door.xml".to_owned()));
    let started = std::time::Instant::now();
    let tree = resolve(&builder, Some(&files))?;
    let elapsed = started.elapsed();
    assert_eq!(
        tree.ports().len(),
        p4kx_assembly::MAX_LOADOUT_PORTS,
        "capped after the merge"
    );
    assert!(
        tree.ports()
            .iter()
            .all(|port| port.attachment() == &Attachment::Empty),
        "every port overridden as empty"
    );
    assert!(
        elapsed < std::time::Duration::from_secs(30),
        "200,000 ports merged in {elapsed:?}"
    );
    Ok(())
}

struct Counting {
    files: Files,
    reads: std::sync::atomic::AtomicUsize,
}

impl LoadoutFiles for Counting {
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        self.reads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.files.read(path)
    }
}

#[test]
fn a_wide_loadout_reads_distinct_includes_and_preserves_port_order() -> TestResult {
    const INCLUDES: usize = 257;
    let root = "scripts/loadouts/door.xml";
    let builder = database(TestLoadout::Xml(root.to_owned()));
    let mut loadouts = TestElement::new("Loadouts");
    let mut documents = HashMap::new();
    for number in 0..INCLUDES {
        let path = format!("scripts/loadouts/{number}.xml");
        for _ in 0..2 {
            loadouts =
                loadouts.child(TestElement::new("Loadout").attribute("loadout", path.as_str()));
        }
        let child = TestElement::new("Loadout")
            .child(TestElement::new("Items").child(item(&format!("port_{number}"), "Decal_Red")));
        documents.insert(path, cryxml(&child));
    }
    documents.insert(
        root.to_owned(),
        cryxml(&TestElement::new("Loadout").child(loadouts)),
    );
    let counting = Counting {
        files: Files(documents),
        reads: std::sync::atomic::AtomicUsize::new(0),
    };
    let bytes = builder.build();
    let forge = DataForge::open(&bytes)?;
    let mut resolver = LoadoutResolver::new(&forge).with_files(&counting);
    let record = resolver.entities().find("Door")?;
    let tree = resolver.resolve(&record)?;

    let reads = counting.reads.load(std::sync::atomic::Ordering::Relaxed);
    assert_eq!(reads, INCLUDES + 1, "one read per distinct document");
    assert!(tree.issues().is_empty(), "issues: {:?}", tree.issues());
    let actual: Vec<_> = tree.ports().iter().map(PortNode::name).collect();
    let expected: Vec<_> = (0..INCLUDES)
        .map(|number| format!("port_{number}"))
        .collect();
    assert_eq!(actual, expected, "every included port in source order");
    Ok(())
}

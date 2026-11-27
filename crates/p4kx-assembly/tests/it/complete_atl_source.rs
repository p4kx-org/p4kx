// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::MAX_ATL_LEN;
use p4kx_cryxml::Xml;
use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestElement, TestEntry};

pub(crate) type TestResult = Result<(), Box<dyn std::error::Error>>;
pub(crate) const TRIGGER: &str = "Complete_Trigger";
pub(crate) const LATE: &str = "after_the_256th_event";
pub(crate) const PATH: &str = "Data\\Libs\\GameAudio\\complete.xml";

pub(crate) fn names() -> Vec<String> {
    let mut names: Vec<_> = (0..255).map(|index| format!("Event_{index:03}")).collect();
    names.push("Event_000".to_owned());
    names.push(LATE.to_owned());
    assert_eq!(
        names.len(),
        257,
        "the fixture has 257 original event occurrences"
    );
    names
}

pub(crate) fn inputs(names: &[String]) -> [Vec<u8>; 2] {
    let mut trigger = TestElement::new("ATLTrigger")
        .attribute("atl_name", TRIGGER)
        .child(TestElement::new("WwiseEvent").attribute("wwise_name", ""));
    for (index, name) in names.iter().enumerate() {
        let event = TestElement::new("WwiseEvent").attribute("wwise_name", name);
        let event = if index + 1 == names.len() {
            event.attribute("wwise_localised", "true")
        } else {
            event
        };
        trigger = trigger.child(event);
    }
    let trigger =
        trigger.child(TestElement::new("OtherEvent").attribute("wwise_name", "not_a_wwise_event"));
    let document = TestElement::new("ATLConfig").child(
        TestElement::new("AudioTriggers")
            .child(trigger)
            .child(TestElement::new("ATLTrigger").attribute("atl_name", "Silent")),
    );
    [
        document.to_xml().into_bytes(),
        document.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little),
    ]
}

pub(crate) fn with_archive(
    bytes: &[u8],
    names: &[String],
    check: impl FnOnce(&Archive) -> TestResult,
) -> TestResult {
    use std::io::Read as _;

    let folder = tempfile::tempdir()?;
    let path = folder.path().join("complete-atl.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(PATH, bytes))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = archive
        .entries()
        .iter()
        .find(|entry| entry.path() == PATH)
        .ok_or("the original ATL Entry")?;
    assert!(
        entry.uncompressed_size() < MAX_ATL_LEN,
        "source byte admission is independent"
    );
    let mut original = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut original)?;
    assert_eq!(
        original, bytes,
        "the actual original Entry retains every authored byte"
    );
    original_events(&original, names)?;
    check(&archive)
}

fn original_events(bytes: &[u8], names: &[String]) -> TestResult {
    let text = match p4kx_cryxml::read(bytes)? {
        Xml::Text(text) => text.to_vec(),
        Xml::CryXml(document) => {
            let mut text = Vec::new();
            document.write_xml(&mut text)?;
            text
        }
    };
    let document = roxmltree::Document::parse(str::from_utf8(&text)?)?;
    let root = document.root_element();
    assert_eq!(root.tag_name().name(), "ATLConfig", "genuine source root");
    let trigger = root
        .descendants()
        .find(|node| node.attribute("atl_name") == Some(TRIGGER))
        .ok_or("the original trigger element")?;
    let children: Vec<_> = trigger
        .children()
        .filter(roxmltree::Node::is_element)
        .collect();
    assert_eq!(
        children.len(),
        names.len() + 2,
        "all original event and ignored slots exist"
    );
    let events: Vec<_> = children
        .iter()
        .filter(|node| node.tag_name().name() == "WwiseEvent")
        .filter(|node| {
            node.attribute("wwise_name")
                .is_some_and(|name| !name.is_empty())
        })
        .collect();
    let actual: Vec<_> = events
        .iter()
        .filter_map(|node| node.attribute("wwise_name"))
        .collect();
    assert_eq!(
        actual, names,
        "the original decoder exposes every ordered duplicate and late event"
    );
    assert_eq!(
        events
            .last()
            .and_then(|node| node.attribute("wwise_localised")),
        Some("true"),
        "the actual late event carries the authored localised flag",
    );
    Ok(())
}

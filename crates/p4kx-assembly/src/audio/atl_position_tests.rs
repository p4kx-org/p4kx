// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use p4kx_archive::Archive;
use p4kx_cryxml::{SourceAttributePosition, SourceElementPosition};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, TestAtl, TestElement, TestEntry,
};

use super::{AtlTrigger, WwiseEvent, wwise_id};
use crate::audio::AudioIndex;

type TestResult = Result<(), Box<dyn Error>>;

const TRIGGER: &str = "Play_Observed";
const EVENT: &str = "Play&Event";
const ENTRY: &str = "Data/Libs/GameAudio/A_Winner.xml";

fn definition() -> TestElement {
    TestElement::new("ATLConfig")
        .child(TestElement::new("Ignored"))
        .child(
            TestElement::new("AudioTriggers")
                .child(TestElement::new("ATLTrigger"))
                .child(TestElement::new("Ignored").attribute("atl_name", "ignored"))
                .child(
                    TestElement::new("ATLTrigger")
                        .attribute("other", "prefix")
                        .attribute("atl_name", TRIGGER)
                        .child(TestElement::new("WwiseEvent").attribute("wwise_name", ""))
                        .child(TestElement::new("OtherEvent").attribute("wwise_name", "ignored"))
                        .child(
                            TestElement::new("WwiseEvent")
                                .attribute("wwise_name", EVENT)
                                .attribute("wwise_localised", "TrUe"),
                        )
                        .child(TestElement::new("WwiseEvent"))
                        .child(TestElement::new("WwiseEvent").attribute("wwise_name", EVENT)),
                ),
        )
}

#[test]
fn atl_positions_follow_the_selected_entry_and_original_occurrences() -> TestResult {
    let definition = definition();
    for bytes in [
        definition.to_xml().into_bytes(),
        definition.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little),
    ] {
        let folder = tempfile::tempdir()?;
        let path = folder.path().join("positions.p4k");
        let shadow = TestAtl::new("shadow").trigger(TRIGGER, &["Shadow"]);
        ArchiveBuilder::new()
            .entry(TestEntry::stored(
                "Data/Libs/GameAudio/Z_Shadow.xml",
                shadow.to_xml(),
            ))
            .entry(TestEntry::stored(ENTRY, bytes.as_slice()))
            .write_to(&path)?;
        let archive = Archive::open(&path)?;
        let index = AudioIndex::build(&archive);
        assert!(
            index.issues().is_empty(),
            "ignored declarations are not read errors"
        );
        let trigger = index.trigger(TRIGGER).ok_or("selected original trigger")?;
        let position = index
            .definition_entry_position(trigger)
            .ok_or("winning Entry")?;
        assert_eq!(position, 1, "source order differs from path order");
        let entry = archive
            .entries()
            .get(position)
            .ok_or("original physical Entry")?;
        assert_eq!(
            entry.path(),
            ENTRY,
            "provenance cites the selected physical Entry"
        );
        assert_eq!(
            index.definition_entry_position(&trigger.clone()),
            None,
            "detached trigger cannot bind its positions to the Archive",
        );
        assert_trigger(trigger, &bytes)?;
        assert_definition_source(&index, &archive, &path, trigger, position)?;
    }
    Ok(())
}

fn assert_definition_source(
    index: &AudioIndex,
    archive: &Archive,
    path: &Path,
    trigger: &AtlTrigger,
    position: usize,
) -> TestResult {
    let source = index
        .definition_source(archive, trigger)
        .ok_or("bound original ATL definition")?;
    let entry = archive.entries().get(position).ok_or("original Entry")?;
    assert!(
        std::ptr::eq(source.entry(), entry),
        "the receipt borrows the actual Archive directory Entry"
    );
    assert!(
        index
            .definition_source(&Archive::open(path)?, trigger)
            .is_none(),
        "equal coordinates from a separately admitted input provide no binding"
    );
    Ok(())
}

fn assert_trigger(trigger: &AtlTrigger, bytes: &[u8]) -> TestResult {
    assert_site(
        trigger.position(),
        trigger.name_position(),
        bytes,
        b"atl_name=\"Play_Observed\"",
        (5, 2),
    )?;
    let [first, last] = trigger.events() else {
        return Err("both repeated Wwise events must retain their original order".into());
    };
    for (event, expected) in [(first, (8, 5)), (last, (10, 7))] {
        assert_eq!(event.name(), EVENT, "the original name is decoded");
        assert_eq!(event.id(), wwise_id(EVENT), "the original name is hashed");
        assert_site(
            event.position(),
            event.name_position(),
            bytes,
            b"wwise_name=\"Play&amp;Event\"",
            expected,
        )?;
    }
    assert_ne!(
        first.position(),
        last.position(),
        "the two occurrences retain separate sites"
    );
    assert_localisation(first, last, bytes)
}

fn assert_localisation(first: &WwiseEvent, last: &WwiseEvent, bytes: &[u8]) -> TestResult {
    assert!(
        first.is_localised(),
        "localisation token is ASCII case insensitive"
    );
    assert!(
        !last.is_localised(),
        "an absent localisation flag defaults to false"
    );
    assert_eq!(
        last.localised_position(),
        None,
        "no invented flag coordinate"
    );
    assert_site(
        first.position(),
        first
            .localised_position()
            .ok_or("authored localisation flag")?,
        bytes,
        b"wwise_localised=\"TrUe\"",
        (8, 6),
    )
}

fn assert_site(
    element: &SourceElementPosition,
    attribute: &SourceAttributePosition,
    bytes: &[u8],
    spelling: &[u8],
    expected: (u32, u32),
) -> TestResult {
    match (element, attribute) {
        (SourceElementPosition::Text(element), SourceAttributePosition::Text(attribute)) => {
            assert!(
                element.start < attribute.start && attribute.end <= element.end,
                "the raw attribute belongs to its element"
            );
            assert_eq!(
                bytes.get(attribute.clone()),
                Some(spelling),
                "exact raw text bytes"
            );
        }
        (
            SourceElementPosition::CryXml(element),
            SourceAttributePosition::CryXml {
                node_index,
                attribute_index,
            },
        ) => {
            assert_eq!(
                (*node_index, *attribute_index),
                expected,
                "unfiltered table positions"
            );
            assert_eq!(
                element, node_index,
                "the attribute belongs to the actual element"
            );
        }
        _ => return Err("element and attribute must belong to the same byte domain".into()),
    }
    Ok(())
}

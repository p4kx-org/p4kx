// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_extraction::sound::radar_audio::{RadarAudioElement, RadarAudioField};
use p4kx_extraction::{Archive, SourceValueState};
use p4kx_testkit::{
    ArchiveBuilder, CryXmlByteOrder, CryXmlSignature, DataForgeBuilder, TestElement, TestEntry,
};
use serde_json::Value;

use super::{
    PATH, RadarAudioBindingStatus, RadarAudioConfiguration, RadarAudioDatum, RadarAudioNode,
};
use crate::catalog_queries::radar_audio::{RadarAudioSelection, read_radar_audio};
use crate::catalog_table::DatasetAvailability;
use crate::{CatalogError, CatalogOptions, build_catalog};

#[path = "radar_audio_catalog_witness.rs"]
mod witness;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const SYNTHETIC: &[u8] = br#"<RadarAudioData>
    <DefaultZoom min="0.25" max="NaN"/>
    <CategoryTypes>
        <CategoryType name="fixture" value="+00018446744073709551616"/>
        <CategoryType name="fixture" value="-9"/>
    </CategoryTypes>
    <SoundEvents>
        <SoundEvent name="no-value"/>
        <SoundEvent name="empty" value=""/>
        <SoundEvent name="authored" value="FixtureToken"/>
        <SoundEvent name="state-token" value="FixtureState"/>
        <SoundEvent name="no-control" value="FixtureNoControl"/>
        <Unknown><SoundEvent name="future" value="other"/></Unknown>
    </SoundEvents>
</RadarAudioData>"#;

#[test]
fn original_slot_states_and_exact_numeric_declarations_match_catalog() -> TestResult {
    let (_folder, archive) = fixture()?;
    let sdk = read_radar_audio(&archive, RadarAudioSelection::All, None)?;
    assert_eq!(
        sdk.entries.selected, 2,
        "only the physical configuration Entries are read"
    );
    let [text, binary] = sdk.rows.as_slice() else {
        return Err("both physical configurations in order".into());
    };
    check_text(text)?;
    check_binary(binary)?;
    let index = binary.identity().entry_index();
    let selected = read_radar_audio(&archive, RadarAudioSelection::EntryIndex(index), None)?;
    witness::check_payload_scope(&archive, index, &selected)?;
    check_catalog(&archive, &sdk.rows)
}

fn fixture() -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let binary = TestElement::new("RadarAudioData")
        .child(
            TestElement::new("SoundEvents").child(
                TestElement::new("SoundEvent")
                    .attribute("name", "duplicate")
                    .attribute("value", "first")
                    .attribute("value", "second"),
            ),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    ArchiveBuilder::new()
        .entry(dataforge())
        .entry(TestEntry::stored(PATH, SYNTHETIC))
        .entry(TestEntry::stored(PATH.to_ascii_lowercase(), binary).encrypted())
        .entry(TestEntry::stored(
            "Data/Libs/GameAudio/fixture-atl.xml",
            br#"<ATLConfig>
                <AudioTriggers>
                    <ATLTrigger atl_name="FixtureToken">
                        <WwiseEvent wwise_name="FixtureToken"/>
                    </ATLTrigger>
                </AudioTriggers>
                <AudioSwitches>
                    <ATLSwitch atl_name="FixtureSwitch">
                        <ATLSwitchState atl_name="FixtureState">
                            <WwiseSwitch wwise_name="FixtureSwitch">
                                <WwiseValue wwise_name="FixtureState"/>
                            </WwiseSwitch>
                        </ATLSwitchState>
                    </ATLSwitch>
                </AudioSwitches>
            </ATLConfig>"#,
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    Ok((folder, archive))
}

fn dataforge() -> TestEntry {
    TestEntry::stored("Data/Game2.dcb", DataForgeBuilder::new().version(8).build())
}

fn check_text(row: &RadarAudioConfiguration) -> TestResult {
    let zoom = row.elements().get(1).ok_or("zoom range")?;
    assert_eq!(
        zoom.fields()[0].value().value(),
        Some(&RadarAudioDatum::Number { value: 0.25 }),
    );
    assert_eq!(zoom.fields()[1].value().state(), SourceValueState::Invalid);
    let categories: Vec<_> = row
        .elements()
        .iter()
        .filter(|node| node.role().value() == Some(&Some(RadarAudioElement::CategoryType)))
        .collect();
    let [first, second] = categories.as_slice() else {
        return Err("repeated categories retain two physical owners".into());
    };
    for (category, decimal) in [(first, "+00018446744073709551616"), (second, "-9")] {
        let value = category.fields()[1].value();
        assert_eq!(
            value.value(),
            Some(&RadarAudioDatum::Integer {
                decimal: decimal.to_owned()
            }),
        );
        assert_eq!(value.origin().contains_encrypted_source(), Some(false));
    }
    assert_eq!(
        binding_states(row.elements()),
        [
            RadarAudioBindingStatus::MissingValue,
            RadarAudioBindingStatus::EmptyValue,
            RadarAudioBindingStatus::UnprovenSelector,
            RadarAudioBindingStatus::UnprovenSelector,
            RadarAudioBindingStatus::UnprovenSelector,
        ],
        "a matching trigger, a matching switch state and an absent control all lack a selector"
    );
    Ok(())
}

fn check_binary(row: &RadarAudioConfiguration) -> TestResult {
    assert_eq!(
        binding_states(row.elements()),
        [RadarAudioBindingStatus::InvalidValue]
    );
    let slot = row.elements().last().ok_or("duplicated physical slot")?;
    assert_eq!(slot.authored().declarations().len(), 3);
    assert_eq!(slot.fields()[1].field(), RadarAudioField::SoundEventValue);
    assert_eq!(
        slot.fields()[1]
            .value()
            .origin()
            .contains_encrypted_source(),
        Some(true)
    );
    Ok(())
}

fn check_catalog(archive: &Archive, sdk: &[RadarAudioConfiguration]) -> TestResult {
    let catalog = build_catalog(
        archive,
        CatalogOptions::new(UNIX_EPOCH).with_full_provenance(),
    )?;
    let json: Value = serde_json::from_slice(catalog.json())?;
    let rows = json
        .pointer("/configurations/radarAudio/rows")
        .and_then(Value::as_array)
        .ok_or("canonical radar audio rows")?;
    assert_eq!(rows.len(), sdk.len());
    for (actual, expected) in rows.iter().zip(sdk) {
        let nodes = actual["elements"].as_array().ok_or("elements")?;
        assert_eq!(nodes.len(), expected.elements().len());
        for (node, expected) in nodes.iter().zip(expected.elements()) {
            check_node(node, expected)?;
        }
    }
    Ok(())
}

fn check_node(actual: &Value, expected: &RadarAudioNode) -> TestResult {
    let expected = serde_json::to_value(expected)?;
    for pointer in [
        "/authored/parent/value",
        "/authored/name/value",
        "/role/value",
        "/bindingStatus/value",
        "/bindingStatus/containsEncryptedSource",
        "/bindingStatus/sourceSetComplete",
    ] {
        assert_eq!(actual.pointer(pointer), expected.pointer(pointer));
    }
    let cells = actual["fields"].as_array().ok_or("typed fields")?;
    let expected_cells = expected["fields"].as_array().ok_or("SDK typed fields")?;
    assert_eq!(cells.len(), expected_cells.len());
    for (cell, expected_cell) in cells.iter().zip(expected_cells) {
        assert_eq!(cell["field"], expected_cell["field"]);
        for part in ["state", "value", "containsEncryptedSource"] {
            assert_eq!(cell["value"][part], expected_cell["value"][part]);
        }
    }
    Ok(())
}

#[test]
fn malformed_radar_bodies_fail_required_projection() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(dataforge())
        .entry(TestEntry::stored(PATH, b"<RadarAudioData"))
        .entry(TestEntry::stored(PATH.to_ascii_lowercase(), b"CryXmlB\0"))
        .entry(TestEntry::stored(
            PATH.to_ascii_uppercase(),
            b"<Unrelated/>",
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let snapshot = read_radar_audio(&archive, RadarAudioSelection::All, None)?;
    assert!(matches!(
        snapshot.availability,
        DatasetAvailability::Unavailable
    ));
    assert_eq!(snapshot.entries.failed_projections, 3);
    assert!(matches!(
        build_catalog(
            &archive,
            CatalogOptions::new(UNIX_EPOCH).with_full_provenance()
        ),
        Err(CatalogError::IncompleteExport(_))
    ));
    Ok(())
}

fn binding_states(elements: &[RadarAudioNode]) -> Vec<RadarAudioBindingStatus> {
    elements
        .iter()
        .filter(|node| node.role().value() == Some(&Some(RadarAudioElement::SoundEvent)))
        .filter_map(RadarAudioNode::binding_status)
        .filter_map(|status| status.value().copied())
        .collect()
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::Value;

use super::{CLIP_SOURCE_TYPE, WeaponProceduralAnimation};
use crate::build::original_input::test_support::{self, TestResult};
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::weapon_procedural_definitions::{
    ActorProceduralRecoilConfig, RECOIL_CONFIG_SOURCE_TYPE, RECOIL_MODIFIERS_SOURCE_TYPE,
};
use crate::record_observation::RecordObservation;
use crate::template_field_capture::TemplateFieldCapture;

const CLIP: u128 = 0x1053_0020;
const MODIFIERS: u128 = 0x1053_0030;
const FILTER: &str = "ActorStateFilter";
const STANCE: &str = "ActorStateFilterByStanceState";
const CLIP_SETUP: &str = "WeaponProceduralClipsSetUp";
const RECOIL_SETUP: &str = "ActorProceduralRecoilSetup";

#[test]
fn weapon_procedural_rows_keep_order_and_physical_reference_targets() -> TestResult {
    test_support::with_input(&fixture(false), |forge, input| {
        let record = forge.record(0).ok_or("actual animation record")?;
        let tree = record.tree()?;
        let admission = RecordObservation::capture(input, &record)?;
        let scope = input.record_scope(&record, &tree)?;
        let originals = OriginalFlowFields::read(&scope, None)?;
        let capture = TemplateFieldCapture::new(&originals, input, &admission)?;
        let numbers = RefCell::new(Vec::new());
        let mut notes = Vec::new();
        let header = CanonicalRecordHeader::read(&tree, input.origins())?;
        let fields = Observed::configuration(&originals, input, &capture, &numbers);
        let animation = WeaponProceduralAnimation::read(header, fields, &mut notes)?;
        assert_eq!(
            animation.linked_definitions().collect::<Vec<_>>(),
            [2, 2],
            "physical target Occurrences preserve repeat and order"
        );
        let row = serde_json::to_value(animation)?;
        let setups = at(&row, "/weaponProceduralClipsSetUp/value")?;
        assert_eq!(
            at(setups, "/0/value/filter/filterByStanceState/value")?,
            "FixtureStand",
            "first inherited authored stance"
        );
        assert_eq!(
            at(setups, "/1/value/filter/filterByStanceState/value")?,
            "FixtureKneel",
            "second inherited authored stance"
        );
        assert_eq!(
            at(setups, "/0/value/weaponProceduralClips/value/1/state")?,
            "null",
            "explicit null position is kept"
        );
        let setup_field = originals
            .field(None, "weaponProceduralClipsSetUp")?
            .ok_or("original setup list")?
            .1;
        let first_setup = originals.first_struct(setup_field)?.ok_or("first setup")?;
        let first_stance = originals
            .field(Some(first_setup), "filterByStanceState")?
            .ok_or("original inherited stance field")?
            .1;
        let cell = at(setups, "/0/value/filter/filterByStanceState")?;
        let inventory = serde_json::to_value(scope.sources().freeze()?)?;
        let location = serde_json::to_value(first_stance.location(OriginalValuePart::Value))?;
        assert_original_payload(cell, &inventory, &location)?;
        assert!(
            notes.is_empty(),
            "well-formed setup fields produce no diagnostic"
        );
        Ok(())
    })
}

#[test]
fn weapon_procedural_recoil_keeps_the_modifier_and_unprojected_fields() -> TestResult {
    test_support::with_input(&fixture(false), |forge, input| {
        let record = forge.record(1).ok_or("actual recoil record")?;
        let tree = record.tree()?;
        let admission = RecordObservation::capture(input, &record)?;
        let scope = input.record_scope(&record, &tree)?;
        let originals = OriginalFlowFields::read(&scope, None)?;
        let capture = TemplateFieldCapture::new(&originals, input, &admission)?;
        let numbers = RefCell::new(Vec::new());
        let mut notes = Vec::new();
        let header = CanonicalRecordHeader::read(&tree, input.origins())?;
        let fields = Observed::configuration(&originals, input, &capture, &numbers);
        let recoil = ActorProceduralRecoilConfig::read(header, fields, &mut notes)?;
        assert_eq!(
            recoil.linked_definitions().collect::<Vec<_>>(),
            [3],
            "recoil modifier target is the actual physical Record"
        );
        let row = serde_json::to_value(recoil)?;
        let slot = at(&row, "/actorProceduralRecoilSetup/value/0/value")?;
        assert_eq!(
            at(slot, "/filter/filterName/value")?,
            "FixtureFilter",
            "the inherited filter name remains sourced"
        );
        assert_eq!(
            at(slot, "/additionalFields/roots/0/name/value")?,
            "fixtureExtra",
            "untyped authored setup members remain in the original arena"
        );
        assert!(
            notes.is_empty(),
            "well-formed setup fields produce no diagnostic"
        );
        Ok(())
    })
}

#[test]
fn weapon_procedural_array_rejection_keeps_its_original_field() -> TestResult {
    test_support::with_input(&fixture(true), |forge, input| {
        let record = forge.record(0).ok_or("actual animation record")?;
        let tree = record.tree()?;
        let admission = RecordObservation::capture(input, &record)?;
        let scope = input.record_scope(&record, &tree)?;
        let originals = OriginalFlowFields::read(&scope, None)?;
        let capture = TemplateFieldCapture::new(&originals, input, &admission)?;
        let numbers = RefCell::new(Vec::new());
        let mut notes = Vec::new();
        let header = CanonicalRecordHeader::read(&tree, input.origins())?;
        let fields = Observed::configuration(&originals, input, &capture, &numbers);
        let animation = WeaponProceduralAnimation::read(header, fields, &mut notes)?;
        assert!(
            animation.linked_definitions().next().is_none(),
            "no guessed clip link"
        );
        let row = serde_json::to_value(animation)?;
        assert_eq!(
            at(&row, "/weaponProceduralClipsSetUp/state")?,
            "unsupported",
            "the wrong nominal owned class does not become a typed setup"
        );
        assert_eq!(
            at(&row, "/additionalFields/roots/0/name/value")?,
            "weaponProceduralClipsSetUp",
            "the complete rejected original setup array remains"
        );
        assert_eq!(notes.len(), 1, "one rejected array declaration");
        Ok(())
    })
}

fn fixture(wrong_setup: bool) -> Vec<u8> {
    let animation_setup = if wrong_setup {
        RECOIL_SETUP
    } else {
        CLIP_SETUP
    };
    let clip_links = TestValue::Array(vec![
        TestValue::Reference(CLIP),
        TestValue::Null,
        TestValue::Reference(CLIP),
    ]);
    let first_setup = setup(CLIP_SETUP, "FixtureStand").value("weaponProceduralClips", clip_links);
    let second_setup = setup(CLIP_SETUP, "FixtureKneel")
        .value("weaponProceduralClips", TestValue::Array(Vec::new()));
    let recoil_setup = setup(RECOIL_SETUP, "FixtureStand")
        .value(
            "actorProceduralRecoilModifiers",
            TestValue::Reference(MODIFIERS),
        )
        .value("fixtureExtra", TestValue::Bool(true));
    let animation_slots = if wrong_setup {
        vec![TestValue::Struct(recoil_setup.clone())]
    } else {
        vec![
            TestValue::Struct(first_setup),
            TestValue::Struct(second_setup),
        ]
    };
    declarations(animation_setup)
        .record(
            record(
                0x1053_0001,
                super::ANIMATION_SOURCE_TYPE,
                "fixture/animation.xml",
            )
            .value(
                "weaponProceduralClipsSetUp",
                TestValue::Array(animation_slots),
            ),
        )
        .record(
            record(0x1053_0002, RECOIL_CONFIG_SOURCE_TYPE, "fixture/recoil.xml").value(
                "actorProceduralRecoilSetup",
                TestValue::Array(vec![TestValue::Struct(recoil_setup)]),
            ),
        )
        .record(record(CLIP, CLIP_SOURCE_TYPE, "fixture/clip.xml"))
        .record(record(
            MODIFIERS,
            RECOIL_MODIFIERS_SOURCE_TYPE,
            "fixture/modifiers.xml",
        ))
        .build()
}

fn declarations(animation_setup: &str) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .enumeration(STANCE, ["FixtureStand", "FixtureKneel"])
        .structure(
            TestStruct::new(FILTER)
                .field("filterName", TestType::String)
                .field("filterByStanceState", TestType::Enum(STANCE.to_owned())),
        )
        .structure(TestStruct::new(CLIP_SETUP).parent(FILTER).array(
            "weaponProceduralClips",
            TestType::ReferenceTo(CLIP_SOURCE_TYPE.to_owned()),
        ))
        .structure(
            TestStruct::new(RECOIL_SETUP)
                .parent(FILTER)
                .field(
                    "actorProceduralRecoilModifiers",
                    TestType::ReferenceTo(RECOIL_MODIFIERS_SOURCE_TYPE.to_owned()),
                )
                .field("fixtureExtra", TestType::Bool),
        )
        .structure(TestStruct::new(super::ANIMATION_SOURCE_TYPE).array(
            "weaponProceduralClipsSetUp",
            TestType::Class(animation_setup.to_owned()),
        ))
        .structure(TestStruct::new(RECOIL_CONFIG_SOURCE_TYPE).array(
            "actorProceduralRecoilSetup",
            TestType::Class(RECOIL_SETUP.to_owned()),
        ))
        .structure(TestStruct::new(CLIP_SOURCE_TYPE))
        .structure(TestStruct::new(RECOIL_MODIFIERS_SOURCE_TYPE))
}

fn setup(source: &str, stance: &str) -> TestInstance {
    TestInstance::new(source)
        .value("filterName", TestValue::text("FixtureFilter"))
        .value("filterByStanceState", TestValue::text(stance))
}

fn record(guid: u128, source_type: &str, path: &str) -> TestRecord {
    TestRecord::new(guid, source_type, format!("{source_type}.Fixture"), path)
}

fn assert_original_payload(cell: &Value, inventory: &Value, location: &Value) -> TestResult {
    let refs = at(cell, "/sourceUsesRefs")?
        .as_array()
        .ok_or("stance origin refs")?;
    let uses = at(inventory, "/uses")?
        .as_array()
        .ok_or("actual original uses")?;
    assert!(
        uses.iter().any(|used| {
            used.get("id").is_some_and(|id| refs.contains(id))
                && used.get("role").and_then(Value::as_str) == Some("payload")
                && used.get("location") == Some(location)
        }),
        "the stance points to its own original payload, not the owning setup"
    );
    Ok(())
}

fn at<'value>(value: &'value Value, path: &str) -> TestResult<&'value Value> {
    value
        .pointer(path)
        .ok_or_else(|| format!("missing procedural source field {path}").into())
}

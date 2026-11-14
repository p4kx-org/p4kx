// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{DataType, SourceLocation, Value as Original};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use super::{AuthoredProjectile, Observed, at};
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::record_observation::RecordObservation;
use crate::template_field_capture::TemplateFieldCapture;

const BEAM: &str = "SBeamEffectParams";
const AUDIO: &str = "SBeamParticleAudioParams";
const UNRELATED: &str = "FixtureUnrelatedClass";

#[derive(Clone, Copy)]
enum ArrayField {
    Beam,
    ParticleAudio,
}

#[test]
fn ammo_empty_beam_effects_bind_the_declared_class_target() -> TestResult {
    check(ArrayField::Beam, BEAM)
}

#[test]
fn ammo_empty_beam_effects_reject_an_unrelated_class_target() -> TestResult {
    check(ArrayField::Beam, UNRELATED)
}

#[test]
fn ammo_empty_particle_audio_binds_the_declared_class_target() -> TestResult {
    check(ArrayField::ParticleAudio, AUDIO)
}

#[test]
fn ammo_empty_particle_audio_rejects_an_unrelated_class_target() -> TestResult {
    check(ArrayField::ParticleAudio, UNRELATED)
}

fn check(field: ArrayField, target: &str) -> TestResult {
    test_support::with_input(&bytes(field, target), |forge, input| {
        let record = forge.record(0).ok_or("original Ammo Record")?;
        let tree = record.tree()?;
        let admission = RecordObservation::capture(input, &record)?;
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let (location, target_index) = original_target(&fields, field)?;
        let declaration = forge
            .structs()
            .nth(usize::from(target_index))
            .ok_or("original declared Class target")?;
        assert_eq!(
            declaration.name()?,
            target,
            "the physical DataForge Class declaration points to the fixture target"
        );

        let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
        let numbers = RefCell::new(Vec::new());
        let mut notes = Vec::new();
        let output = AuthoredProjectile::read_ammo(
            Observed::definition(&fields, input, &capture, &numbers),
            &mut notes,
        )?;
        let json = serde_json::to_value(output)?;
        let cell = at(&json, field.wire_path())?;
        check_cell(cell, target, notes.len())?;
        let inventory = serde_json::to_value(scope.sources().freeze()?)?;
        check_selector(cell, &inventory, &location)?;
        check_selector(
            cell,
            &inventory,
            &SourceLocation::Subresource {
                namespace: "dataforgeStruct".to_owned(),
                indices: vec![u64::from(target_index)],
                field: "name".to_owned(),
            },
        )
    })
}

fn original_target(
    fields: &OriginalFlowFields<'_, '_, '_>,
    field: ArrayField,
) -> TestResult<(SourceLocation, u16)> {
    let (_, projectile) = fields
        .field(None, "projectileParams")?
        .ok_or("original projectile selector")?;
    let (_, beams) = fields
        .field(Some(projectile), "beamEffects")?
        .ok_or("original beam array")?;
    let array = match field {
        ArrayField::Beam => beams,
        ArrayField::ParticleAudio => {
            let Original::Array(values) = beams.value() else {
                return Err("original beam array values".into());
            };
            let [value] = values.as_ref() else {
                return Err("the original first beam".into());
            };
            let (_, audio) = fields
                .field(Some(fields.fact(value)?), "particleAudioParams")?
                .ok_or("original particle audio array")?;
            audio
        }
    };
    let storage = array.field().storage();
    assert_eq!(storage.data_type(), DataType::Class, "actual Class storage");
    assert!(storage.is_array(), "actual array storage");
    assert!(
        matches!(array.value(), Original::Array(values) if values.is_empty()),
        "both declarations decode to a genuinely empty array"
    );
    Ok((array.location(OriginalValuePart::Target), storage.target()))
}

fn check_cell(cell: &Value, target: &str, note_count: usize) -> TestResult {
    let (state, value, count) = if target == UNRELATED {
        ("unsupported", Value::Null, 1)
    } else {
        ("present", json!([]), 0)
    };
    assert_eq!(
        at(cell, "/state")?,
        state,
        "state of the declared Class target"
    );
    assert_eq!(
        at(cell, "/value")?,
        &value,
        "empty array versus rejected value"
    );
    assert_eq!(
        at(cell, "/sourceSetComplete")?,
        true,
        "the original declaration is readable in either case"
    );
    assert_eq!(
        note_count, count,
        "one bounded diagnostic only for the unrelated declared Class"
    );
    Ok(())
}

fn check_selector(cell: &Value, inventory: &Value, location: &SourceLocation) -> TestResult {
    let expected = serde_json::to_value(location)?;
    let refs = at(cell, "/sourceUsesRefs")?
        .as_array()
        .ok_or("cell source uses")?;
    let uses = at(inventory, "/uses")?
        .as_array()
        .ok_or("original source inventory")?;
    assert!(
        uses.iter().any(|used| {
            used.get("id").is_some_and(|id| refs.contains(id))
                && used.get("role").and_then(Value::as_str) == Some("selector")
                && used.get("location") == Some(&expected)
        }),
        "the cell must cite the original declaration selector {expected}"
    );
    Ok(())
}

impl ArrayField {
    const fn wire_path(self) -> &'static str {
        match self {
            Self::Beam => "/value/kind/value/beamEffects",
            Self::ParticleAudio => {
                "/value/kind/value/beamEffects/value/0/value/particleAudioParams"
            }
        }
    }
}

fn bytes(field: ArrayField, target: &str) -> Vec<u8> {
    let (beam_target, audio_target, beams) = match field {
        ArrayField::Beam => (target, AUDIO, Vec::new()),
        ArrayField::ParticleAudio => {
            let beam =
                TestInstance::new(BEAM).value("particleAudioParams", TestValue::Array(Vec::new()));
            (BEAM, target, vec![TestValue::Struct(beam)])
        }
    };
    let projectile =
        TestInstance::new("TachyonProjectileParams").value("beamEffects", TestValue::Array(beams));
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("AmmoParams").field(
            "projectileParams",
            TestType::StrongPointer("ProjectileParams".to_owned()),
        ))
        .structure(TestStruct::new("ProjectileParams"))
        .structure(
            TestStruct::new("TachyonProjectileParams")
                .parent("ProjectileParams")
                .array("beamEffects", TestType::Class(beam_target.to_owned())),
        )
        .structure(TestStruct::new(BEAM).array(
            "particleAudioParams",
            TestType::Class(audio_target.to_owned()),
        ))
        .structure(TestStruct::new(AUDIO).field("subParticlePath", TestType::String))
        .structure(TestStruct::new(UNRELATED).field("fixtureWord", TestType::UInt8))
        .record(
            TestRecord::new(
                0x8120,
                "AmmoParams",
                "AmmoParams.ClassArrays",
                "fixture/class_arrays.xml",
            )
            .value("projectileParams", TestValue::Struct(projectile)),
        )
        .build()
}

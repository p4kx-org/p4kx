// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Read as _;

use p4kx_extraction::{
    Archive, DataForgeEntry, EntityMaterialEffects, EntityMaterialEffectsComponent,
    EntityMaterialEffectsError, FieldOrigin, FieldPath, SourceInventory, SourceLocation,
    SourceUseRole,
};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue, test_key,
};
use serde_json::{Value, json};

const ENTITY: &str = "EntityClassDefinition";
const COMPONENT: &str = "EntityComponentParams";
const EFFECTS: &str = "SEntityComponentEffects";
const MULTIPLIER: &str = "SEntityEffectSystem_MultiplierModifier";
const MIN_MAX: &str = "SEntityEffectSystem_MinMaxModifier";
const UNKNOWN: &str = "SyntheticFutureModifier";
const MODIFIERS_PATH: &str = "/tagEffects/0/propertyLinks/0/modifiers";

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn reads_effects_in_original_components_and_attributes_disabled_and_unknown_values() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(r"Data\Game2.dcb", fixture()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let record = forge.record(0).ok_or("synthetic entity")?;
    let result = EntityMaterialEffects::read(&entry, &forge, record.index(), record.guid())?;
    assert_eq!(result.guid, record.guid(), "same physical source record");
    let [component] = result.components.as_slice() else {
        return Err("one effect component required".into());
    };
    assert_eq!(
        component.component, 2,
        "prior null and unrelated slots stay in the source ordinal"
    );
    assert_eq!(component.source_type, EFFECTS, "exact component type");
    assert_eq!(
        component.declaration.get("enabled"),
        Some(&Value::Bool(false)),
        "disabled source group is preserved"
    );
    let original = DataForgeEntry::locate(&archive).ok_or("original DataForge Entry")?;
    let mut decoded = Vec::new();
    archive.open_entry(original)?.read_to_end(&mut decoded)?;
    verify_modifiers(component, &result.provenance, &decoded)?;
    let wrong = forge.record(1).ok_or("synthetic second record")?;
    assert!(
        matches!(
            EntityMaterialEffects::read(&entry, &forge, record.index(), wrong.guid()),
            Err(EntityMaterialEffectsError::Identity)
        ),
        "record index and original guid must agree"
    );
    Ok(())
}

fn verify_modifiers(
    component: &EntityMaterialEffectsComponent,
    inventory: &SourceInventory,
    input: &[u8],
) -> TestResult {
    assert_eq!(
        component.declaration.pointer(MODIFIERS_PATH),
        Some(&json!([
            {"$type": MULTIPLIER, "multiplier": 0.5},
            {"$type": MIN_MAX, "minimum": -0.25, "maximum": 0.75},
            {"$type": MULTIPLIER, "multiplier": 2.25},
            {"$type": UNKNOWN, "factor": "NaN"},
        ])),
        "authored types, fields and order survive including the unknown modifier"
    );
    for (field, expected) in [
        ("0/multiplier", 0.5),
        ("1/minimum", -0.25),
        ("1/maximum", 0.75),
        ("2/multiplier", 2.25),
        ("3/factor", f32::NAN),
    ] {
        verify_modifier_origin(component, inventory, input, field, expected)?;
    }
    Ok(())
}

fn verify_modifier_origin(
    component: &EntityMaterialEffectsComponent,
    inventory: &SourceInventory,
    input: &[u8],
    field: &str,
    expected: f32,
) -> TestResult {
    let path = FieldPath::parse(&format!("{MODIFIERS_PATH}/{field}"))?;
    let origin = component
        .field_origins
        .get(&path)
        .ok_or("modifier origin")?;
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "{field}: original encrypted DataForge input"
    );
    assert!(
        origin.source_set_complete(),
        "{field}: exact route and value evidence"
    );
    verify_modifier_ranges(inventory, input, origin, field, expected)
}

fn verify_modifier_ranges(
    inventory: &SourceInventory,
    input: &[u8],
    origin: &FieldOrigin,
    field: &str,
    expected: f32,
) -> TestResult {
    let mut header_count = 0;
    let mut pointers = 0;
    let mut slots = Vec::new();
    for site in inventory.uses() {
        if !origin.source_uses_refs().contains(site.id()) {
            continue;
        }
        if let SourceLocation::Subresource {
            namespace,
            field,
            indices,
        } = site.location()
            && namespace == "dataforgeDecodedBytes"
        {
            match (field.as_str(), site.role()) {
                ("arrayHeader", SourceUseRole::Selector) => header_count += 1,
                ("pointerSelector", SourceUseRole::Selector) => {
                    let [start, end] = indices.as_slice() else {
                        return Err("original pointer selector span".into());
                    };
                    assert_eq!(
                        end.checked_sub(*start),
                        Some(size_of::<[u32; 2]>() as u64),
                        "the route uses only the original struct and instance words",
                    );
                    pointers += 1;
                }
                ("slot", SourceUseRole::Payload) => slots.push(indices.as_slice()),
                (_, SourceUseRole::Payload) => return Err("unexpected decoded payload span".into()),
                _ => {}
            }
        }
    }
    assert_eq!(
        header_count, 4,
        "{field}: the original component, effect, link and modifier array headers"
    );
    assert_eq!(
        pointers, 2,
        "{field}: component and modifier pointer selectors"
    );
    let [[start, end]] = slots.as_slice() else {
        return Err(format!("{field}: expected one original Float32 slot").into());
    };
    assert_eq!(
        input.get(usize::try_from(*start)?..usize::try_from(*end)?),
        Some(expected.to_le_bytes().as_slice()),
        "{field}: exact original Float32 bytes at the reported position"
    );
    Ok(())
}

fn fixture() -> Vec<u8> {
    let builder = DataForgeBuilder::new()
        .structure(
            TestStruct::new(ENTITY).array("Components", TestType::StrongPointer(COMPONENT.into())),
        )
        .structure(TestStruct::new(COMPONENT))
        .structure(
            TestStruct::new(EFFECTS)
                .parent(COMPONENT)
                .field("entityEffects", TestType::Class("Effects".into())),
        )
        .structure(
            TestStruct::new("Effects")
                .field("materialEffects", TestType::Class("MaterialEffects".into())),
        )
        .structure(
            TestStruct::new("MaterialEffects")
                .field("enabled", TestType::Bool)
                .array("tagEffects", TestType::Class("TagEffect".into())),
        )
        .structure(
            TestStruct::new("TagEffect")
                .array("propertyLinks", TestType::Class("PropertyLink".into())),
        )
        .structure(
            TestStruct::new("PropertyLink")
                .field("materialProperty", TestType::String)
                .array("modifiers", TestType::StrongPointer("Modifier".into())),
        );
    modifier_shapes(builder)
        .record(
            TestRecord::new(1, ENTITY, "EntityClassDefinition.Source", "source.xml").value(
                "Components",
                TestValue::Array(vec![
                    TestValue::Null,
                    TestValue::Struct(TestInstance::new(COMPONENT)),
                    effect(),
                ]),
            ),
        )
        .record(TestRecord::new(
            2,
            ENTITY,
            "EntityClassDefinition.Other",
            "other.xml",
        ))
        .build()
}

fn modifier_shapes(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(TestStruct::new("Modifier"))
        .structure(
            TestStruct::new(MULTIPLIER)
                .parent("Modifier")
                .field("multiplier", TestType::Single),
        )
        .structure(
            TestStruct::new(MIN_MAX)
                .parent("Modifier")
                .field("minimum", TestType::Single)
                .field("maximum", TestType::Single),
        )
        .structure(
            TestStruct::new(UNKNOWN)
                .parent("Modifier")
                .field("factor", TestType::Single),
        )
}

fn effect() -> TestValue {
    let scale_down = TestInstance::new(MULTIPLIER).value("multiplier", TestValue::Single(0.5));
    let clamp = TestInstance::new(MIN_MAX)
        .value("minimum", TestValue::Single(-0.25))
        .value("maximum", TestValue::Single(0.75));
    let scale_up = TestInstance::new(MULTIPLIER).value("multiplier", TestValue::Single(2.25));
    let unknown = TestInstance::new(UNKNOWN).value("factor", TestValue::Single(f32::NAN));
    let modifiers = [scale_down, clamp, scale_up, unknown]
        .into_iter()
        .map(TestValue::Struct)
        .collect();
    let link = TestInstance::new("PropertyLink")
        .value("materialProperty", TestValue::text("Glow"))
        .value("modifiers", TestValue::Array(modifiers));
    let tag = TestInstance::new("TagEffect").value(
        "propertyLinks",
        TestValue::Array(vec![TestValue::Struct(link)]),
    );
    let groups = TestInstance::new("MaterialEffects")
        .value("enabled", TestValue::Bool(false))
        .value("tagEffects", TestValue::Array(vec![TestValue::Struct(tag)]));
    let entity = TestInstance::new("Effects").value("materialEffects", TestValue::Struct(groups));
    TestValue::Struct(TestInstance::new(EFFECTS).value("entityEffects", TestValue::Struct(entity)))
}

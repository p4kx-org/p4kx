// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{
    Archive, DataForgeEntry, EntityMaterialEffects, EntityMaterialEffectsError, FieldPath,
    SourceLocation,
};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue, test_key,
};
use serde_json::Value;

const ENTITY: &str = "EntityClassDefinition";
const COMPONENT: &str = "EntityComponentParams";
const EFFECTS: &str = "SEntityComponentEffects";

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
    assert_eq!(
        component
            .declaration
            .pointer("/tagEffects/0/propertyLinks/0/modifiers/0/factor"),
        Some(&Value::from("NaN")),
        "unknown modifier and its nonfinite authored value stay visible",
    );
    let leaf = FieldPath::parse("/tagEffects/0/propertyLinks/0/modifiers/0/factor")?;
    let origin = component.field_origins.get(&leaf).ok_or("leaf evidence")?;
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(true),
        "encrypted original input"
    );
    assert!(
        origin.source_set_complete(),
        "exact route and value evidence"
    );
    assert!(
        result.provenance.uses().iter().filter(|site| {
            origin.source_uses_refs().contains(site.id())
                && matches!(site.location(), SourceLocation::Subresource { namespace, field, .. }
                if namespace == "dataforgeDecodedBytes" && field == "arrayHeader")
        }).count() == 4,
        "each original component, effect, link and modifier array header contributes"
    );
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

fn fixture() -> Vec<u8> {
    DataForgeBuilder::new()
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
        )
        .structure(TestStruct::new("Modifier"))
        .structure(
            TestStruct::new("SyntheticFutureModifier")
                .parent("Modifier")
                .field("factor", TestType::Single),
        )
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

fn effect() -> TestValue {
    let modifier =
        TestInstance::new("SyntheticFutureModifier").value("factor", TestValue::Single(f32::NAN));
    let link = TestInstance::new("PropertyLink")
        .value("materialProperty", TestValue::text("Glow"))
        .value(
            "modifiers",
            TestValue::Array(vec![TestValue::Struct(modifier)]),
        );
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

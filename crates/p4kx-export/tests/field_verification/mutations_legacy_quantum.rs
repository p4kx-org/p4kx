// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, build_catalog_from};
use p4kx_extraction::{Archive, DataForge, DataForgeEntry, Guid};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestInstance, TestRecord,
    TestStruct, TestType, TestValue,
};
use serde_json::{Value, json};

use super::accounting::{DeclarationState, Population, Target};
use super::baseline::{self, LEGACY_EFFECT_FIELDS, LEGACY_EFFECT_OWNER};
use super::coverage::{self, QUANTUM_PROFILE_TYPE, Scope};
use super::inventory::{Disposition, FieldKey, Inventory};

const DRIVE: &str = "SCItemQuantumDriveParams";
const BASE: &str = "SQuantumDriveEffectBaseParams";
const SIBLING: &str = "FixtureSiblingEffect";
const LEGACY_TAG: u128 = 0x1050;
const SIBLING_TAG: u128 = 0x1051;

#[test]
fn legacy_quantum_profile_inventory_reconciles_original_reference_slots() -> super::Result<()> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database()))
        .entry(TestEntry::stored(
            "Data/Localization/english/global.ini",
            GlobalIniBuilder::new().build(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let built = build_catalog_from(
        &archive,
        &entry,
        CatalogOptions::new(UNIX_EPOCH).with_full_provenance(),
    )?;
    let mut catalog: Value = serde_json::from_slice(built.json())?;
    let mut inventory = baseline::inventory();
    inventory
        .fields
        .retain(|mapping| mapping.source.owner == LEGACY_EFFECT_OWNER);
    assert_eq!(inventory.fields.len(), 12, "the verified legacy fields");
    compare_slots(&forge, &inventory, &catalog, 2, 0)?;
    verify_remaining_reference(&forge, &inventory, &catalog)?;

    assert_eq!(
        replace_legacy_owners(&mut catalog)?,
        2,
        "each authored legacy mode has a concrete owner",
    );
    compare_slots(&forge, &inventory, &catalog, 0, 2)
}

fn compare_slots(
    forge: &DataForge<'_>,
    inventory: &Inventory,
    catalog: &Value,
    exported: u64,
    unhandled: u64,
) -> super::Result<()> {
    let counts = coverage::run(
        forge,
        inventory,
        catalog,
        BTreeSet::new(),
        BTreeMap::new(),
        Scope::default(),
        &|| false,
    )?;
    for name in LEGACY_EFFECT_FIELDS {
        let count = counts
            .get(&FieldKey::new(LEGACY_EFFECT_OWNER, name))
            .ok_or("reviewed legacy reference count")?;
        assert_eq!(
            (count.declaration, count.target, count.population),
            (
                DeclarationState::Declared,
                Target::Stable,
                Population::NewlyPopulated
            ),
            "new population requires review despite the verified Tag declaration: {name}",
        );
        assert_eq!(
            (
                count.present,
                count.populated,
                count.exported,
                count.unhandled
            ),
            (2, 1, exported, unhandled),
            "a distinct null slot and original Tag need the exact concrete owner: {name}",
        );
    }
    Ok(())
}

fn verify_remaining_reference(
    forge: &DataForge<'_>,
    inventory: &Inventory,
    catalog: &Value,
) -> super::Result<()> {
    let owner = forge.record(0).ok_or("synthetic quantum owner")?.guid();
    let expected = forge.record(1).ok_or("synthetic legacy Tag")?.guid();
    let projections = coverage::projections(catalog, inventory, &|| false)?;
    for mapping in &inventory.fields {
        let Disposition::Exported { group, path } = &mapping.disposition else {
            return Err("the reviewed legacy Reference has no Catalog path".into());
        };
        let rows = projections
            .get(group)
            .and_then(|rows| rows.get(&owner))
            .ok_or("synthetic quantum profiles")?;
        let mut observed = Vec::new();
        for row in rows {
            for slot in path.select(row, &|| false)? {
                if let Some(guid) = slot
                    .pointer("/value/value/guid/value")
                    .and_then(Value::as_str)
                {
                    observed.push(guid.parse::<Guid>()?);
                }
            }
        }
        assert_eq!(observed, [expected], "only the original Tag GUID");
    }
    Ok(())
}

fn replace_legacy_owners(catalog: &mut Value) -> super::Result<usize> {
    let rows = catalog
        .pointer_mut("/configurations/quantumDrives/rows")
        .and_then(Value::as_array_mut)
        .ok_or("canonical quantum drives")?;
    let mut changed = 0;
    for row in rows {
        for profile in ["/standard", "/spline"] {
            let Some(values) = row
                .pointer_mut(&format!("{profile}/value/effectTags/value/values"))
                .and_then(Value::as_array_mut)
            else {
                continue;
            };
            for node in values {
                if let Some(kind) = node.pointer_mut("/value/value/sourceType/value")
                    && kind.as_str() == Some(LEGACY_EFFECT_OWNER)
                {
                    *kind = json!(SIBLING);
                    changed += 1;
                }
            }
        }
    }
    Ok(changed)
}

fn database() -> Vec<u8> {
    let class = |name: &str| TestType::Class(name.to_owned());
    let pointer = |name: &str| TestType::StrongPointer(name.to_owned());
    let initial = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("EntityClassDefinition")
                .array("Components", pointer("DataForgeComponentParams")),
        )
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(
            TestStruct::new("SAttachableComponentParams")
                .parent("DataForgeComponentParams")
                .field("AttachDef", class("FixtureAttach")),
        )
        .structure(TestStruct::new("FixtureAttach").field("Type", TestType::String))
        .structure(
            TestStruct::new(DRIVE)
                .parent("DataForgeComponentParams")
                .field("params", class(QUANTUM_PROFILE_TYPE))
                .field("splineJumpParams", class(QUANTUM_PROFILE_TYPE)),
        )
        .structure(TestStruct::new(QUANTUM_PROFILE_TYPE).field("effectTags", pointer(BASE)))
        .structure(TestStruct::new(BASE))
        .structure(TestStruct::new("Tag").field("tagName", TestType::String))
        .record(owner());
    let database = [LEGACY_EFFECT_OWNER, SIBLING]
        .into_iter()
        .fold(initial, |builder, name| {
            let effect = LEGACY_EFFECT_FIELDS
                .into_iter()
                .fold(TestStruct::new(name).parent(BASE), |effect, field| {
                    effect.field(field, TestType::ReferenceTo("Tag".to_owned()))
                });
            builder.structure(effect)
        });
    [(LEGACY_TAG, "Legacy"), (SIBLING_TAG, "Sibling")]
        .into_iter()
        .fold(database, |builder, (guid, name)| {
            builder.record(
                TestRecord::new(
                    guid,
                    "Tag",
                    format!("Tag.{name}"),
                    "fixture/quantum/tags.xml",
                )
                .value("tagName", TestValue::text(name)),
            )
        })
        .build()
}

fn owner() -> TestRecord {
    let attach = TestInstance::new("SAttachableComponentParams").value(
        "AttachDef",
        TestValue::Struct(
            TestInstance::new("FixtureAttach").value("Type", TestValue::text("QuantumDrive")),
        ),
    );
    let drive = |kind, target| {
        TestInstance::new(DRIVE)
            .value("params", profile(kind, target))
            .value("splineJumpParams", profile(kind, None))
    };
    TestRecord::new(
        0x1052,
        "EntityClassDefinition",
        "EntityClassDefinition.fixture_legacy_quantum",
        "libs/foundry/records/entities/scitem/ships/fixture_legacy_quantum.xml",
    )
    .value(
        "Components",
        TestValue::Array(vec![
            TestValue::Struct(attach),
            TestValue::Struct(drive(LEGACY_EFFECT_OWNER, Some(LEGACY_TAG))),
            TestValue::Struct(drive(SIBLING, Some(SIBLING_TAG))),
        ]),
    )
}

fn profile(owner: &str, tag: Option<u128>) -> TestValue {
    let effect = LEGACY_EFFECT_FIELDS
        .into_iter()
        .fold(TestInstance::new(owner), |effect, field| {
            effect.value(field, tag.map_or(TestValue::Null, TestValue::Reference))
        });
    TestValue::Struct(
        TestInstance::new(QUANTUM_PROFILE_TYPE).value("effectTags", TestValue::Struct(effect)),
    )
}

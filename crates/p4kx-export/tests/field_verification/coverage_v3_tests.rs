// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::CatalogOptions;
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::{Value, json};

use super::inventory::{Disposition, FieldKey, Inventory, Mapping};
use super::path::Path;

fn catalog() -> super::Result<Value> {
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("ResourceType").field("displayName", TestType::Locale))
        .structure(TestStruct::new("Tag").field("tagName", TestType::String))
        .structure(TestStruct::new("FixtureComponent"))
        .structure(TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("FixtureComponent".to_owned()),
        ))
        .record(
            TestRecord::new(
                1,
                "ResourceType",
                "ResourceType.Audit",
                "libs/foundry/records/resourcetypedatabase/resource/audit.xml",
            )
            .value("displayName", TestValue::text("@audit")),
        )
        .record(TestRecord::new(
            2,
            "EntityClassDefinition",
            "EntityClassDefinition.AuditCommodity",
            "libs/foundry/records/entities/commodities/audit.xml",
        ));
    let database = database.record(
        TestRecord::new(3, "Tag", "Tag.Audit", "synthetic/dev/tags.xml")
            .value("tagName", TestValue::text("Audit")),
    );
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let built = p4kx_export::build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH))?;
    assert!(
        built.summary().export_health().is_complete(),
        "the source fixture is a genuinely healthy actual producer"
    );
    Ok(serde_json::from_slice(built.json())?)
}

fn inventory() -> Inventory {
    let fields = [
        ("resourceTypes", "ResourceType", "displayName"),
        ("commodities", "EntityClassDefinition", "Components"),
        ("tags", "Tag", "tagName"),
    ]
    .into_iter()
    .map(|(group, owner, field)| Mapping {
        source: FieldKey::new(owner, field),
        declared_target: None,
        disposition: Disposition::Exported {
            group: group.to_owned(),
            path: Path::fields(&["guid", "value"]),
        },
        previously_empty: false,
    })
    .collect();
    Inventory {
        schema_version: 1,
        roots: vec![
            "ResourceType".to_owned(),
            "EntityClassDefinition".to_owned(),
            "Tag".to_owned(),
        ],
        fields,
    }
}

#[test]
fn actual_dynamic_audit_joins_the_original_canonical_definition_ids() -> super::Result<()> {
    let mut catalog = catalog()?;
    catalog
        .as_object_mut()
        .ok_or("catalog object")?
        .insert("commodities".to_owned(), json!([]));
    catalog
        .as_object_mut()
        .ok_or("catalog object")?
        .insert("resourceTypes".to_owned(), json!([]));
    let inventory = inventory();
    inventory.reviewed()?;
    let projections = super::coverage::projections(&catalog, &inventory, &|| false)?;
    for group in ["resourceTypes", "commodities", "tags"] {
        assert_eq!(
            projections.get(group).ok_or("actual mapped group")?.len(),
            1,
            "{group} cannot silently use an empty legacy root"
        );
    }
    Ok(())
}

#[test]
fn missing_canonical_groups_and_nonpresent_identity_states_fail_the_actual_audit_router()
-> super::Result<()> {
    let catalog = catalog()?;
    for group in ["resourceTypes", "commodities", "tags"] {
        let mut missing = catalog.clone();
        missing
            .get_mut("definitions")
            .and_then(Value::as_object_mut)
            .ok_or("canonical definitions")?
            .remove(group)
            .ok_or("canonical group")?;
        assert!(
            super::coverage::projections(&missing, &inventory(), &|| false).is_err(),
            "missing {group} cannot be a zero-row success"
        );
        let mut absent = catalog.clone();
        let identity = absent
            .pointer_mut(&format!("/definitions/{group}/rows/0/guid"))
            .and_then(Value::as_object_mut)
            .ok_or("original sourced identity")?;
        identity.insert("state".to_owned(), json!("absent"));
        identity.insert("value".to_owned(), Value::Null);
        assert!(
            super::coverage::projections(&absent, &inventory(), &|| false).is_err(),
            "nonpresent {group} GUID is not a join identity"
        );
    }
    Ok(())
}

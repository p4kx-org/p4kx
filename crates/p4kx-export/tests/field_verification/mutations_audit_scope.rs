// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogError, CatalogOptions, CatalogPublicationMode, ExportHealth, Kind};
use p4kx_extraction::{Archive, DataForge, Filter};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};

use super::{audit_scope, source_scope};

fn database(global_failure: bool) -> Vec<u8> {
    let mut builder = DataForgeBuilder::new()
        .structure(TestStruct::new("SCItemManufacturer").field("Code", TestType::String))
        .structure(TestStruct::new("EntityClassDefinition").field("bad", TestType::Bool))
        .structure(
            TestStruct::new("CraftingGlobalParams")
                .field("value", TestType::Int32)
                .declared_size(6),
        )
        .record(
            TestRecord::new(
                1,
                "SCItemManufacturer",
                "SCItemManufacturer.SyntheticAudit",
                "libs/foundry/records/scitemmanufacturer/synthetic_audit.xml",
            )
            .value("Code", TestValue::text("AUDIT")),
        )
        .record(
            TestRecord::new(
                2,
                "EntityClassDefinition",
                "EntityClassDefinition.SyntheticAudit",
                "libs/foundry/records/entities/spaceships/synthetic_audit.xml",
            )
            .value("bad", TestValue::Raw(vec![2])),
        );
    if global_failure {
        builder = builder.record(TestRecord::new(
            3,
            "CraftingGlobalParams",
            "CraftingGlobalParams.SyntheticAudit",
            "libs/foundry/records/crafting/globalparams/synthetic_audit.xml",
        ));
    }
    builder.build()
}

fn health(global_failure: bool) -> super::Result<ExportHealth> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            database(global_failure),
        ))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    assert!(
        matches!(
            p4kx_export::build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH)),
            Err(CatalogError::IncompleteExport(_))
        ),
        "the real production Normal gate refuses the actual source defect"
    );
    let built = p4kx_export::build_catalog(&archive, &audit_scope::options(UNIX_EPOCH))?;
    assert_eq!(
        built.summary().export_health().publication_mode,
        CatalogPublicationMode::DiagnosticPartial,
        "only the independent audit explicitly requests partial diagnostic data"
    );
    Ok(built.summary().export_health().clone())
}

#[test]
fn an_unrelated_kind_failure_does_not_prevent_an_independent_mapped_audit() -> super::Result<()> {
    let health = health(false)?;
    assert!(
        !health.is_complete(),
        "the source defect remains visible in the full artifact health"
    );
    audit_scope::verify(&health, &[Kind::Manufacturers])?;
    assert!(
        audit_scope::verify(&health, &[Kind::Ships]).is_err(),
        "actual mapped-kind failure is never converted to an audit skip"
    );
    assert!(
        health
            .failed_records()
            .iter()
            .any(|source| source.kind == Some(Kind::Ships)
                && source.record_index().value() == Some(&1)),
        "the exact failed physical source remains available"
    );
    Ok(())
}

#[test]
fn an_unscoped_failure_cannot_be_assumed_unrelated_to_the_mapping() -> super::Result<()> {
    let health = health(true)?;
    assert!(
        health
            .failed_records()
            .iter()
            .any(|source| source.kind.is_none() && source.record_index().value() == Some(&2)),
        "the actual unreadable global source remains an exact physical identity"
    );
    assert!(
        audit_scope::verify(&health, &[Kind::Manufacturers]).is_err(),
        "an unscoped blocking source never becomes a claimed independent success"
    );
    Ok(())
}

fn aliases(outside: bool, same_guid: bool) -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(TestStruct::new("EntityClassDefinition"))
        .record(TestRecord::new(
            1,
            "EntityClassDefinition",
            "EntityClassDefinition.Selected",
            "libs/foundry/records/entities/scitem/selected.xml",
        ))
        .record(TestRecord::new(
            if same_guid { 1 } else { 2 },
            "EntityClassDefinition",
            "EntityClassDefinition.Other",
            if outside {
                "synthetic/outside.xml"
            } else {
                "libs/foundry/records/entities/scitem/other.xml"
            },
        ))
        .build()
}

#[test]
fn relevant_guid_aliases_never_merge_counts_or_exclude_a_selected_physical_source()
-> super::Result<()> {
    for outside in [false, true] {
        let bytes = aliases(outside, true);
        let forge = DataForge::open(&bytes)?;
        let error = source_scope::records(&forge, &[Kind::Items], None, &|| false)
            .err()
            .ok_or("source aliases were silently collapsed")?;
        let alias = error
            .downcast_ref::<source_scope::Alias>()
            .ok_or("typed source alias absent")?;
        assert_eq!(
            (alias.first.record_index, alias.second.record_index),
            (0, 1),
            "both actual physical source positions are retained, including an outside-selection alias"
        );
        assert_eq!(
            alias.guid,
            forge.record(0).ok_or("source absent")?.guid(),
            "the shared source GUID is metadata, not a replacement for physical identity"
        );
        assert!(
            alias.first.path.is_ok() && alias.second.path.is_ok(),
            "known source paths remain explicit"
        );
    }
    Ok(())
}

#[test]
fn unrelated_sources_and_cancellation_keep_their_existing_scope() -> super::Result<()> {
    let bytes = aliases(true, false);
    let forge = DataForge::open(&bytes)?;
    let records = source_scope::records(&forge, &[Kind::Items], None, &|| false)?;
    assert_eq!(
        records.len(),
        1,
        "an unrelated physical source does not expand the mapped scope"
    );
    assert_eq!(
        records[0].record.index(),
        0,
        "the source remains the exact original Record"
    );
    assert!(
        source_scope::records(&forge, &[Kind::Items], None, &|| true).is_err(),
        "source identity validation remains cancellable"
    );
    assert!(
        audit_scope::from_names(["unknown catalog group"]).is_err(),
        "unknown mappings cannot silently declare zero source coverage"
    );
    Ok(())
}

#[test]
fn the_record_filter_limits_source_accounting_and_keeps_outside_aliases() -> super::Result<()> {
    let filter = Filter::new("/selected");
    let distinct = aliases(false, false);
    let forge = DataForge::open(&distinct)?;
    let rows = source_scope::records(&forge, &[Kind::Items], Some(&filter), &|| false)?;
    let [selected] = rows.as_slice() else {
        return Err("exactly one physical Record matches the audit filter".into());
    };
    assert_eq!(
        selected.record.index(),
        0,
        "the matching original Record is retained"
    );

    let duplicate = aliases(false, true);
    let forge = DataForge::open(&duplicate)?;
    let error = source_scope::records(&forge, &[Kind::Items], Some(&filter), &|| false)
        .err()
        .ok_or("outside-filter source aliases were silently ignored")?;
    assert!(
        error.downcast_ref::<source_scope::Alias>().is_some(),
        "the physical alias stays visible under a filter"
    );
    Ok(())
}

#[test]
fn field_review_follows_owned_storage_but_keeps_link_targets_as_declarations() -> super::Result<()>
{
    let bytes = DataForgeBuilder::new()
        .structure(
            TestStruct::new("Source")
                .field("reference", TestType::ReferenceTo("Referenced".to_owned()))
                .field("weak", TestType::WeakPointer("Referenced".to_owned()))
                .field("owned", TestType::StrongPointer("Owned".to_owned())),
        )
        .structure(TestStruct::new("Referenced").field("otherOwnerField", TestType::Bool))
        .structure(TestStruct::new("Owned").field("ownedField", TestType::Bool))
        .structure(
            TestStruct::new("DerivedOwned")
                .parent("Owned")
                .field("derivedField", TestType::Bool),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let source = super::schema::Schema::read(&forge, &["Source".to_owned()], &|| false)?;
    assert!(
        source.owners.contains_key("Owned") && source.owners.contains_key("DerivedOwned"),
        "owned fields and strong-pointer alternatives are independently reviewed"
    );
    assert!(
        !source.owners.contains_key("Referenced"),
        "link targets need their own inventory roots"
    );
    for name in ["reference", "weak"] {
        let field = super::inventory::FieldKey::new("Source", name);
        assert!(
            source
                .declarations
                .get(&field)
                .is_some_and(|entry| entry.target.as_deref() == Some("Referenced")),
            "the link declaration retains its exact external target type"
        );
    }
    Ok(())
}

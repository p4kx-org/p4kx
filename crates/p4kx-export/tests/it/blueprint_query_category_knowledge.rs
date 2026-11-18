// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    BlueprintSnapshot, CatalogLanguageSelection, CatalogRecordSelection, with_blueprints,
};
use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{Archive, DataForge};
use p4kx_testkit::{TestRecord, TestStruct};
use serde_json::Value;

use super::blueprint_query_fixture as fixture;
use crate::canonical_header::wire::Wire;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

const FOREIGN_PATH: &str = "libs/foundry/records/unrelated/sdk/category_alias.xml";

#[derive(Clone, Copy)]
enum Candidate {
    UnclassifiedCategory,
    ProvenForeign,
}

#[test]
fn unreadable_referenced_category_header_retains_guid_with_incomplete_knowledge() -> TestResult {
    for origin in [DcbOrigin::Plain, DcbOrigin::Encrypted, DcbOrigin::Unknown] {
        verify_candidate(Candidate::UnclassifiedCategory, origin)?;
    }
    Ok(())
}

#[test]
fn unreadable_same_guid_foreign_header_does_not_poison_category_knowledge() -> TestResult {
    for origin in [DcbOrigin::Plain, DcbOrigin::Encrypted, DcbOrigin::Unknown] {
        verify_candidate(Candidate::ProvenForeign, origin)?;
    }
    Ok(())
}

fn verify_candidate(candidate: Candidate, origin: DcbOrigin) -> TestResult {
    let (bytes, physical) = malformed_header(candidate)?;
    let (_folder, archive) = common::open_bytes(bytes, origin)?;
    let cell = canonical_category(&archive)?;
    let complete = matches!(candidate, Candidate::ProvenForeign);
    assert_eq!(
        cell.get("state").and_then(Value::as_str),
        Some("present"),
        "Unclassified candidates do not erase the authored category GUID"
    );
    assert_eq!(
        cell.get("value").and_then(Value::as_str),
        Some(common::guid(fixture::CATEGORY).as_str()),
        "The exact original category GUID survives"
    );
    assert_eq!(
        cell.get("sourceSetComplete").and_then(Value::as_bool),
        Some(complete),
        "Only an unclassifiable possible category affects the derived choice"
    );
    let marker = match origin {
        DcbOrigin::Encrypted => Some(true),
        DcbOrigin::Plain if complete => Some(false),
        DcbOrigin::Plain | DcbOrigin::Unknown => None,
    };
    assert_eq!(
        cell.get("containsEncryptedSource").and_then(Value::as_bool),
        marker,
        "Incomplete plain knowledge cannot establish a wholly plain source set"
    );
    with_blueprints(
        &archive,
        CatalogRecordSelection::Guid(common::guid(fixture::OWNER).parse()?),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| verify_selected(&snapshot, &cell, physical, complete),
    )??;
    Ok(())
}

fn verify_selected(
    snapshot: &BlueprintSnapshot<'_>,
    canonical: &Value,
    physical: u32,
    complete: bool,
) -> TestResult {
    let [presentation] = snapshot.presentations() else {
        return Err("one selected physical Blueprint presentation".into());
    };
    let category = presentation.category();
    assert_eq!(
        category.value().map(|guid| guid.get().to_string()),
        Some(common::guid(fixture::CATEGORY)),
        "The SDK retains the authored GUID independently of target readability"
    );
    assert_eq!(
        category.origin().source_set_complete(),
        complete,
        "The canonical and SDK choice share the same knowledge boundary"
    );
    Wire::new(canonical, snapshot.source_inventory())
        .cell(category, |guid, wire| wire.raw(&guid.get().to_string()))?;
    let expected = if complete { Vec::new() } else { vec![physical] };
    assert_eq!(
        snapshot
            .dependency_coverage()
            .categories()
            .unreadable_headers,
        expected,
        "Only the exact unclassified possible category remains a coverage gap"
    );
    assert!(
        snapshot
            .dependency_coverage()
            .categories()
            .unreadable_bodies
            .is_empty(),
        "An unreadable header does not claim a failed body read"
    );
    Ok(())
}

fn canonical_category(archive: &Archive) -> TestResult<Value> {
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value = serde_json::from_slice(build_catalog(archive, &options)?.json())?;
    let owner = common::guid(fixture::OWNER);
    let physical = catalog
        .pointer("/definitions/blueprints/rows")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter().find(|row| {
                row.pointer("/guid/value").and_then(Value::as_str) == Some(owner.as_str())
            })
        })
        .and_then(|row| row.pointer("/recordIndex/value"))
        .and_then(Value::as_u64)
        .ok_or("selected physical Blueprint index")?;
    catalog
        .pointer("/projections/blueprintPresentation/rows")
        .and_then(Value::as_array)
        .and_then(|rows| {
            rows.iter().find(|row| {
                row.pointer("/blueprint/recordIndex")
                    .and_then(Value::as_u64)
                    == Some(physical)
            })
        })
        .and_then(|row| row.get("category"))
        .cloned()
        .ok_or_else(|| "canonical category choice".into())
}

fn malformed_header(candidate: Candidate) -> TestResult<(Vec<u8>, u32)> {
    let path = match candidate {
        Candidate::UnclassifiedCategory => {
            "libs/foundry/records/crafting/blueprintcategories/sdk/unclassified_alias.xml"
        }
        Candidate::ProvenForeign => FOREIGN_PATH,
    };
    let database = fixture::database()
        .structure(TestStruct::new("SdkUnclassifiedCategoryAlias"))
        .record(TestRecord::new(
            fixture::CATEGORY,
            "SdkUnclassifiedCategoryAlias",
            "SdkUnclassifiedCategoryAlias.Original",
            path,
        ));
    let mut bytes = database.build();
    let physical = {
        let forge = DataForge::open(&bytes)?;
        let guid = common::guid(fixture::CATEGORY).parse()?;
        forge
            .records()
            .find(|record| {
                record.guid() == guid
                    && record
                        .record_path()
                        .is_ok_and(|original| Some(original) == path.strip_suffix(".xml"))
            })
            .map(|record| record.index())
            .ok_or("exact candidate header")?
    };
    corrupt_type(&mut bytes, physical)?;
    {
        let forge = DataForge::open(&bytes)?;
        let record = forge.record(physical).ok_or("malformed physical Record")?;
        assert!(
            record.record_path().is_ok(),
            "The exact path remains readable"
        );
        assert!(
            record.type_name().is_err(),
            "The actual candidate type header is unreadable"
        );
        let target = forge
            .record_by_guid(common::guid(fixture::CATEGORY).parse()?)
            .ok_or("first authored reference target")?;
        assert!(
            target.type_name().is_ok(),
            "The independently authored first reference target remains readable"
        );
    }
    Ok((bytes, physical))
}
fn corrupt_type(bytes: &mut [u8], physical: u32) -> TestResult {
    let index = {
        let forge = DataForge::open(bytes)?;
        forge
            .record(physical)
            .ok_or("exact physical Record")?
            .struct_type()?
            .index()
    };
    let header_words = 9 + p4kx_extraction::ValueKind::ALL.len() + 2;
    let struct_bytes = 16;
    let name_field = 0;
    let offset =
        header_words * size_of::<u32>() + usize::try_from(index)? * struct_bytes + name_field;
    bytes
        .get_mut(offset..offset + size_of::<u32>())
        .ok_or("original Struct name offset")?
        .copy_from_slice(&u32::MAX.to_le_bytes());
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, MissionLocationTemplateSnapshot,
    with_mission_location_templates,
};
use std::cell::Cell;
use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_export::{
    CatalogError, CatalogFile, CatalogPublicationMode, Compression, build_catalog,
    write_catalog_from,
};
use p4kx_extraction::{Archive, DataForgeEntry, SourceValueState};
use serde_json::Value;

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[path = "mission_location_source_fixture.rs"]
mod fixture;
#[path = "mission_location_source_proof.rs"]
mod proof;

#[test]
fn location_identity_flags_names_and_cluster_links_have_direct_original_cells() -> TestResult {
    for (origin, marker) in [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ] {
        for case in fixture::ALL {
            let (_folder, archive) = common::open(&fixture::database(case), origin)?;
            with_mission_location_templates(
                &archive,
                CatalogRecordSelection::Guid(common::guid(fixture::OWNER).parse()?),
                CatalogLanguageSelection::Selected(&[]),
                None,
                |snapshot| proof::verify(&snapshot, case, marker),
            )??;
        }
    }
    Ok(())
}

#[test]
fn equal_location_guids_preserve_physical_ties_and_original_header_locations() -> TestResult {
    let (_folder, archive) = common::open(&fixture::duplicated(), DcbOrigin::Plain)?;
    with_mission_location_templates(
        &archive,
        CatalogRecordSelection::Guid(common::guid(fixture::OWNER).parse()?),
        CatalogLanguageSelection::Selected(&[]),
        None,
        |snapshot| duplicate_rows(&snapshot),
    )??;
    Ok(())
}

fn duplicate_rows(snapshot: &MissionLocationTemplateSnapshot<'_>) -> TestResult {
    let [first, second] = snapshot.rows.as_slice() else {
        return Err("both physically distinct equal-GUID locations".into());
    };
    assert_eq!(
        first.guid(),
        second.guid(),
        "original nominal identity remains equal"
    );
    assert_eq!(
        first.disabled().value(),
        Some(&false),
        "first physical tie remains first"
    );
    assert_eq!(
        second.disabled().value(),
        Some(&true),
        "second physical tie remains second"
    );
    assert!(
        first.record_index() < second.record_index(),
        "stable original tie order"
    );
    assert_ne!(
        first.source().guid().origin().source_uses_refs(),
        second.source().guid().origin().source_uses_refs(),
        "header observations are not collapsed by nominal GUID",
    );
    Ok(())
}

#[test]
fn malformed_location_flags_remain_diagnostic_and_refuse_normal_publication() -> TestResult {
    let (folder, archive) = common::open(
        &fixture::database(fixture::Case::InvalidFlag),
        DcbOrigin::Plain,
    )?;
    assert!(
        matches!(
            build_catalog(&archive, &common::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "normal publication cannot hide the original malformed Bool"
    );
    refusal_before_output(&archive, folder.path())?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    let document: Value = serde_json::from_slice(built.json())?;
    let cell = document
        .pointer("/missionLocationTemplates/0/disabled")
        .ok_or("original diagnostic flag")?;
    assert_eq!(
        cell.get("state").and_then(Value::as_str),
        Some("invalid"),
        "the actual malformed field remains typed"
    );
    assert_eq!(
        cell.get("containsEncryptedSource").and_then(Value::as_bool),
        Some(false),
        "original plain source is still known"
    );
    crate::commodity_v3_source_inventory::reference_closure(&document)
}

#[test]
fn absent_and_empty_original_names_are_distinct_typed_collection_states() -> TestResult {
    for (case, expected) in [
        (fixture::Case::Sparse, SourceValueState::Absent),
        (fixture::Case::EmptyNames, SourceValueState::Present),
        (fixture::Case::NullNames, SourceValueState::Null),
    ] {
        let (_folder, archive) = common::open(&fixture::database(case), DcbOrigin::Plain)?;
        with_mission_location_templates(
            &archive,
            CatalogRecordSelection::All,
            CatalogLanguageSelection::Selected(&[]),
            None,
            |snapshot| {
                let row = snapshot.rows.first().ok_or("one original location")?;
                assert_eq!(row.names().state(), expected, "source collection state");
                if let Some(names) = row.names().value() {
                    assert!(
                        names.is_empty(),
                        "authored empty collection remains present"
                    );
                }
                Ok::<_, Box<dyn Error>>(())
            },
        )??;
    }
    Ok(())
}

fn refusal_before_output(archive: &Archive, directory: &Path) -> TestResult {
    let entry = DataForgeEntry::read(archive)?;
    for existing in [false, true] {
        let output = directory.join(format!("location-source-{existing}.json"));
        if existing {
            fs::write(&output, b"previous complete location output")?;
        }
        let called = Cell::new(false);
        let result = write_catalog_from(
            archive,
            &entry,
            &common::options(),
            Compression::None,
            || {
                called.set(true);
                CatalogFile::create(&output)
            },
        );
        assert!(
            matches!(result, Err(CatalogError::IncompleteExport(_))),
            "the public writer retains its typed normal refusal"
        );
        assert!(!called.get(), "refusal precedes destination creation");
        if existing {
            assert_eq!(
                fs::read(&output)?,
                b"previous complete location output",
                "last complete output remains unchanged"
            );
        } else {
            assert!(!output.exists(), "no refused fresh destination exists");
        }
    }
    Ok(())
}

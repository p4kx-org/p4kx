// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;
use std::time::UNIX_EPOCH;

use p4kx_export::{
    CatalogOptions, ProfileDocument, ProfileKind, ProfileState, build_catalog,
    read_client_profile_settings_for_with_provenance_mode,
};
use p4kx_extraction::{Archive, Filter, InputScope, SourceInventory, SourceUseRole};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, test_key};
use serde_json::Value;

use super::super::CLIENT_PROFILE_DOCUMENTS;
use super::{Route, TestResult};
use crate::sdk_full_proof as full_proof;

const GENERAL: &str = "Data/Libs/Config/Profiles/default/attributes.xml";
const VIRTUAL_REALITY: &str = "Data/Libs/Config/Profiles/default/attributesVR.xml";

pub(super) fn read() -> TestResult<Route> {
    let directory = tempfile::tempdir()?;
    let archive = fixture(directory.path())?;
    let report = read_client_profile_settings_for_with_provenance_mode(
        &archive,
        ProfileKind::VirtualReality,
        full_proof::MODE,
        None,
    )?;
    let documents = report.settings().documents();
    let [general, virtual_reality] = documents else {
        return Err("the two canonical profile documents must be retained".into());
    };
    verify_unselected(general, report.provenance())?;
    verify_selected(virtual_reality)?;
    verify_filtered_catalog(&archive, documents)?;
    Route::queried_rows(
        CLIENT_PROFILE_DOCUMENTS,
        documents,
        "scoped ProfileDocuments and filtered synthetic producer parity witnessed; complete installed producer parity open",
    )
}

fn verify_unselected(document: &ProfileDocument, inventory: &SourceInventory) -> TestResult<()> {
    assert_eq!(document.kind(), ProfileKind::General, "first profile role");
    assert_eq!(
        document.status().value(),
        Some(&ProfileState::Filtered),
        "the unselected profile is represented by directory evidence"
    );
    assert!(document.elements().is_empty(), "no sibling elements");
    let [candidate] = document.candidates() else {
        return Err("the unselected physical candidate must be retained".into());
    };
    assert!(
        !inventory.iter_uses().any(|row| {
            row.source() == candidate.input_ref()
                && (row.scope() == InputScope::Payload || row.role() == SourceUseRole::Attempted)
        }),
        "the unreadable sibling was never opened"
    );
    Ok(())
}

fn verify_selected(document: &ProfileDocument) -> TestResult<()> {
    assert_eq!(
        document.kind(),
        ProfileKind::VirtualReality,
        "second profile role"
    );
    assert_eq!(
        document.status().value(),
        Some(&ProfileState::Read),
        "selected profile"
    );
    let [_, attribute] = document.elements() else {
        return Err("the selected profile must retain both XML elements".into());
    };
    let [_, declaration] = attribute.declarations() else {
        return Err("the attribute's name and value declarations must be retained".into());
    };
    let value = declaration.value().value();
    assert_eq!(
        value.value().map(String::as_str),
        Some("1"),
        "the typed row retains the original assigned value"
    );
    assert_eq!(
        value.origin().contains_encrypted_source(),
        Some(true),
        "the selected value carries its encrypted payload origin"
    );
    Ok(())
}

fn verify_filtered_catalog(archive: &Archive, documents: &[ProfileDocument]) -> TestResult<()> {
    let filter = Filter::parse(VIRTUAL_REALITY)?;
    let catalog = build_catalog(
        archive,
        CatalogOptions {
            filter: Some(&filter),
            ..CatalogOptions::new(UNIX_EPOCH)
        }
        .with_full_provenance(),
    )?;
    let document: Value = serde_json::from_slice(catalog.json())?;
    let queried = serde_json::to_value(documents)?;
    assert_eq!(
        document.pointer(CLIENT_PROFILE_DOCUMENTS),
        Some(&queried),
        "filtered Catalog rows retain profile order, values and direct origins"
    );
    Ok(())
}

fn fixture(directory: &Path) -> TestResult<Archive> {
    let path = directory.join("client_profiles.p4k");
    let dataforge = DataForgeBuilder::new()
        .structure(TestStruct::new("Fixture"))
        .record(TestRecord::new(
            1,
            "Fixture",
            "Fixture.One",
            "fixture/one.xml",
        ))
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", dataforge))
        .entry(TestEntry::stored(GENERAL, b"<Attributes/>").with_crc32c(1))
        .entry(
            TestEntry::stored(
                VIRTUAL_REALITY,
                b"<Attributes><Attr name='fixture' value='1'/></Attributes>",
            )
            .encrypted(),
        )
        .write_to(&path)?;
    Ok(Archive::open_with_key(path, Some(test_key()))?)
}

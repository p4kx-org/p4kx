// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordSelection, ContractGeneratorSnapshot,
    with_contract_generators,
};
use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::{Archive, DataForgeEntry};
use serde_json::Value;

#[path = "catalog_contract_generator_scope.rs"]
mod scope;
#[path = "catalog_source_arena_access.rs"]
mod source_access;

use super::TestResult;
use crate::commodity_v3_fixture::{self, DcbOrigin, guid};
use crate::contracts_fixture;
use crate::health_origin_assertions as proof;
use crate::sdk_full_proof as full_proof;

#[test]
fn selected_generator_definitions_match_canonical_catalog() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        contracts_fixture::write_archive(folder.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let mut options = commodity_v3_fixture::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, options.with_full_provenance())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let expected = proof::rows(&catalog, "/definitions/contractGenerators/rows")?;
    let entry = DataForgeEntry::read(&archive)?;
    let session = full_proof::from_entry(
        &archive,
        &entry,
        CatalogLanguageSelection::AllAvailable,
        None,
    )?;
    let parsed = session.parsed(None)?;
    parsed.with_contract_generators(CatalogRecordSelection::All, None, |snapshot| {
        assert_eq!(
            serde_json::to_value(&snapshot.rows)?,
            serde_json::to_value(expected)?,
            "the selected source dataset uses the canonical Generator projector"
        );
        Ok::<_, Box<dyn std::error::Error>>(())
    })??;
    for selection in [
        CatalogRecordSelection::Guid(guid(contracts_fixture::GENERATOR).parse()?),
        CatalogRecordSelection::RecordIndex(expected_index(expected)?),
    ] {
        parsed.with_contract_generators(selection, None, |snapshot| {
            verify_selected(&snapshot, expected)
        })??;
    }
    Ok(())
}

fn expected_index(expected: &[Value]) -> TestResult<u32> {
    let record = expected
        .iter()
        .find(|row| {
            row.pointer("/guid/value") == Some(&Value::from(guid(contracts_fixture::GENERATOR)))
        })
        .ok_or("the canonical selected Generator")?;
    Ok(u32::try_from(
        record
            .pointer("/recordIndex/value")
            .and_then(Value::as_u64)
            .ok_or("the Generator's physical index")?,
    )?)
}

fn verify_selected(snapshot: &ContractGeneratorSnapshot<'_>, expected: &[Value]) -> TestResult {
    let [row] = snapshot.rows.as_slice() else {
        return Err("one selected physical Generator".into());
    };
    let physical = row
        .header()
        .record_index()
        .value()
        .ok_or("physical Generator position")?;
    let expected = expected
        .iter()
        .find(|row| {
            row.pointer("/recordIndex/value").and_then(Value::as_u64) == Some(u64::from(*physical))
        })
        .ok_or("the matching canonical physical Generator")?;
    assert_eq!(
        serde_json::to_value(row)?,
        *expected,
        "Guid and physical Record selection preserve the same source row"
    );
    Ok(())
}

#[test]
fn generator_source_arena_has_typed_external_access_for_every_origin() -> TestResult {
    for (origin, encrypted) in [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ] {
        let (_folder, archive) = commodity_v3_fixture::open(&source_access::database(), origin)?;
        with_contract_generators(
            &archive,
            CatalogRecordSelection::Guid(guid(source_access::OWNER).parse()?),
            CatalogLanguageSelection::AllAvailable,
            None,
            |snapshot| {
                let [row] = snapshot.rows.as_slice() else {
                    return Err("one selected original owner".into());
                };
                let first = row
                    .source()
                    .values()
                    .first()
                    .ok_or("one original source value")?;
                assert_eq!(
                    first.value().origin().contains_encrypted_source(),
                    encrypted,
                    "each original source value retains its own encryption evidence"
                );
                source_access::verify(row.source(), &snapshot.source_inventory)
            },
        )??;
    }
    Ok(())
}

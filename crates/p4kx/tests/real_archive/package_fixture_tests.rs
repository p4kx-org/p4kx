// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::Entities;
use p4kx_extraction::{DataForge, Record};
use p4kx_testkit::media_fixture::Candidate;
use p4kx_testkit::{DataForgeBuilder, TestEntity};

use crate::media_fixtures::Identity;
use crate::{package_fault, package_fixtures};

fn candidate(record: &Record<'_>, class: &str) -> package_fault::Result<Candidate<Identity>> {
    Ok(Candidate::new(
        Identity {
            guid: record.guid(),
            record_path: record.record_path()?.to_owned(),
            record_index: record.index(),
        },
        class.to_owned(),
    ))
}

#[test]
fn cli_argument_resolves_the_exact_physical_guid_alias() -> package_fault::Result<()> {
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(TestEntity::new(1, "Family_First", "synthetic/first.xml").record())
        .record(TestEntity::new(1, "Family_Second", "synthetic/second.xml").record())
        .build();
    let forge = DataForge::open(&bytes)?;
    let entities = Entities::new(&forge);
    let second = forge.record(1).ok_or("second physical record missing")?;
    let argument = package_fixtures::argument(&entities, &candidate(&second, "Family_Second")?)?;
    let resolved = entities.find(&argument)?;
    assert_eq!(
        resolved.index(),
        1,
        "the CLI selector must not perform GUID first-wins lookup"
    );
    assert_eq!(
        resolved.guid(),
        second.guid(),
        "the exact selected GUID is retained"
    );
    assert_eq!(
        resolved.record_path()?,
        second.record_path()?,
        "the original source path is retained"
    );
    Ok(())
}

#[test]
fn an_ambiguous_class_uses_only_a_unique_exact_physical_path() -> package_fault::Result<()> {
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(TestEntity::new(1, "Family_Repeated", "synthetic/first.xml").record())
        .record(TestEntity::new(1, "Family_Repeated", "synthetic/second.xml").record())
        .build();
    let forge = DataForge::open(&bytes)?;
    let entities = Entities::new(&forge);
    let second = forge.record(1).ok_or("second physical record missing")?;
    let argument = package_fixtures::argument(&entities, &candidate(&second, "Family_Repeated")?)?;
    assert_eq!(
        argument,
        second.record_path()?,
        "an ambiguous class is not passed to the CLI"
    );
    assert_eq!(
        entities.find(&argument)?.index(),
        1,
        "the fallback argument is still exact physical provenance"
    );
    Ok(())
}

#[test]
fn ambiguous_class_and_path_fail_without_exposing_the_sources() -> package_fault::Result<()> {
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(TestEntity::new(1, "PRIVATE_SENTINEL", "synthetic/PRIVATE_SENTINEL.xml").record())
        .record(TestEntity::new(1, "PRIVATE_SENTINEL", "synthetic/PRIVATE_SENTINEL.xml").record())
        .build();
    let forge = DataForge::open(&bytes)?;
    let entities = Entities::new(&forge);
    let second = forge.record(1).ok_or("second physical record missing")?;
    let error = package_fixtures::argument(&entities, &candidate(&second, "PRIVATE_SENTINEL")?)
        .err()
        .ok_or("ambiguous physical selectors accepted")?;
    assert!(
        !format!("{error:?} {error}").contains("PRIVATE_SENTINEL"),
        "a private class/path must not enter failure formatting"
    );
    assert!(
        error.source().is_none(),
        "generic error-chain formatting cannot reveal retained lookup errors"
    );
    Ok(())
}

#[test]
fn a_foreign_class_cannot_reuse_an_otherwise_matching_source_identity() -> package_fault::Result<()>
{
    let bytes = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(TestEntity::new(1, "Family_Original", "synthetic/original.xml").record())
        .build();
    let forge = DataForge::open(&bytes)?;
    let entities = Entities::new(&forge);
    let record = forge.record(0).ok_or("source record missing")?;
    let foreign = candidate(&record, "Family_Foreign")?;
    assert!(
        package_fixtures::argument(&entities, &foreign).is_err(),
        "matching index/GUID/path never authorize a different class"
    );
    Ok(())
}

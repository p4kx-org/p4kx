// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::{
    CatalogGuid, CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
    ContractSnapshot, ParsedCatalogQueries, SourcePayload,
};
use p4kx_extraction::{
    DataForge, DataForgeError, FieldOrigin, SourceInventory, SourceLocation, SourceUseDescription,
    SourceUseRole, SourcedValue,
};

use super::TestResult;
use crate::commodity_v3_fixture::{self, DcbOrigin};

use super::query_source_fixture as fixture;

type IntroMatches = SourcedValue<Vec<SourcedValue<CatalogGuid>>>;

const SELECTED: u32 = 1;
const OUTSIDE: u32 = 2;

#[test]
fn contract_sdk_negative_reverse_survey_has_the_actual_false_outcome_witness() -> TestResult {
    let bytes = fixture::database_with(fixture::Options {
        successful_grant: false,
        include_collision: false,
        ..fixture::Options::default()
    });
    let (_folder, archive) = commodity_v3_fixture::open_bytes(bytes, DcbOrigin::Plain)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?
            .with_provenance_mode(full_proof::MODE);
    let parsed = session.parsed(None)?;
    let negative = negative_outcome(&parsed)?;
    parsed.with_contracts(CatalogRecordSelection::RecordIndex(SELECTED), None, |snapshot| {
        let origin = empty_intro(&snapshot)?.origin();
        assert!(origin.source_set_complete(), "a complete source survey proves absence of an admitted grant");
        assert_eq!(origin.contains_encrypted_source(), Some(false), "the complete negative proof uses plain original inputs");
        let uses = contributing_uses(origin, &snapshot.source_inventory)?;
        assert!(uses.iter().any(|used| used.role() == SourceUseRole::Selector && matches!(used.location(), SourceLocation::Subresource {namespace, indices, field}
            if namespace == "dataforgeValue" && indices.as_slice() == [u64::from(OUTSIDE), negative] && field == "$value")),
            "the empty result cites the actual outside missionResults[0]=false occurrence");
        assert!(snapshot.dependency_generators.is_empty(), "the negative witness does not activate a whole unrelated Generator projector");
        Ok::<_, Box<dyn std::error::Error>>(())
    })??;
    Ok(())
}

#[test]
fn contract_sdk_unreadable_reverse_owner_keeps_empty_intros_incomplete_and_unknown() -> TestResult {
    let bytes = fixture::database_with(fixture::Options {
        outside_instance: Some(u16::MAX),
        ..fixture::Options::default()
    });
    let forge = DataForge::open(&bytes)?;
    let source = forge.record(OUTSIDE).ok_or("the original outside owner")?;
    assert!(
        source.record_path().is_ok() && source.type_name().is_ok(),
        "the failure belongs to the body rather than its selection headers"
    );
    let actual = source
        .tree()
        .expect_err("the declared instance does not exist");
    let outside_position = usize::try_from(OUTSIDE)?;
    assert!(
        matches!(&actual, DataForgeError::IndexOutOfRange {part: "Record", index, target: "instance", target_index} if *index == outside_position && *target_index == u64::from(u16::MAX)),
        "the source fixture actually fails its canonical body read"
    );
    let (_folder, archive) = commodity_v3_fixture::open_bytes(bytes, DcbOrigin::Plain)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?
            .with_provenance_mode(full_proof::MODE);
    session.with_contracts(CatalogRecordSelection::RecordIndex(SELECTED), None, |snapshot| {
        let origin = empty_intro(&snapshot)?.origin();
        assert!(!origin.source_set_complete(), "an unreadable reverse source cannot prove absence of a grant");
        assert_eq!(origin.contains_encrypted_source(), None, "incomplete plain evidence must not be labeled publicly unencrypted");
        let failure = snapshot.coverage.dependencies.survey_failures.iter().find(|failure| failure.record().index() == OUTSIDE).ok_or("the real failed reverse body")?;
        assert_eq!(failure.canonical_error(), Some(&actual), "coverage retains the exact original source error");
        let uses = contributing_uses(origin, &snapshot.source_inventory)?;
        for field_name in ["fileName", "sourceType"] {
            assert!(uses.iter().any(|used| used.role() == SourceUseRole::Selector && matches!(used.location(), SourceLocation::Subresource {namespace, indices, field}
                if namespace == "dataforgeRecordHeader" && indices.as_slice() == [u64::from(OUTSIDE)] && field == field_name)),
                "the incomplete join retains the actual readable outside {field_name} selector");
        }
        Ok::<_, Box<dyn std::error::Error>>(())
    })??;
    Ok(())
}

fn negative_outcome(parsed: &ParsedCatalogQueries<'_>) -> TestResult<u64> {
    parsed.with_contract_generators(
        CatalogRecordSelection::RecordIndex(OUTSIDE),
        None,
        |snapshot| {
            let [owner] = snapshot.rows.as_slice() else {
                return Err("one raw outside source owner".into());
            };
            let mut values = owner
                .source()
                .values()
                .iter()
                .enumerate()
                .filter(|(_, node)| {
                    matches!(
                        node.value().value(),
                        Some(SourcePayload::Boolean { value: false })
                    )
                });
            let (position, _) = values
                .next()
                .ok_or("the original unsuccessful mission outcome")?;
            assert!(
                values.next().is_none(),
                "the fixture has one unambiguous false outcome occurrence"
            );
            Ok::<_, Box<dyn std::error::Error>>(u64::try_from(position)?)
        },
    )?
}

fn empty_intro<'read>(snapshot: &'read ContractSnapshot<'_>) -> TestResult<&'read IntroMatches> {
    let parent = snapshot
        .rows
        .iter()
        .find(|row| row.base.sub_contract_guid.value().is_none())
        .ok_or("the selected parent contract")?;
    let [required] = parent
        .required_intros
        .value()
        .ok_or("the original required tag")?
        .as_slice()
    else {
        return Err("one required completion tag".into());
    };
    assert!(
        required.intro_contracts.value().is_some_and(Vec::is_empty),
        "no usable source supplies the requested intro tag"
    );
    Ok(&required.intro_contracts)
}

fn contributing_uses<'read>(
    origin: &FieldOrigin,
    inventory: &'read SourceInventory,
) -> TestResult<Vec<&'read SourceUseDescription>> {
    origin
        .source_uses_refs()
        .iter()
        .map(|reference| {
            inventory
                .iter_uses()
                .find(|used| used.id() == reference)
                .ok_or_else(|| {
                    "a field's source-use reference is outside its result inventory".into()
                })
        })
        .collect()
}

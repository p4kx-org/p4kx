// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::CatalogRecordSelection;
use p4kx_export::catalog_queries::equipment::ItemSnapshot;
use p4kx_extraction::{
    DataForge, DataForgeError, SourceLocation, SourceUseDescription, SourceUseRole,
    SourceValueState,
};
use p4kx_testkit::{TestRecord, TestStruct, TestType};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::variants_fixture::item_rank as fixture;

const CANDIDATE_INDEX: u32 = 5;
const CANDIDATE_GUID: u128 = 0x2000_0000_0000_4000_8000_0000_0000_0110;
const NON_ITEM: &str = "FixtureVariantNonItem";
const ENTITY: &str = "EntityClassDefinition";
const ITEM_PATH: &str = "libs/foundry/records/entities/scitem/fixture_header.xml";
const OUTSIDE_PATH: &str = "fixture/variant_header.xml";

#[derive(Clone, Copy)]
enum Fault {
    FileName,
    SourceType,
}

impl Fault {
    const fn field(self) -> &'static str {
        match self {
            Self::FileName => "fileName",
            Self::SourceType => "sourceType",
        }
    }
}

#[derive(Clone, Copy)]
enum Scope {
    PossibleItem,
    Excluded,
}

#[test]
fn disjoint_headers_cannot_make_a_known_item_variant_incomplete() -> TestResult {
    for fault in [Fault::FileName, Fault::SourceType] {
        let bytes = fixture_bytes(fault, Scope::Excluded)?;
        let (_folder, archive) = common::open_bytes(bytes, DcbOrigin::Encrypted)?;
        for selection in [
            CatalogRecordSelection::RecordIndex(fixture::SELECTED_RECORD_INDEX),
            CatalogRecordSelection::All,
        ] {
            full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?.with_items(
                selection,
                None,
                |snapshot| check_exclusion(&snapshot, fault),
            )??;
        }
    }
    Ok(())
}

fn check_exclusion(snapshot: &ItemSnapshot<'_>, fault: Fault) -> TestResult {
    assert!(
        snapshot.coverage().unreadable_headers.is_empty()
            && snapshot.coverage().incomplete_fields.is_empty(),
        "a disproved Item candidate cannot affect Item query coverage"
    );
    let row = selected(snapshot)?;
    let variant = row.variant_of();
    let base = variant.value().ok_or("known Item variant base")?;
    assert_eq!(
        base.target_record_index().value(),
        Some(&fixture::BASE_RECORD_INDEX),
        "the original complete cohort retains its physical winner"
    );
    assert!(
        variant.origin().source_set_complete() && row.variant_failed_source_uses().is_empty(),
        "an exclusion supplies no failed input to the variant decision"
    );
    let attempted = header_use(snapshot, fault.field(), SourceUseRole::Attempted);
    match fault {
        Fault::FileName => {
            assert!(attempted.is_some(), "the filename read was attempted");
            let exclusion = header_use(snapshot, "sourceType", SourceUseRole::Selector)
                .ok_or("the original non-Item type selector")?;
            assert!(
                variant.origin().source_uses_refs().contains(exclusion.id()),
                "the exact readable type proves exclusion from the family"
            );
        }
        Fault::SourceType => assert!(
            attempted.is_none(),
            "the readable outside path excludes the Record before its type is consulted"
        ),
    }
    Ok(())
}

#[test]
fn relevant_header_failures_keep_their_original_attempts_and_no_variant_winner() -> TestResult {
    for fault in [Fault::FileName, Fault::SourceType] {
        let bytes = fixture_bytes(fault, Scope::PossibleItem)?;
        let (_folder, archive) = common::open_bytes(bytes, DcbOrigin::Encrypted)?;
        full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?.with_items(
            CatalogRecordSelection::RecordIndex(fixture::SELECTED_RECORD_INDEX),
            None,
            |snapshot| check_relevant_failure(&snapshot, fault),
        )??;
    }
    Ok(())
}

fn check_relevant_failure(snapshot: &ItemSnapshot<'_>, fault: Fault) -> TestResult {
    let row = selected(snapshot)?;
    let variant = row.variant_of();
    assert_eq!(
        variant.state(),
        SourceValueState::Unreadable,
        "the unreadable original header can hide a competing Item"
    );
    assert!(
        variant.value().is_none() && !variant.origin().source_set_complete(),
        "an undecidable admission cannot support a complete winner"
    );
    assert_eq!(
        snapshot.coverage().incomplete_fields.as_slice(),
        &[fixture::SELECTED_RECORD_INDEX],
        "the selected original Item retains its partial derivation coverage"
    );
    let attempted = header_use(snapshot, fault.field(), SourceUseRole::Attempted)
        .ok_or("the physical failed header read")?;
    assert_eq!(
        row.variant_failed_source_uses(),
        std::slice::from_ref(attempted.id()),
        "only the relevant physical header failure belongs to the decision"
    );
    assert!(
        !variant.origin().source_uses_refs().contains(attempted.id()),
        "an attempted read remains separate from contributing selectors"
    );
    Ok(())
}

fn selected<'snapshot, 'source>(
    snapshot: &'snapshot ItemSnapshot<'source>,
) -> TestResult<&'snapshot p4kx_export::Item<'source>> {
    snapshot
        .rows()
        .iter()
        .find(|row| {
            row.source().source().record_index().value() == Some(&fixture::SELECTED_RECORD_INDEX)
        })
        .ok_or_else(|| "selected physical Item".into())
}

fn header_use<'snapshot>(
    snapshot: &'snapshot ItemSnapshot<'_>,
    expected_field: &str,
    expected_role: SourceUseRole,
) -> Option<&'snapshot SourceUseDescription> {
    snapshot.source_inventory().iter_uses().find(|used| {
        used.role() == expected_role
            && matches!(
                used.location(),
                SourceLocation::Subresource { namespace, indices, field }
                    if namespace == "dataforgeRecordHeader"
                        && indices.as_slice() == [u64::from(CANDIDATE_INDEX)]
                        && field == expected_field
            )
    })
}

fn fixture_bytes(fault: Fault, scope: Scope) -> TestResult<Vec<u8>> {
    let (source_type, path) = match (fault, scope) {
        (Fault::FileName, Scope::PossibleItem) => (ENTITY, ITEM_PATH),
        (Fault::SourceType, Scope::PossibleItem) => (NON_ITEM, ITEM_PATH),
        (_, Scope::Excluded) => (NON_ITEM, OUTSIDE_PATH),
    };
    let mut bytes = fixture::database()
        .structure(TestStruct::new(NON_ITEM).field("flag", TestType::Bool))
        .record(TestRecord::new(
            CANDIDATE_GUID,
            source_type,
            "NewHeader",
            path,
        ))
        .build();
    let byte = {
        let forge = DataForge::open(&bytes)?;
        let candidate = forge
            .record(CANDIDATE_INDEX)
            .ok_or("original fixture Record")?;
        let text = match fault {
            Fault::FileName => candidate.stored_file_name()?,
            Fault::SourceType => candidate.type_name()?,
        };
        text.as_ptr()
            .addr()
            .checked_sub(bytes.as_ptr().addr())
            .ok_or("the exact original header string lies inside DataForge")?
    };
    *bytes
        .get_mut(byte)
        .ok_or("physical byte of the header string")? = 0xff;
    verify_source_pair(&bytes, fault, source_type, path)?;
    Ok(bytes)
}

fn verify_source_pair(bytes: &[u8], fault: Fault, source_type: &str, path: &str) -> TestResult {
    let forge = DataForge::open(bytes)?;
    let candidate = forge
        .record(CANDIDATE_INDEX)
        .ok_or("physical fixture Record after damage")?;
    let failure = match fault {
        Fault::FileName => {
            assert_eq!(
                candidate.type_name()?,
                source_type,
                "the fixture retains its independently readable type"
            );
            candidate.record_path()
        }
        Fault::SourceType => {
            assert_eq!(
                candidate.stored_file_name()?,
                path,
                "the fixture retains its independently readable path"
            );
            candidate.type_name()
        }
    };
    assert!(
        matches!(failure, Err(DataForgeError::InvalidString { .. })),
        "the fixture damages exactly the intended physical header field"
    );
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, DataForge, Filter, Guid, SourceLocation, SourceUseRole, SourceUses,
    SourceValueState,
};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::{AncestorFacts, AncestorOwner};
use crate::build::original_input::test_support::{self, TestResult};
use crate::guid_text::CatalogGuid;
use crate::harvest_locations::{NoObject, Objects};
use crate::starmap_containers::Placements;
use crate::{CatalogError, ExportAccountingError};

const RECORD_TYPE: &str = "StarMapObject";

#[test]
fn demanded_aliases_keep_definition_order_and_read_failures() -> TestResult {
    test_support::with_projection(&fixture(), |forge, projection| {
        let [repeated, partial, unreadable, undemanded] =
            [0, 2, 4, 5].map(|index| guid(forge, index));
        let (repeated, partial, unreadable, undemanded) =
            (repeated?, partial?, unreadable?, undemanded?);
        let facts = AncestorFacts::read(
            forge,
            projection.original(),
            [repeated, partial, unreadable],
            &Placements::default(),
            None,
            None,
        )?;
        assert_eq!(
            facts.owner(Some(repeated)),
            AncestorOwner::Ambiguous(1),
            "both physical aliases survive and the producer uses the last one"
        );
        assert_eq!(
            facts.owner(Some(partial)),
            AncestorOwner::Unavailable(Some(3)),
            "a readable sibling cannot close a failed competing physical alias"
        );
        assert_eq!(
            facts.owner(Some(unreadable)),
            AncestorOwner::Unavailable(Some(4)),
            "unreadable description is distinct from a non-marker"
        );
        assert_eq!(
            facts.owner(Some(undemanded)),
            AncestorOwner::Unavailable(None),
            "unread demands carry no complete absence proof"
        );
        assert_eq!(
            facts.coverage.unreadable_bodies,
            [2],
            "only demanded body failure"
        );
        assert_eq!(
            facts.coverage.incomplete_fields,
            [4],
            "the original description failure"
        );
        assert_alias_receipts(&facts, repeated)?;
        let objects = facts.objects();
        let sources = &projection.original().origins().uses;
        for selected_guid in [repeated, partial] {
            assert_incomplete_origin(&objects, selected_guid, sources)?;
        }
        Ok(())
    })
}

#[test]
fn unreadable_alias_retains_its_exact_failed_filename() -> TestResult {
    let mut bytes = fixture();
    corrupt_filename(&mut bytes, 1)?;
    test_support::with_projection(&bytes, |forge, projection| {
        let repeated = guid(forge, 0)?;
        let input = projection.original();
        let facts =
            AncestorFacts::read(forge, input, [repeated], &Placements::default(), None, None)?;
        assert_eq!(
            facts.owner(Some(repeated)),
            AncestorOwner::Unavailable(Some(0))
        );
        assert_eq!(facts.coverage.unreadable_headers, [1]);
        let [attempted] = facts.failed_source_uses.as_slice() else {
            return Err("the single failed header observation".into());
        };
        let inventory = input
            .origins()
            .uses
            .finish_with(|| Ok::<(), CatalogError>(()))?;
        assert!(inventory.iter_uses().any(|used| {
            used.id() == attempted
                && used.role() == SourceUseRole::Attempted
                && matches!(
                    used.location(),
                    SourceLocation::Subresource { namespace, indices, field }
                        if namespace == "dataforgeRecordHeader"
                            && indices.as_slice() == [1]
                            && field == "fileName"
                )
        }));
        Ok(())
    })
}

fn corrupt_filename(bytes: &mut [u8], index: u32) -> TestResult {
    let forge = DataForge::open(bytes)?;
    let table = forge.record_table_span().ok_or("the Record table")?;
    let width = (table.end - table.start) / u64::try_from(forge.record_count())?;
    let start = usize::try_from(table.start + u64::from(index) * width)?;
    let offset = start + size_of::<u32>();
    bytes
        .get_mut(offset..offset + size_of::<u32>())
        .ok_or("the original filename offset")?
        .copy_from_slice(&u32::MAX.to_le_bytes());
    Ok(())
}

fn assert_alias_receipts(facts: &AncestorFacts, guid: Guid) -> TestResult {
    let [first, last] = facts.admitted_aliases(guid) else {
        return Err("two physical aliases".into());
    };
    assert_eq!((first.index, last.index), (0, 1), "definition order");
    assert_eq!(
        (first.marker.value(), last.marker.value()),
        (Some(&true), Some(&false)),
        "both exact marker observations remain available"
    );
    assert_ne!(
        first.marker.origin(),
        last.marker.origin(),
        "each physical alias owns distinct original observations"
    );
    Ok(())
}

fn assert_incomplete_origin(
    objects: &Objects<'_>,
    guid: Guid,
    sources: &SourceUses,
) -> Result<(), CatalogError> {
    let selected = objects.location(Some(guid));
    assert_eq!(
        selected,
        Ok(CatalogGuid(guid)),
        "the last readable non-marker supplies the compatibility value"
    );
    let origin = objects.location_origin(Some(guid), selected, sources)?;
    assert!(
        !origin.origin().source_set_complete(),
        "duplicate or failed membership stays incomplete for an observed value"
    );
    Ok(())
}

#[test]
fn filter_exclusion_is_distinct_from_explicit_none_and_cancellation() -> TestResult {
    test_support::with_projection(&fixture(), |forge, projection| {
        let repeated = guid(forge, 0)?;
        let filtered = guid(forge, 2)?;
        let input = projection.original();
        let placements = Placements::default();
        let filter = Filter::new("/last");
        let facts = AncestorFacts::read(
            forge,
            input,
            [repeated, filtered],
            &placements,
            Some(&filter),
            None,
        )?;
        assert_eq!(
            facts.owner(Some(repeated)),
            AncestorOwner::Unique(1),
            "one physical selection"
        );
        assert_eq!(
            facts.owner(Some(filtered)),
            AncestorOwner::NotAdmitted,
            "complete exclusion"
        );
        assert_eq!(
            facts.owner(None),
            AncestorOwner::NoAncestor,
            "explicit no ancestor"
        );
        assert!(
            facts.coverage.unreadable_bodies.is_empty(),
            "the filtered body is never read"
        );
        assert_absence_states(&facts, filtered, &input.origins().uses)?;

        let cancel = CancelToken::new();
        cancel.cancel();
        assert!(
            matches!(
                AncestorFacts::read(forge, input, [repeated], &placements, None, Some(&cancel)),
                Err(CatalogError::Cancelled)
            ),
            "cancellation cannot certify absence"
        );
        Ok(())
    })
}

fn assert_absence_states(
    facts: &AncestorFacts,
    filtered: Guid,
    sources: &SourceUses,
) -> Result<(), CatalogError> {
    let objects = facts.objects();
    let absent = objects.location(None);
    let missing = objects.location(Some(filtered));
    assert_eq!(
        absent,
        Err(NoObject::NoneAbove),
        "explicit null logical key"
    );
    assert_eq!(
        missing,
        Err(NoObject::NotInCatalog),
        "filtered demanded ancestor"
    );
    let missing = objects.location_origin(Some(filtered), missing, sources)?;
    assert_eq!(
        missing.state(),
        SourceValueState::Absent,
        "canonical value of exclusion"
    );
    assert!(
        !missing.origin().source_set_complete(),
        "alias exclusion has no published negative origin"
    );

    Ok(())
}

fn guid(forge: &DataForge<'_>, at: u32) -> Result<Guid, CatalogError> {
    forge
        .record(at)
        .map(|record| record.guid())
        .ok_or_else(|| ExportAccountingError::ConflictingSourceDatum.into())
}

fn fixture() -> Vec<u8> {
    let record = |guid, name, description| {
        TestRecord::new(
            guid,
            RECORD_TYPE,
            format!("{RECORD_TYPE}.{name}"),
            format!("libs/foundry/records/starmap/pu/system/sample/{name}.xml"),
        )
        .value("description", description)
    };
    let text = |value: &str| TestValue::Text(value.to_owned());
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(RECORD_TYPE).field("description", TestType::Locale))
        .record(record(1, "First", text("@MiningClaims_Desc_001")))
        .record(record(1, "Last", text("plain")))
        .record(record(2, "Broken", text("plain")).with_instance_index(u16::MAX))
        .record(record(2, "Readable", text("plain")))
        .record(record(
            3,
            "Unreadable",
            TestValue::Raw(u32::MAX.to_le_bytes().to_vec()),
        ))
        .record(record(4, "Undemanded", text("plain")).with_instance_index(u16::MAX))
        .build()
}

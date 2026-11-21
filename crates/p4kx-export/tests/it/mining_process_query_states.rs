// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::mining::{
    with_mining_global_params, with_mining_global_params_from, with_refining_processes,
    with_refining_processes_from,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, DefinitionStatus,
    ResourceNumber,
};
use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, Language, SourceValueState};
use p4kx_testkit::{ArchiveBuilder, TestEntry};

use super::fixture;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn wide_numbers_nonfinite_bits_signed_zero_and_dangling_reference_remain_exact() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(false), DcbOrigin::Plain)?;
    with_mining_global_params(
        &archive,
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| -> TestResult {
            let row = &snapshot.rows()[0];
            exact_numbers(row)?;
            let reference = row
                .waste_resource_type()
                .value()
                .ok_or("original waste reference")?;
            assert!(
                matches!(reference.status(), DefinitionStatus::Dangling),
                "missing targets remain dangling"
            );
            assert_eq!(
                reference.guid().value().map(ToString::to_string),
                Some(common::guid(fixture::MISSING)),
                "the original missing GUID survives"
            );
            assert!(
                snapshot.coverage().incomplete_fields.is_empty(),
                "legal original nonfinite values alone do not invalidate the typed source projection"
            );
            Ok(())
        },
    )??;
    Ok(())
}

fn exact_numbers(
    row: &p4kx_export::catalog_queries::mining::MiningGlobalConfiguration<'_>,
) -> TestResult {
    let Some(ResourceNumber::UInt64 { value }) = row.absorbable_volume_threshold().value() else {
        return Err("original UInt64".into());
    };
    assert_eq!(
        value.get(),
        u64::MAX,
        "unsigned digits survive without binary64 conversion"
    );
    let Some(ResourceNumber::Int64 { value }) = row.child_rock_inactivity_lifetime().value() else {
        return Err("original Int64".into());
    };
    assert_eq!(value.get(), fixture::WIDE, "wide signed digits survive");
    let Some(ResourceNumber::Single(value)) = row.child_rock_invulnerability_time().value() else {
        return Err("original Single NaN".into());
    };
    assert_eq!(
        value.raw_bits(),
        fixture::NAN_BITS,
        "NaN sign and payload survive"
    );
    assert!(
        value.finite().is_none(),
        "nonfinite is retained with absent finite convenience"
    );
    let Some(ResourceNumber::Double(value)) = row.child_rock_life_timer().value() else {
        return Err("original Double".into());
    };
    assert_eq!(
        value.raw_bits(),
        1.0e300_f64.to_bits(),
        "Double never narrows to Single"
    );
    let Some(ResourceNumber::Single(value)) = row.child_rock_zero_g_damping().value() else {
        return Err("original signed zero".into());
    };
    assert_eq!(
        value.raw_bits(),
        (-0.0_f32).to_bits(),
        "Single signed zero survives"
    );
    Ok(())
}

#[test]
fn unsupported_typed_number_still_has_complete_original_arena_and_distinct_field_coverage()
-> TestResult {
    let (_folder, archive) = common::open(&fixture::database(true), DcbOrigin::Plain)?;
    with_mining_global_params(
        &archive,
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| {
            let row = &snapshot.rows()[0];
            assert_ne!(
                row.absorbable_volume_threshold().state(),
                SourceValueState::Present,
                "a Bool is never silently converted to a numeric scalar"
            );
            assert!(
                row.fields()
                    .roots()
                    .iter()
                    .any(|field| field.name().value() == Some(&"absorbableVolumeThreshold")),
                "the actual original declaration remains accessible"
            );
            assert_eq!(
                snapshot.coverage().incomplete_fields,
                [0],
                "the failed projection keeps its physical owner"
            );
            assert!(
                snapshot.coverage().unreadable_bodies.is_empty(),
                "a retained source row is distinct from a failed body"
            );
        },
    )?;
    Ok(())
}

#[test]
fn global_and_empty_refining_queries_open_no_locked_locale_or_world_payload() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("source.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            "Data/Game2.dcb",
            fixture::database(false).build(),
        ))
        .entry(
            TestEntry::stored(
                Language::English.global_ini_path(),
                b"process_method=unused
",
            )
            .encrypted(),
        )
        .entry(
            TestEntry::stored(
                "Data/ObjectContainers/PU/system/synthetic/syntheticsystem.socpak",
                b"unopened-world",
            )
            .encrypted(),
        )
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let before = archive.key_demand().snapshot().unread();
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    parsed.with_mining_global_params(CatalogRecordSelection::All, None, |snapshot| {
        assert_eq!(
            snapshot.rows().len(),
            2,
            "two selected globals need no locale"
        );
    })?;
    parsed.with_refining_processes(CatalogRecordSelection::RecordIndex(4), None, |snapshot| {
        assert!(
            snapshot.rows().is_empty(),
            "foreign family has no refining owner"
        );
    })?;
    assert_eq!(
        archive.key_demand().snapshot().unread(),
        before,
        "unrelated encrypted payloads stay unopened"
    );
    Ok(())
}

#[test]
fn source_origin_mismatch_and_cancellation_never_enter_consumers() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(false), DcbOrigin::Plain)?;
    let (_other_folder, other) = common::open(&fixture::database(false), DcbOrigin::Plain)?;
    let foreign = DataForgeEntry::read(&other)?;
    let called = std::cell::Cell::new(false);
    assert!(
        matches!(
            with_mining_global_params_from(
                &archive,
                &foreign,
                CatalogRecordSelection::All,
                None,
                |_| called.set(true)
            ),
            Err(p4kx_export::CatalogError::SourceOriginMismatch)
        ),
        "global Entry binding is checked"
    );
    assert!(
        matches!(
            with_refining_processes_from(
                &archive,
                &foreign,
                CatalogRecordSelection::All,
                CatalogLanguageSelection::Selected(&[]),
                None,
                |_| called.set(true)
            ),
            Err(p4kx_export::CatalogError::SourceOriginMismatch)
        ),
        "refining Entry binding is checked"
    );
    let token = CancelToken::new();
    token.cancel();
    assert!(
        matches!(
            with_mining_global_params(&archive, CatalogRecordSelection::All, Some(&token), |_| {
                called.set(true);
            }),
            Err(p4kx_export::CatalogError::Cancelled)
        ),
        "global cancellation is explicit"
    );
    assert!(
        matches!(
            with_refining_processes(
                &archive,
                CatalogRecordSelection::All,
                CatalogLanguageSelection::Selected(&[]),
                Some(&token),
                |_| called.set(true)
            ),
            Err(p4kx_export::CatalogError::Cancelled)
        ),
        "refining cancellation is explicit"
    );
    assert!(!called.get(), "refused inputs never reach the callback");
    Ok(())
}

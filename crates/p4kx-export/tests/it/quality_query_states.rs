// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::quality::{
    CatalogRecordSelection, DefinitionStatus, QualityPayload, QualitySource, ResourceNumber,
    with_distributions, with_distributions_from, with_quantizations, with_quantizations_from,
};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};
use p4kx_extraction::{CancelToken, DataForgeEntry, SourceValueState};

use super::fixture;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn original_overrides_preserve_null_slots_repetitions_and_integer_values() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(), DcbOrigin::Plain)?;
    with_distributions(
        &archive,
        CatalogRecordSelection::Guid(common::guid(fixture::OVERRIDE).parse()?),
        None,
        |snapshot| -> TestResult {
            let [row] = snapshot.rows() else {
                return Err("one original override owner".into());
            };
            assert_eq!(
                row.quality_distribution().state(),
                SourceValueState::Absent,
                "the other root declaration is genuinely absent"
            );
            let overrides = inline(row.location_override().value().ok_or("override source")?)?;
            let slots = overrides
                .location_override_list()
                .value()
                .ok_or("original override array")?;
            assert_eq!(
                slots.len(),
                3,
                "nullable and repeated positions cannot be compacted"
            );
            assert_eq!(
                slots[1].state(),
                SourceValueState::Null,
                "the middle source slot remains null"
            );
            for index in [0, 2] {
                original_override_slot(slots[index].value().ok_or("original override entry")?)?;
            }
            Ok(())
        },
    )??;
    Ok(())
}

fn original_override_slot(
    entry: &p4kx_export::catalog_queries::quality::QualityOverrideSource<'_>,
) -> TestResult {
    let Some(p4kx_export::catalog_queries::quality::QualityOverridePayload::Entry(fields)) =
        entry.payload().value()
    else {
        return Err("recognized original entry".into());
    };
    assert_eq!(
        fields.location().state(),
        SourceValueState::Null,
        "an empty authored location is not a dangling target"
    );
    let curve = inline(
        fields
            .quality_distribution()
            .value()
            .ok_or("override distribution")?,
    )?;
    assert!(
        matches!(
            curve.min().value(),
            Some(ResourceNumber::Int32 { value: 17 })
        ),
        "each repeated slot retains its original exact Int32"
    );
    Ok(())
}

#[test]
fn direct_definitions_keep_null_roots_and_dangling_reference_headers() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(), DcbOrigin::Plain)?;
    with_distributions(
        &archive,
        CatalogRecordSelection::Guid(common::guid(fixture::NULL_DISTRIBUTION).parse()?),
        None,
        |snapshot| {
            assert_eq!(
                snapshot.rows()[0].quality_distribution().state(),
                SourceValueState::Null,
                "null distribution root remains explicit"
            );
        },
    )?;
    with_quantizations(
        &archive,
        CatalogRecordSelection::Guid(common::guid(fixture::NULL_QUANTIZATION).parse()?),
        None,
        |snapshot| {
            assert_eq!(
                snapshot.rows()[0].quality_quantization().state(),
                SourceValueState::Null,
                "null quantization root remains explicit"
            );
        },
    )?;
    with_distributions(
        &archive,
        CatalogRecordSelection::Guid(common::guid(fixture::DANGLING_DISTRIBUTION).parse()?),
        None,
        |snapshot| -> TestResult {
            dangling(
                snapshot.rows()[0]
                    .quality_distribution()
                    .value()
                    .ok_or("original distribution reference")?,
            )
        },
    )??;
    with_quantizations(
        &archive,
        CatalogRecordSelection::RecordIndex(4),
        None,
        |snapshot| -> TestResult {
            dangling(
                snapshot.rows()[0]
                    .quality_quantization()
                    .value()
                    .ok_or("original quantization reference")?,
            )
        },
    )??;
    Ok(())
}

fn dangling<T>(source: &QualitySource<'_, T>) -> TestResult {
    let Some(QualityPayload::RecordRef(original)) = source.payload().value() else {
        return Err("original Record reference wrapper".into());
    };
    let target = original
        .reference()
        .value()
        .ok_or("original target facts")?;
    assert!(
        matches!(target.status(), DefinitionStatus::Dangling),
        "unavailable target bodies are not synthesized"
    );
    assert_eq!(
        target.guid().value().map(ToString::to_string),
        Some(common::guid(fixture::MISSING)),
        "the actual dangling GUID survives"
    );
    assert_eq!(
        target.target_record_index().state(),
        SourceValueState::Absent,
        "a missing target has no fabricated physical identity"
    );
    Ok(())
}

#[test]
fn original_wide_integers_and_ieee_values_keep_separate_field_coverage() -> TestResult {
    let (_folder, archive) = common::open(&fixture::wide_distribution(), DcbOrigin::Plain)?;
    with_distributions(
        &archive,
        CatalogRecordSelection::All,
        None,
        |snapshot| -> TestResult {
            let [row] = snapshot.rows() else {
                return Err("one wide original owner".into());
            };
            let curve = inline(row.quality_distribution().value().ok_or("original curve")?)?;
            let Some(ResourceNumber::Int64 { value }) = curve.min().value() else {
                return Err("original signed Int64".into());
            };
            assert_eq!(
                value.get(),
                fixture::WIDE_MIN,
                "integer digits beyond binary64 precision are preserved"
            );
            let Some(ResourceNumber::Double(mean)) = curve.mean().value() else {
                return Err("original Double mean".into());
            };
            assert_eq!(
                mean.raw_bits(),
                1.0e300_f64.to_bits(),
                "Double precision is never narrowed to Single"
            );
            let Some(ResourceNumber::Double(deviation)) = curve.stddev().value() else {
                return Err("original Double deviation".into());
            };
            assert_eq!(
                deviation.raw_bits(),
                (-0.0_f64).to_bits(),
                "signed zero remains an exact source value"
            );
            assert_eq!(
                snapshot.coverage().incomplete_fields,
                [0],
                "the declared Int32 contract violation stays distinct from retained raw data"
            );
            assert!(
                snapshot.coverage().unreadable_bodies.is_empty(),
                "field coverage never fabricates a failed owner read"
            );
            Ok(())
        },
    )??;
    Ok(())
}

fn inline<'read, T>(source: &'read QualitySource<'_, T>) -> TestResult<&'read T> {
    let Some(QualityPayload::Inline(original)) = source.payload().value() else {
        return Err("original inline quality payload".into());
    };
    original
        .value()
        .value()
        .ok_or_else(|| "present inline source data".into())
}

#[test]
fn quality_queries_open_neither_locked_locales_nor_world_entries() -> TestResult {
    let (_folder, archive) = fixture::open_without_key()?;
    let before = archive.key_demand().snapshot().unread();
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    parsed.with_quality_distributions(CatalogRecordSelection::All, None, |snapshot| {
        assert_eq!(
            snapshot.rows().len(),
            5,
            "the definition family needs no localization payload"
        );
    })?;
    parsed.with_quality_quantizations(CatalogRecordSelection::All, None, |snapshot| {
        assert_eq!(
            snapshot.rows().len(),
            3,
            "raw definitions need no inverse or world payload"
        );
    })?;
    assert_eq!(
        archive.key_demand().snapshot().unread(),
        before,
        "unrelated encrypted entries remain unopened without any Archive key"
    );
    Ok(())
}

#[test]
fn cancelled_and_foreign_entry_queries_never_enter_consumers() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(), DcbOrigin::Plain)?;
    let (_other_folder, other) = common::open(&fixture::database(), DcbOrigin::Plain)?;
    let foreign = DataForgeEntry::read(&other)?;
    let called = std::cell::Cell::new(false);
    let result = with_distributions_from(
        &archive,
        &foreign,
        CatalogRecordSelection::All,
        None,
        |_| called.set(true),
    );
    assert!(
        matches!(result, Err(p4kx_export::CatalogError::SourceOriginMismatch)),
        "decoded Entries cannot cross their original Archive binding"
    );
    let result = with_quantizations_from(
        &archive,
        &foreign,
        CatalogRecordSelection::All,
        None,
        |_| called.set(true),
    );
    assert!(
        matches!(result, Err(p4kx_export::CatalogError::SourceOriginMismatch)),
        "quantization queries preserve the same binding check"
    );
    let cancel = CancelToken::new();
    cancel.cancel();
    let result = with_distributions(&archive, CatalogRecordSelection::All, Some(&cancel), |_| {
        called.set(true);
    });
    assert!(
        matches!(result, Err(p4kx_export::CatalogError::Cancelled)),
        "distribution cancellation is explicit"
    );
    let result = with_quantizations(&archive, CatalogRecordSelection::All, Some(&cancel), |_| {
        called.set(true);
    });
    assert!(
        matches!(result, Err(p4kx_export::CatalogError::Cancelled)),
        "quantization cancellation is explicit"
    );
    assert!(
        !called.get(),
        "source refusal and cancellation precede the consumer callback"
    );
    Ok(())
}

#[test]
fn original_nonfinite_double_and_single_signed_zero_keep_their_bits() -> TestResult {
    let (_folder, archive) = common::open(&fixture::database(), DcbOrigin::Plain)?;
    with_distributions(
        &archive,
        CatalogRecordSelection::RecordIndex(0),
        None,
        |snapshot| -> TestResult {
            let [row] = snapshot.rows() else {
                return Err("one physical original curve".into());
            };
            let curve = inline(row.quality_distribution().value().ok_or("original curve")?)?;
            let Some(ResourceNumber::Double(mean)) = curve.mean().value() else {
                return Err("original Double mean".into());
            };
            assert_eq!(
                mean.raw_bits(),
                fixture::NAN_BITS,
                "NaN sign and payload remain exact"
            );
            assert!(
                mean.finite().is_none(),
                "nonfinite convenience values remain absent"
            );
            let Some(ResourceNumber::Single(deviation)) = curve.stddev().value() else {
                return Err("original Single deviation".into());
            };
            assert_eq!(
                deviation.raw_bits(),
                (-0.0_f32).to_bits(),
                "Single signed zero is not promoted or normalized"
            );
            Ok(())
        },
    )??;
    Ok(())
}

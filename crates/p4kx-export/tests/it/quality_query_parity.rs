// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
use p4kx_export::catalog_queries::quality::{
    DistributionSnapshot, QualityDefinitionSnapshot, QuantizationSnapshot,
};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_export::{CatalogPublicationMode, build_catalog};
use p4kx_extraction::DataForgeEntry;
use serde::Serialize;
use serde_json::Value;

use super::fixture;
use crate::canonical_header::wire::Wire;
use crate::catalog_resources::resource_consumer_walk::{
    visit_curve, visit_header, visit_overrides, visit_quality, visit_quantization,
};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::quality_v3_support as support;

#[test]
fn selected_quality_definitions_match_canonical_wire_through_all_typed_getters() -> TestResult {
    for (origin, marker) in [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ] {
        let (_folder, archive) = common::open(&fixture::database(), origin)?;
        let mut options = common::options();
        options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
        let built = build_catalog(&archive, options.with_full_provenance())?;
        let catalog: Value = serde_json::from_slice(built.json())?;
        let entry = DataForgeEntry::read(&archive)?;
        let session = full_proof::from_entry(
            &archive,
            &entry,
            CatalogLanguageSelection::AllAvailable,
            None,
        )?;
        let parsed = session.parsed(None)?;
        parsed.with_quality_distributions(CatalogRecordSelection::All, None, |snapshot| {
            distributions(&snapshot, &catalog, marker)
        })??;
        parsed.with_quality_quantizations(CatalogRecordSelection::All, None, |snapshot| {
            quantizations(&snapshot, &catalog, marker)
        })??;
    }
    Ok(())
}

fn distributions(
    snapshot: &DistributionSnapshot<'_>,
    catalog: &Value,
    marker: Option<bool>,
) -> TestResult {
    let expected = support::rows(catalog, support::DISTRIBUTIONS)?;
    assert_eq!(
        snapshot.rows().len(),
        expected.len(),
        "all physical distribution and override rows survive"
    );
    assert_eq!(
        canonical(snapshot.rows())?,
        canonical(expected)?,
        "selected canonical source rows have identical wire bytes after parsing"
    );
    for (row, expected) in snapshot.rows().iter().zip(expected) {
        let wire = Wire::new(expected, snapshot.source_inventory());
        visit_header(row.source(), wire)?;
        wire.field("qualityDistribution")?
            .cell(row.quality_distribution(), |source, wire| {
                visit_quality(source, wire, visit_curve)
            })?;
        wire.field("locationOverride")?
            .cell(row.location_override(), |source, wire| {
                visit_quality(source, wire, visit_overrides)
            })?;
        assert_eq!(
            row.source().guid().origin().contains_encrypted_source(),
            marker,
            "physical source encryption is independent of successful decoding"
        );
    }
    verify_snapshot(snapshot, catalog, support::DISTRIBUTIONS)
}

fn quantizations(
    snapshot: &QuantizationSnapshot<'_>,
    catalog: &Value,
    marker: Option<bool>,
) -> TestResult {
    let expected = support::rows(catalog, support::QUANTIZATIONS)?;
    assert_eq!(
        snapshot.rows().len(),
        expected.len(),
        "every physical quantization row survives independently of inverse eligibility"
    );
    assert_eq!(
        canonical(snapshot.rows())?,
        canonical(expected)?,
        "no raw quantization payload is reduced for the SDK"
    );
    for (row, expected) in snapshot.rows().iter().zip(expected) {
        let wire = Wire::new(expected, snapshot.source_inventory());
        visit_header(row.source(), wire)?;
        wire.field("qualityQuantization")?
            .cell(row.quality_quantization(), |source, wire| {
                visit_quality(source, wire, visit_quantization)
            })?;
        assert_eq!(
            row.source().guid().origin().contains_encrypted_source(),
            marker,
            "quantization identity keeps its actual physical input marker"
        );
    }
    verify_snapshot(snapshot, catalog, support::QUANTIZATIONS)
}

fn verify_snapshot<Row>(
    snapshot: &QualityDefinitionSnapshot<Row>,
    catalog: &Value,
    dataset: &str,
) -> TestResult {
    assert!(
        snapshot.build().is_none(),
        "the synthetic Archive declares no installed Build"
    );
    assert!(
        snapshot.coverage().unreadable_bodies.is_empty(),
        "all selected synthetic owner bodies are readable"
    );
    assert!(
        snapshot.coverage().unreadable_headers.is_empty(),
        "all selected original headers are readable"
    );
    assert_eq!(
        canonical(&snapshot.availability())?,
        *support::at(catalog, &format!("/{dataset}/availability"))?,
        "source-family availability is the same canonical declaration fact"
    );
    Ok(())
}

fn canonical<T: Serialize + ?Sized>(value: &T) -> TestResult<Value> {
    Ok(serde_json::from_slice(&serde_json::to_vec(value)?)?)
}

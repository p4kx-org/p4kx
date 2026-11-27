// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Compression};
use p4kx_extraction::Filter;
use serde_json::{Value, json};

use super::{common, entities, fixture, proof};

const TABLE: &str = "/configurations/itemBounds";
const ROWS: &str = "/configurations/itemBounds/rows";

#[test]
fn staged_bounds_replay_in_physical_order_with_original_provenance() -> common::TestResult {
    let database = entities::database();
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    for (origin, encrypted) in [
        (common::DcbOrigin::Plain, false),
        (common::DcbOrigin::Encrypted, true),
    ] {
        let (_directory, archive) = common::open(&database, origin)?;
        let built = p4kx_export::build_catalog(&archive, options.with_full_provenance())?;
        let (summary, compressed) = p4kx_export::write_catalog(
            &archive,
            options.with_full_provenance(),
            Compression::ZSTD,
            || Ok(Vec::new()),
        )?;
        assert!(
            summary.staged_row_bytes() > 0,
            "the producer accounts for staged bytes"
        );
        assert_eq!(
            zstd::stream::decode_all(compressed.as_slice())?,
            built.json(),
            "measured and replayed rows match the raw writer exactly"
        );
        let catalog: Value = serde_json::from_slice(built.json())?;
        let rows = proof::rows(&catalog, ROWS)?;
        let expected = [
            (entities::WEARABLE, 0),
            (entities::WEARABLE, 1),
            (entities::VEHICLE, 0),
            (entities::VEHICLE, 1),
        ];
        assert_eq!(
            rows.len(),
            expected.len(),
            "all physical component occurrences"
        );
        for (row, (guid, ordinal)) in rows.iter().zip(expected) {
            proof::present(
                row,
                "ownerGuid",
                &json!(common::guid(guid)),
                Some(encrypted),
            )?;
            proof::present(row, "componentOrdinal", &json!(ordinal), Some(encrypted))?;
        }
        let source = proof::at(&rows[0], "/attachDef/value")?;
        proof::cell(source, "inventoryOccupancyDimensionsUIOverride", "null")?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn empty_staged_bounds_preserve_declared_source_availability() -> common::TestResult {
    let filter = Filter::parse("fixture record outside this catalog")?;
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    for (database, availability) in [
        (entities::database(), "available"),
        (fixture::database(), "absent"),
    ] {
        let (_directory, archive) = common::open(&database, common::DcbOrigin::Plain)?;
        let built = p4kx_export::build_catalog(&archive, options.with_full_provenance())?;
        let catalog: Value = serde_json::from_slice(built.json())?;
        assert_eq!(
            *proof::at(&catalog, TABLE)?,
            json!({"availability": availability, "rows": []}),
            "empty filtered selection reflects physical source declarations"
        );
    }
    Ok(())
}

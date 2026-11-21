// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::commodity_v3_fixture::{self as common, DcbOrigin};
use crate::gameplay_v3_source as source;
use crate::gameplay_v3_support::{self as support, TestResult};
use crate::gameplay_v3_values as values;
use p4kx_export::{self as export, CancelToken, CatalogError, CatalogOptions, Compression};
use p4kx_extraction::DataForge;
use serde_json::Value;
use std::cell::Cell;

#[test]
fn gameplay_v3_keeps_interpreted_read_crafting_and_plain_compressed_cancelled_publication_separate()
-> TestResult {
    let database = values::simple();
    source::declarations(&database)?;
    source::basic(&database)?;
    interpreted(&database)?;
    let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
    let options = common::options();
    let built = export::build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    support::row(&catalog, 0)?;
    let (summary, compressed) =
        export::write_catalog(&archive, &options, Compression::ZSTD, || Ok(Vec::new()))?;
    let decoded = zstd::stream::decode_all(compressed.as_slice())?;
    assert_eq!(
        serde_json::from_slice::<Value>(&decoded)?,
        catalog,
        "plain and compressed paths emit the same canonical facts"
    );
    assert_eq!(
        summary
            .kind(export::Kind::GameplayProperties)
            .map(|kind| kind.count),
        Some(1),
        "compression does not introduce another accounting unit"
    );
    cancelled(&archive, options);
    Ok(())
}

fn interpreted(database: &p4kx_testkit::DataForgeBuilder) -> TestResult {
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let interpreted = export::read_crafting(&forge, None, None)?;
    let property = interpreted
        .properties
        .first()
        .ok_or("existing interpreted property")?;
    assert_eq!(
        interpreted.properties.len(),
        1,
        "the independent public SDK still reads one property"
    );
    assert_eq!(
        property.guid.to_string(),
        common::guid(values::FIRST),
        "existing interpreted identity"
    );
    assert_eq!(
        property.class_name, "Primary",
        "existing interpreted class name"
    );
    assert_eq!(
        property.name_key,
        Some("@resource_name"),
        "existing interpreted Locale key behavior"
    );
    Ok(())
}

fn cancelled(archive: &p4kx_extraction::Archive, options: CatalogOptions<'_>) {
    let token = CancelToken::new();
    token.cancel();
    let cancelled = CatalogOptions {
        cancel: Some(&token),
        ..options
    };
    let creates = Cell::new(0);
    let result = export::write_catalog(archive, &cancelled, Compression::None, || {
        creates.set(creates.get() + 1);
        Ok(Vec::<u8>::new())
    });
    assert!(
        matches!(result, Err(CatalogError::Cancelled)),
        "cancellation remains a fatal typed outcome"
    );
    assert_eq!(
        creates.get(),
        0,
        "a cancelled C2 build never creates or replaces its output sink"
    );
}

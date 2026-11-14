// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::{Kind, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestStruct};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as fixture, TestResult};

#[test]
fn canonical_tables_keep_guid_identity_shared_resource_links_and_resource_only_backfill()
-> TestResult {
    let (built, catalog) = fixture::build(&fixture::database())?;
    assert_eq!(
        catalog["schemaVersion"], 3,
        "the producer publishes canonical v3"
    );
    assert!(
        catalog.get("commodities").is_none(),
        "no parallel flat legacy Commodity view"
    );
    for path in [
        "/definitions/commodities",
        "/projections/commodityPresentation",
    ] {
        assert_eq!(
            catalog.pointer(path).ok_or("canonical table")?["availability"],
            "available"
        );
        assert_eq!(
            catalog
                .pointer(&format!("{path}/rows"))
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(4)
        );
    }
    let ids: BTreeSet<_> = catalog["definitions"]["commodities"]["rows"]
        .as_array()
        .ok_or("definitions")?
        .iter()
        .filter_map(|row| row["guid"]["value"].as_str())
        .collect();
    assert_eq!(
        ids.len(),
        4,
        "distinct physical Commodity identities are not collapsed by resource link"
    );
    assert_entity_links(&catalog)?;
    assert_resource_backfill(&catalog)?;
    assert_eq!(
        built
            .summary()
            .kind(Kind::Commodities)
            .map(|kind| (kind.count, kind.complete)),
        Some((4, true))
    );
    Ok(())
}

#[test]
fn availability_distinguishes_absent_families_from_healthy_empty_tables() -> TestResult {
    for (database, availability) in [
        (DataForgeBuilder::new().version(8), "absent"),
        (
            DataForgeBuilder::new()
                .version(8)
                .structure(TestStruct::new("EntityClassDefinition")),
            "available",
        ),
    ] {
        let (built, catalog) = fixture::build(&database)?;
        for path in [
            "/definitions/commodities",
            "/projections/commodityPresentation",
        ] {
            let table = catalog.pointer(path).ok_or("canonical table")?;
            assert_eq!(table["availability"], availability);
            assert_eq!(table["rows"].as_array().map(Vec::len), Some(0));
        }
        assert_eq!(
            built
                .summary()
                .kind(Kind::Commodities)
                .map(|kind| (kind.count, kind.complete)),
            Some((0, true))
        );
    }
    Ok(())
}

#[test]
fn repeated_production_builds_serialize_the_same_prepared_contract() -> TestResult {
    let (_folder, archive) = fixture::open(&fixture::database(), fixture::DcbOrigin::Plain)?;
    let first = build_catalog(&archive, &fixture::options())?;
    let second = build_catalog(&archive, &fixture::options())?;
    assert_eq!(
        first.json(),
        second.json(),
        "source-use registration order does not change the product wire"
    );
    Ok(())
}

fn assert_entity_links(catalog: &Value) -> TestResult {
    let resource = fixture::guid(fixture::RESOURCE);
    for guid in [fixture::ALPHA, fixture::BETA] {
        let definition = fixture::definition(catalog, guid)?;
        assert_eq!(
            fixture::at(definition, "/guid/value")?,
            &json!(fixture::guid(guid)),
            "each Entity retains its own GUID"
        );
        assert_eq!(
            fixture::at(definition, "/selection/value")?,
            &json!("entity"),
            "authored Entities retain their selection kind"
        );
        assert_eq!(
            fixture::at(fixture::presentation(catalog, guid)?, "/subtypeGuid/value")?,
            &json!(resource),
            "shared resources do not merge Entity identities"
        );
    }
    let unlinked = fixture::presentation(catalog, fixture::UNLINKED)?;
    assert_eq!(
        fixture::at(unlinked, "/subtypeGuid/state")?,
        &json!("present"),
        "the presentation explicitly chose no link"
    );
    assert!(
        fixture::at(unlinked, "/subtypeGuid/value")?.is_null(),
        "the absent relationship has an explicit null payload"
    );
    Ok(())
}

fn assert_resource_backfill(catalog: &Value) -> TestResult {
    let fallback = fixture::definition(catalog, fixture::RESOURCE_FALLBACK)?;
    assert_eq!(
        fixture::at(fallback, "/selection/value")?,
        &json!("resourceFallback"),
        "retained costs can select a resource-only row"
    );
    assert_eq!(
        fixture::at(fallback, "/sourceType/value")?,
        &json!("ResourceType"),
        "backfill retains its physical source type"
    );
    assert_eq!(
        fixture::at(fallback, "/entity/state")?,
        &json!("absent"),
        "backfill has no invented Entity source"
    );
    assert_eq!(
        fixture::at(
            fixture::presentation(catalog, fixture::RESOURCE_FALLBACK)?,
            "/subtypeGuid/value"
        )?,
        fixture::at(fallback, "/guid/value")?,
        "resource-only presentation links to its own definition"
    );
    assert!(
        fixture::definition(catalog, fixture::RESOURCE_UNUSED).is_err(),
        "unused resources are not added as Commodities"
    );
    Ok(())
}

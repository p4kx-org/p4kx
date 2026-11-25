// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::sdk_full_proof as full_proof;
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
    let first = build_catalog(&archive, fixture::options().with_full_provenance())?;
    let second = build_catalog(&archive, fixture::options().with_full_provenance())?;
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

#[test]
fn sdk_commodities_match_the_selected_catalog_rows_and_their_original_sources() -> TestResult {
    use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};

    let (_folder, archive) = fixture::open(&fixture::database(), fixture::DcbOrigin::Encrypted)?;
    let built = build_catalog(&archive, fixture::options().with_full_provenance())?;
    let catalog = serde_json::from_slice(built.json())?;
    let session = full_proof::open(&archive, CatalogLanguageSelection::AllAvailable, None)?;
    let parsed = session.parsed(None)?;
    let alpha = fixture::guid(fixture::ALPHA).parse()?;
    let fallback = fixture::guid(fixture::RESOURCE_FALLBACK).parse()?;
    let represented = fixture::guid(fixture::RESOURCE).parse()?;
    let alpha_index = u32::try_from(fixture::ALPHA_INDEX)?;
    for (selection, wanted) in [
        (CatalogRecordSelection::All, None),
        (CatalogRecordSelection::Guid(alpha), Some(fixture::ALPHA)),
        (
            CatalogRecordSelection::Guid(fallback),
            Some(fixture::RESOURCE_FALLBACK),
        ),
        (
            CatalogRecordSelection::RecordIndex(alpha_index),
            Some(fixture::ALPHA),
        ),
        (
            CatalogRecordSelection::RecordIndex(1),
            Some(fixture::RESOURCE_FALLBACK),
        ),
    ] {
        parsed.with_commodities(selection, None, |snapshot| {
            assert_commodity_snapshot(&snapshot, &catalog, wanted)
        })??;
    }
    parsed.with_commodities(
        CatalogRecordSelection::Guid(represented),
        None,
        |snapshot| {
            assert!(
                snapshot.definitions.is_empty(),
                "a represented ResourceType is not a Commodity fallback"
            );
            assert_eq!(
                snapshot.presentations.len(),
                0,
                "a represented resource has no fallback presentation"
            );
        },
    )?;
    Ok(())
}

fn assert_commodity_snapshot(
    snapshot: &p4kx_export::catalog_queries::commodities::CommoditySnapshot<'_>,
    catalog: &Value,
    wanted: Option<u128>,
) -> TestResult {
    for (scope, failures) in [
        ("selected headers", &snapshot.coverage.unreadable_headers),
        ("selected bodies", &snapshot.coverage.unreadable_bodies),
        (
            "dependency headers",
            &snapshot.dependency_coverage.unreadable_headers,
        ),
        (
            "dependency bodies",
            &snapshot.dependency_coverage.unreadable_bodies,
        ),
    ] {
        assert_eq!(
            failures.as_slice(),
            &[] as &[u32],
            "the fixture reads all {scope}"
        );
    }
    assert_eq!(
        &json!(snapshot.availability),
        fixture::at(catalog, "/definitions/commodities/availability")?,
        "definition availability matches the catalog"
    );
    let definitions: Vec<Value> =
        serde_json::from_slice(&serde_json::to_vec(&snapshot.definitions)?)?;
    let presentations: Vec<Value> =
        serde_json::from_slice(&serde_json::to_vec(&snapshot.presentations)?)?;
    if let Some(wanted) = wanted {
        assert_eq!(
            definitions.as_slice(),
            std::slice::from_ref(fixture::definition(catalog, wanted)?),
            "selected definition matches its catalog row"
        );
        assert_eq!(
            presentations.as_slice(),
            std::slice::from_ref(fixture::presentation(catalog, wanted)?),
            "selected presentation matches its catalog row"
        );
    } else {
        assert_eq!(
            &json!(definitions),
            fixture::at(catalog, "/definitions/commodities/rows")?,
            "all definitions match the canonical catalog order and values"
        );
        assert_eq!(
            &json!(presentations),
            fixture::at(catalog, "/projections/commodityPresentation/rows")?,
            "all presentations match the canonical catalog order and values"
        );
    }
    assert_commodity_source_uses(
        definitions.iter().chain(&presentations),
        &snapshot.source_inventory,
        catalog,
    )
}

fn assert_commodity_source_uses<'row>(
    rows: impl Iterator<Item = &'row Value>,
    inventory: &p4kx_extraction::SourceInventory,
    catalog: &Value,
) -> TestResult {
    let mut needed = BTreeSet::new();
    for row in rows {
        collect_commodity_refs(row, &mut needed);
    }
    assert!(
        !needed.is_empty(),
        "the fixtures retain original source uses"
    );
    let sources: std::collections::BTreeMap<_, _> = inventory
        .iter_sources()
        .map(|source| (source.id().as_str(), source))
        .collect();
    let catalog_uses = crate::health_origin_assertions::rows(catalog, "/provenance/uses")?;
    let catalog_uses: std::collections::BTreeMap<_, _> = catalog_uses
        .iter()
        .filter_map(|row| Some((row.get("id")?.as_str()?, row)))
        .collect();
    for used in inventory.iter_uses() {
        let id = used.id().as_str();
        if needed.remove(id) {
            let catalog_use = catalog_uses.get(id).ok_or("Catalog source use")?;
            assert_eq!(
                serde_json::to_value(used)?,
                **catalog_use,
                "source use {id} is identical"
            );
            let source = used.source().as_str();
            assert_eq!(
                serde_json::to_value(sources.get(source).ok_or("SDK physical source")?)?,
                *crate::health_origin_assertions::source(catalog, catalog_use)?,
                "physical source {source} is identical"
            );
        }
    }
    assert!(
        needed.is_empty(),
        "every returned field resolves in the snapshot"
    );
    Ok(())
}

fn collect_commodity_refs<'row>(value: &'row Value, output: &mut BTreeSet<&'row str>) {
    match value {
        Value::Array(values) => {
            for value in values {
                collect_commodity_refs(value, output);
            }
        }
        Value::Object(fields) => {
            if let Some(ids) = fields.get("sourceUsesRefs").and_then(Value::as_array) {
                output.extend(ids.iter().filter_map(Value::as_str));
            }
            for child in fields.values() {
                collect_commodity_refs(child, output);
            }
        }
        _ => {}
    }
}

#[test]
fn commodity_sdk_guid_selection_keeps_physical_aliases_and_point_reads_stay_local() -> TestResult {
    use p4kx_export::catalog_queries::{
        CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection,
    };
    use p4kx_extraction::SourceLocation;
    use p4kx_testkit::TestRecord;

    let database = fixture::database().record(TestRecord::new(
        fixture::ALPHA,
        "EntityClassDefinition",
        "EntityClassDefinition.AlphaAlias",
        "libs/foundry/records/entities/commodities/alpha_alias.xml",
    ));
    let (_folder, archive) = fixture::open(&database, fixture::DcbOrigin::Plain)?;
    let session =
        CatalogQuerySession::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    let guid = fixture::guid(fixture::ALPHA).parse()?;
    let first = u32::try_from(fixture::ALPHA_INDEX)?;
    let positions =
        parsed.with_commodities(CatalogRecordSelection::Guid(guid), None, |snapshot| {
            snapshot
                .definitions
                .iter()
                .filter_map(|row| row.source().record_index().value().copied())
                .collect::<Vec<_>>()
        })?;
    let [a, alias] = positions.as_slice() else {
        return Err("expected both physical GUID aliases".into());
    };
    assert_eq!(*a, first, "the original physical row comes first");
    assert!(*alias > first, "the alias retains its later physical index");

    parsed.with_commodities(CatalogRecordSelection::RecordIndex(*alias), None, |snapshot| {
        assert_eq!(snapshot.definitions.len(), 1, "only the selected definition is returned");
        assert_eq!(snapshot.presentations.len(), 1, "only the selected presentation is returned");
        assert!(snapshot.source_inventory.iter_uses().all(|used| {
            !matches!(
                used.location(),
                SourceLocation::DataForge { record_index, .. } if record_index != alias
            )
        }), "the unlinked alias does not open other Commodity, Resource, group or quantization bodies");
    })?;
    Ok(())
}

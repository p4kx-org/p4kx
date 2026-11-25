// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::resource_network::{
    ResourceComponentSnapshot, ResourceConnectionFields, ResourceConnectionSnapshot,
    ResourceControlParameter,
};
use p4kx_export::catalog_queries::{CatalogLanguageSelection, CatalogRecordSelection};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::DataForgeEntry;
use serde_json::{Value, json};

use super::fixture;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;
use crate::sdk_full_proof as full_proof;

#[test]
fn resource_queries_keep_the_producer_values_and_exact_physical_origins() -> TestResult {
    let database = fixture::database();
    for origin in [DcbOrigin::Plain, DcbOrigin::Encrypted] {
        let (_directory, archive) = common::open(&database, origin)?;
        let catalog: Value = serde_json::from_slice(
            build_catalog(&archive, common::options().with_full_provenance())?.json(),
        )?;
        let entry = DataForgeEntry::read(&archive)?;
        let session = full_proof::from_entry(
            &archive,
            &entry,
            CatalogLanguageSelection::AllAvailable,
            None,
        )?;
        let selection = CatalogRecordSelection::RecordIndex(0);
        session.with_resource_components(selection, None, |snapshot| {
            check_components(&snapshot, &catalog)
        })??;
        session.with_resource_connections(selection, None, |snapshot| {
            check_connections(&snapshot, &catalog)
        })??;
    }
    Ok(())
}

fn check_components(snapshot: &ResourceComponentSnapshot<'_>, catalog: &Value) -> TestResult {
    assert!(
        snapshot.coverage().incomplete_fields.is_empty(),
        "the selected resource components are complete"
    );
    assert!(
        snapshot.requested_languages().is_empty(),
        "the resource query reads no locale payloads"
    );
    let [row] = snapshot.rows() else {
        return Err("the selected resource owner".into());
    };
    let Some(parameters) = row.controls.control_parameters.value() else {
        return Err("original control parameter array".into());
    };
    let [_, empty, _, _, future] = parameters.as_slice() else {
        return Err("all original control positions".into());
    };
    assert!(
        empty.value().is_none(),
        "the empty slot remains at its physical position"
    );
    assert!(
        matches!(
            future.value(),
            Some(ResourceControlParameter::UnknownParameter { .. })
        ),
        "unknown control discriminants retain their authored shape"
    );
    compare_rows(
        "/configurations/resourceComponents/rows",
        snapshot.rows(),
        &json!({"provenance": snapshot.source_inventory()}),
        catalog,
    )
}

fn check_connections(snapshot: &ResourceConnectionSnapshot<'_>, catalog: &Value) -> TestResult {
    assert!(
        snapshot.coverage().incomplete_fields.is_empty(),
        "the selected connections are complete"
    );
    let [power, heat] = snapshot.rows() else {
        return Err("both original power and heat connections".into());
    };
    assert!(
        matches!(&power.fields, ResourceConnectionFields::Power(_)),
        "power retains its typed fields"
    );
    assert!(
        matches!(&heat.fields, ResourceConnectionFields::Heat(_)),
        "heat retains its typed fields"
    );
    compare_rows(
        "/configurations/resourceConnections/rows",
        snapshot.rows(),
        &json!({"provenance": snapshot.source_inventory()}),
        catalog,
    )
}

#[test]
fn invalid_resource_controls_preserve_their_diagnostic_and_scope() -> TestResult {
    let database = fixture::invalid_controls_database();
    let (_directory, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, options.with_full_provenance())?.json())?;
    let selection = CatalogRecordSelection::RecordIndex(0);
    let session = full_proof::open(&archive, CatalogLanguageSelection::Selected(&[]), None)?;
    let parsed = session.parsed(None)?;
    parsed.with_resource_components(selection, None, |snapshot| -> TestResult {
        assert_eq!(
            snapshot.coverage().incomplete_fields,
            [0],
            "the invalid control configuration is incomplete"
        );
        let [row] = snapshot.rows() else {
            return Err("the original malformed resource row".into());
        };
        proof::cell(&serde_json::to_value(row)?, "controlParameters", "invalid")?;
        compare_rows(
            "/configurations/resourceComponents/rows",
            snapshot.rows(),
            &json!({"provenance": snapshot.source_inventory()}),
            &catalog,
        )
    })??;
    parsed.with_resource_connections(selection, None, |snapshot| -> TestResult {
        assert!(
            snapshot.coverage().incomplete_fields.is_empty(),
            "invalid controls are outside the connection scope"
        );
        compare_rows(
            "/configurations/resourceConnections/rows",
            snapshot.rows(),
            &json!({"provenance": snapshot.source_inventory()}),
            &catalog,
        )
    })??;
    Ok(())
}

fn compare_rows(
    path: &str,
    actual: &[impl serde::Serialize],
    inventory: &Value,
    catalog: &Value,
) -> TestResult {
    let actual = proof::with_origins(serde_json::to_value(actual)?, inventory)?;
    let expected = proof::with_origins(proof::at(catalog, path)?.clone(), catalog)?;
    assert_eq!(actual, expected, "{path} original fields and locations");
    Ok(())
}

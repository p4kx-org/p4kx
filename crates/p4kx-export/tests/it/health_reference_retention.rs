// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use serde_json::{Value, json};

use crate::health_origin_assertions as check;
use crate::health_origin_fixture::{self as archive, Fixture, Input, TestResult};
use crate::health_reference_fixture as fixture;
use crate::health_reference_source as source;

#[test]
fn health_reference_successful_targets_do_not_register_unretained_reference_payload() -> TestResult
{
    let bytes = fixture::database();
    source::original(&bytes)?;
    let original = Fixture::open(&bytes, Input::Encrypted, Vec::new())?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..archive::options()
    };
    let built = build_catalog_from(&original.archive, &original.entry, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let manufacturers = check::rows(&catalog, "/definitions/manufacturers/rows")?;
    assert_eq!(
        manufacturers.len(),
        2,
        "both genuine original physical targets are actually emitted"
    );
    assert!(
        check::rows(&catalog, "/exportHealth/unresolvedReferences")?.is_empty(),
        "successful target joins create no unresolved source occurrence"
    );
    let uses = check::rows(&catalog, "/provenance/uses")?;
    assert!(
        !uses.iter().any(|used| {
            used.get("role") == Some(&json!("payload"))
                && used.pointer("/location/namespace") == Some(&json!("dataforgeValue"))
                && used
                    .pointer("/location/indices")
                    .and_then(Value::as_array)
                    .is_some_and(|indices| indices.first() == Some(&json!("1")))
        }),
        "unretained Reference candidates cannot pre-register body Payload contributions"
    );
    check::closure(&catalog)
}

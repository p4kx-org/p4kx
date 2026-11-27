// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::health_origin_fixture::TestResult;
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph::Template;
use crate::template_fixture_context as context;
use crate::template_fixture_fields as fields;
use crate::template_fixture_tags as tags;

pub(crate) fn details(graph: &Template<'_>, rows: &[Value], owner: &str) -> TestResult {
    let nodes = rows
        .iter()
        .map(|row| {
            graph
                .owner
                .follow(row.get("source").ok_or("modifier source")?)
        })
        .collect::<TestResult<Vec<_>>>()?;
    let clearance = *nodes.first().ok_or("clearance modifier")?;
    assert_eq!(
        fields::text(graph.field(clearance, "modifierName")?)?,
        "GiveClearence",
        "stored name"
    );
    fields::scalar_fields(graph, clearance, &[("enabled", "boolean", json!(true))])?;
    context::property_target(
        graph,
        graph.field(clearance, "locationProperty")?,
        "Home_BP",
    )?;
    fields::reference(graph.field(clearance, "clearanceToken")?, owner)?;
    let rental = *nodes.get(1).ok_or("rental modifier")?;
    assert_eq!(
        fields::text(graph.field(rental, "modifierName")?)?,
        "RentalMiner",
        "stored rental name"
    );
    fields::scalar_fields(
        graph,
        rental,
        &[
            ("enabled", "boolean", json!(true)),
            ("clearRentalOnFail", "boolean", json!(true)),
        ],
    )?;
    fields::numeric_fields(graph, rental, &[("durationSeconds", json!(3600))])?;
    raw::source_null(graph.field(rental, "locationProperty")?);
    raw::source_null(graph.field(rental, "itemRecordGUID")?);
    let kiosk = *nodes.get(2).ok_or("kiosk modifier")?;
    kiosk_terms(graph, rows, kiosk)?;
    let law = *nodes.get(3).ok_or("law modifier")?;
    fields::scalar_fields(graph, law, &[("enabled", "boolean", json!(true))])?;
    fields::absent_field(law, "modifierName")?;
    fields::absent_field(law, "locationProperty")?;
    let license = graph.field(law, "licenseType")?;
    assert_eq!(
        fields::text(license)?,
        "IllegalGoodsLicense_ClassA",
        "complete authored licence"
    );
    assert_eq!(
        fields::vocabulary(graph, license)?.get("value"),
        Some(&json!("illegalGoodsLicenseClassA")),
        "the existing contextual licence value"
    );
    Ok(())
}

fn kiosk_terms(graph: &Template<'_>, rows: &[Value], kiosk: &Value) -> TestResult {
    fields::scalar_fields(graph, kiosk, &[("enabled", "boolean", json!(false))])?;
    fields::absent_field(kiosk, "modifierName")?;
    raw::source_null(graph.field(kiosk, "locationProperty")?);
    let terms = raw::array(rows.get(2).ok_or("kiosk interpretation")?, "kioskTerms")?;
    assert_eq!(
        terms.len(),
        1,
        "the contextual kiosk selection has one actual term"
    );
    let term = graph
        .owner
        .follow(terms.first().ok_or("actual selected term")?)?;
    let positive = fields::elements(graph, graph.field(term, "positiveTags")?)?;
    assert_eq!(positive.len(), 1, "one original positive tag");
    tags::path(
        graph,
        positive.first().ok_or("actual positive tag")?,
        Some("Kill"),
    )?;
    assert!(
        raw::elements(graph.field(term, "negativeTags")?)?.is_empty(),
        "original negative tags empty"
    );
    Ok(())
}

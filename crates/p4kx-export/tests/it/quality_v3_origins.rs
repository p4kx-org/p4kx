// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::CatalogPublicationMode;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{DcbOrigin, TestResult};
use crate::commodity_v3_source_inventory;
use crate::quality_v3_fixture_values as values;
use crate::quality_v3_origin_support::{self as origins, Domain, Use};
use crate::quality_v3_support as support;

#[test]
fn quality_v3_exposes_immediate_markers_with_distinct_original_header_and_body_locations()
-> TestResult {
    for (origin, expected) in [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ] {
        let (_, catalog) = support::build(
            &values::healthy(),
            origin,
            CatalogPublicationMode::DiagnosticPartial,
            None,
        )?;
        let distribution = support::row(&catalog, support::DISTRIBUTIONS, 0)?;
        let quantization = support::row(&catalog, support::QUANTIZATIONS, 1)?;
        let inverse = support::row(&catalog, support::INVERSE, 1)?;
        for row in [distribution, quantization, inverse] {
            origins::marker_tree(row, expected)?;
        }
        for field in [
            "guid",
            "recordName",
            "className",
            "recordPath",
            "recordIndex",
            "sourceType",
            "fileName",
            "metadataTag",
        ] {
            support::marker(support::at(distribution, &format!("/{field}"))?, expected)?;
        }
        assert_header_uses(&catalog, distribution)?;
        assert_body_use(&catalog, distribution, expected)?;
        let bands = support::at(quantization, support::BANDS)?
            .as_array()
            .ok_or("source bands")?;
        for band in bands {
            support::marker(band, expected)?;
            for field in ["sourceType", "start", "end", "mappedValue"] {
                support::marker(support::at(band, &format!("/value/{field}"))?, expected)?;
            }
        }
        commodity_v3_source_inventory::reference_closure(&catalog)?;
    }
    Ok(())
}

fn assert_header_uses(catalog: &Value, distribution: &Value) -> TestResult {
    for (cell, field, role) in [
        ("guid", "guid", "payload"),
        ("recordName", "recordName", "payload"),
        ("recordIndex", "recordIndex", "selector"),
        ("sourceType", "sourceType", "selector"),
        ("fileName", "fileName", "payload"),
        ("metadataTag", "metadataTag", "payload"),
        ("recordPath", "fileName", "conversion"),
    ] {
        assert_eq!(
            origins::uses(catalog, support::at(distribution, &format!("/{cell}"))?)?,
            BTreeSet::from([Use::new(Domain::Header, 0, field, role)]),
            "original header cells have their own namespace and roles"
        );
    }
    assert_eq!(
        origins::uses(catalog, support::at(distribution, "/className")?)?,
        BTreeSet::from([
            Use::new(Domain::Header, 0, "recordName", "conversion"),
            Use::new(Domain::Header, 0, "sourceType", "selector"),
        ]),
        "the derived class name uses its original header text and type selector"
    );
    Ok(())
}

fn assert_body_use(catalog: &Value, distribution: &Value, expected: Option<bool>) -> TestResult {
    let mean = support::at(distribution, &format!("{}/mean", support::CURVE))?;
    support::marker(mean, expected)?;
    assert_eq!(
        origins::uses(catalog, mean)?,
        BTreeSet::from([Use::new(
            Domain::Body,
            0,
            "qualityDistribution.mean",
            "payload"
        ),]),
        "a body scalar is not a Record-header field or every loaded locale"
    );
    assert_eq!(
        support::at(mean, "/sourceSetComplete")?,
        &json!(true),
        "known contributor enumeration is separate from unknown stored encryption"
    );
    Ok(())
}

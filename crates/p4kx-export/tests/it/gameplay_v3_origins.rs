// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::commodity_v3_fixture::DcbOrigin;
use crate::commodity_v3_source_inventory;
use crate::gameplay_v3_original as original;
use crate::gameplay_v3_schema as schema;
use crate::gameplay_v3_source as source;
use crate::gameplay_v3_support::{self as support, TestResult};
use crate::gameplay_v3_values as values;
use crate::quality_v3_origin_support::{self as origins, Domain, Use};
use p4kx_export::CatalogPublicationMode;
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[test]
fn gameplay_v3_exposes_per_datum_original_coordinates_roles_and_plain_encrypted_unknown_markers()
-> TestResult {
    let database = values::simple().record(values::record(
        values::SECOND,
        "Fieldless",
        values::owned(schema::PERCENT),
    ));
    source::basic(&database)?;
    source::with_tree(&database, 1, |tree| {
        assert_eq!(
            source::structure(tree.root().field("displayTransformation"))?.type_name(),
            schema::PERCENT,
            "the original fieldless conversion class is materialized before its contribution claim"
        );
        Ok(())
    })?;
    for (origin, marker) in [
        (DcbOrigin::Plain, Some(false)),
        (DcbOrigin::Encrypted, Some(true)),
        (DcbOrigin::Unknown, None),
    ] {
        let (_, catalog) = support::build(
            &database,
            origin,
            CatalogPublicationMode::DiagnosticPartial,
            None,
        )?;
        let row = support::row(&catalog, 0)?;
        origins::marker_tree(row, marker)?;
        assert_headers(&catalog, row)?;
        assert_body(&catalog, row)?;
        for record in [0, 1] {
            original::check(&catalog, &database, record)?;
        }
        commodity_v3_source_inventory::reference_closure(&catalog)?;
    }
    Ok(())
}

fn assert_headers(catalog: &Value, row: &Value) -> TestResult {
    for (field, input, role) in [
        ("guid", "guid", "payload"),
        ("recordName", "recordName", "payload"),
        ("recordIndex", "recordIndex", "selector"),
        ("sourceType", "sourceType", "selector"),
        ("fileName", "fileName", "payload"),
        ("metadataTag", "metadataTag", "payload"),
        ("recordPath", "fileName", "conversion"),
    ] {
        assert_eq!(
            origins::uses(catalog, support::at(row, &format!("/{field}"))?)?,
            BTreeSet::from([Use::new(Domain::Header, 0, input, role)]),
            "each header keeps its own actual original metadata coordinate"
        );
    }
    Ok(())
}

fn assert_body(catalog: &Value, row: &Value) -> TestResult {
    assert_eq!(
        origins::uses(catalog, support::at(row, support::SCALE)?)?,
        BTreeSet::from([Use::new(
            Domain::Body,
            0,
            "displayTransformation.scale",
            "payload"
        )]),
        "the exact numeric datum is not tainted by loaded locales or the whole row"
    );
    {
        let cell = "/displayTransformation/value/nodes/0/sourceType";
        assert_eq!(
            origins::uses(catalog, support::at(row, cell)?)?,
            BTreeSet::from([Use::new(
                Domain::Body,
                0,
                "displayTransformation.$type",
                "selector"
            )]),
            "operation classification is the actual source type selector, not executed arithmetic"
        );
    }
    assert_eq!(
        support::at(row, "/displayTransformation/value/nodes/0/sourceType/value")?,
        &json!(schema::SCALE),
        "the classified type remains original source data"
    );
    let fieldless = support::row(catalog, 1)?;
    assert_eq!(
        origins::uses(
            catalog,
            support::at(fieldless, "/displayTransformation/value/nodes/0/payload")?
        )?,
        BTreeSet::from([Use::new(
            Domain::Body,
            1,
            "displayTransformation.$type",
            "selector"
        )]),
        "a parameterless Convert declaration has a source selector and no fabricated arithmetic conversion"
    );
    Ok(())
}

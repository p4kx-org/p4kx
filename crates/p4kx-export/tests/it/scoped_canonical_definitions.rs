// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use super::{Case, Family};
use crate::loot_v3_graph as raw;
use crate::mission_template_graph as source;
use crate::scoped_canonical::{self, OWNERS, Owner};
use crate::scoped_fixture::TestResult;

pub(super) fn definition(family: &Family, case: &Case, catalog: &Value) -> TestResult {
    for owner in OWNERS {
        check_definition(family, case, catalog, owner)?;
    }
    Ok(())
}

fn check_definition(family: &Family, case: &Case, catalog: &Value, owner: Owner) -> TestResult {
    let property = owner.property(catalog)?;
    let filter = property.field(property.node, family.field)?;
    raw::structure(filter, case.source)?;
    property.filter_kind(filter, case.tag)?;
    if case.multiple {
        let definitions = property.field(filter, family.plural)?;
        let references = raw::elements(definitions)?;
        assert_eq!(
            references.len(),
            4,
            "all original definition positions survive"
        );
        let mut guids = Vec::new();
        for index in 0..references.len() {
            let node = property.element(definitions, index)?;
            if index == 1 {
                raw::source_null(node);
                guids.push(Value::Null);
            } else {
                guids.push(json!(raw::reference_guid(node)?));
            }
        }
        assert_eq!(
            guids,
            [
                json!(family.second_guid),
                Value::Null,
                json!(family.first_guid),
                json!(family.second_guid)
            ],
            "the canonical source preserves order, repetitions and null entries"
        );
        assert_ne!(
            property.element(definitions, 0)?.get("sourceOrdinal"),
            property.element(definitions, 3)?.get("sourceOrdinal"),
            "duplicate target identities remain separate original occurrences"
        );
    } else {
        check_singular(&property, filter, family)?;
    }
    Ok(())
}

fn check_singular(property: &source::Property<'_>, filter: &Value, family: &Family) -> TestResult {
    let target = source::scalar(property.field(filter, family.singular)?, "recordReference")?;
    assert_eq!(
        raw::present(target.get("guid").ok_or("actual target GUID")?)?,
        &json!(family.first_guid),
        "the actual authored singular target is retained"
    );
    assert_eq!(
        target.get("status"),
        Some(&json!("present")),
        "that exact source target is available"
    );
    assert!(
        raw::present(
            target
                .get("targetRecordIndex")
                .ok_or("actual target index")?
        )?
        .is_u64(),
        "a present target retains its original physical source position"
    );
    for field in ["recordPath", "targetType"] {
        assert!(
            raw::present(target.get(field).ok_or("actual target metadata")?)?.is_string(),
            "a present target retains independently sourced original metadata"
        );
    }
    Ok(())
}

pub(super) fn faction(catalog: &Value, source_type: &str, kind: &str) -> TestResult {
    for owner in OWNERS {
        check_faction(catalog, source_type, kind, owner)?;
    }
    Ok(())
}

fn check_faction(catalog: &Value, source_type: &str, kind: &str, owner: Owner) -> TestResult {
    let property = owner.property(catalog)?;
    let filter = property.field(property.node, "factionFilter")?;
    raw::structure(filter, source_type)?;
    property.filter_kind(filter, kind)?;
    assert_eq!(
        source::scalar(property.field(filter, "hostility")?, "enum")?,
        &json!("Hostile"),
        "both faction variants retain the exact original hostility value"
    );
    Ok(())
}

pub(super) fn location(catalog: &Value, family: &str, source_type: &str, kind: &str) -> TestResult {
    for owner in OWNERS {
        check_location(catalog, family, source_type, kind, owner)?;
    }
    Ok(())
}

fn check_location(
    catalog: &Value,
    family: &str,
    source_type: &str,
    kind: &str,
    owner: Owner,
) -> TestResult {
    let property = owner.property(catalog)?;
    let filter = property.field(property.node, family)?;
    raw::structure(filter, source_type)?;
    property.filter_kind(filter, kind)?;
    if family == "relativeLocationFilter" {
        assert_eq!(
            source::scalar(property.field(filter, "scope")?, "enum")?,
            &json!("FutureScope"),
            "the selected scope variant retains its raw original enum value"
        );
    }
    scoped_canonical::resolved(owner, &property, property.field(filter, "location")?)
}

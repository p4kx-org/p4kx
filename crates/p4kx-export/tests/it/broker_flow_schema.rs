// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde_json::{Value, json};

use crate::broker_flow_fixture as fixture;
use crate::health_origin_fixture::{self, Input, TestResult};
use crate::template_flow_depth_fixture::{Branch, Tail};

#[test]
fn the_real_broker_flow_document_has_required_closed_tagged_physical_owner_links() -> TestResult {
    let input = fixture::open(Branch::Not, 1, Tail::Known, Input::Plain)?;
    crate::broker_flow_source::check(&input, Branch::Not, 1, Tail::Known)?;
    let built = p4kx_export::build_catalog_from(
        &input.archive,
        &input.entry,
        &health_origin_fixture::options(),
    )?;
    let document: Value = serde_json::from_slice(built.json())?;
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    assert!(
        validator.is_valid(&document),
        "the actual complete canonical document satisfies the generated schema"
    );
    for path in [
        "/configurations/missionFlows/rows/0/owner",
        "/definitions/missionBrokerEntries/rows/0/missionFlows/0/owner",
        "/configurations/missionBrokerEntries/rows/0/broker",
    ] {
        for field in ["type", "inputRef", "recordIndex", "recordGuid"] {
            let mut missing = document.clone();
            missing
                .pointer_mut(path)
                .and_then(Value::as_object_mut)
                .ok_or("actual typed physical navigation")?
                .remove(field);
            assert!(
                !validator.is_valid(&missing),
                "each actual physical owner component is required"
            );
        }
        let mut unknown = document.clone();
        unknown
            .pointer_mut(path)
            .and_then(Value::as_object_mut)
            .ok_or("actual closed owner tag")?
            .insert("type".to_owned(), json!("otherOwner"));
        assert!(
            !validator.is_valid(&unknown),
            "unknown families cannot impersonate a canonical owner"
        );
    }
    assert!(
        document.get("missionBrokerEntries").is_none(),
        "the canonical C table is the only Broker context"
    );
    assert!(
        p4kx_export::catalog_schema()
            .pointer("/$defs/FlowCondition")
            .is_none(),
        "the shared condition arena has no retired recursive Broker condition schema"
    );
    Ok(())
}

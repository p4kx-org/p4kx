// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::build_catalog;
use p4kx_testkit::{TestCapacity, TestCargoGridLayout, cargo_grid_facility_database_for};
use serde_json::{Value, json};

use super::{INVENTORY_CONTAINERS, only_grid};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[test]
fn cargo_grid_closed_container_uses_its_authored_unit() -> TestResult {
    for (authored, member, expected) in [
        (
            TestCapacity::Standard(1.125),
            "standardCargoUnits",
            Some(1.125),
        ),
        (TestCapacity::Centi(175), "centiSCU", Some(1.75)),
        (TestCapacity::Micro(12_345), "microSCU", Some(0.0123)),
        (TestCapacity::Micro(0), "microSCU", Some(0.0)),
        (TestCapacity::Centi(-1), "centiSCU", None),
    ] {
        let database = cargo_grid_facility_database_for(TestCargoGridLayout::Closed(authored));
        let (_directory, archive) = common::open(&database, DcbOrigin::Encrypted)?;
        let catalog: Value =
            serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
        let capacity = proof::at(only_grid(&catalog)?, "/capacity")?;
        let scu = if let Some(expected) = expected {
            assert_eq!(
                proof::at(capacity, "/status")?,
                "available",
                "{authored:?} provides a valid nonnegative closed capacity"
            );
            assert_eq!(
                proof::at(&catalog, "/ships/0/cargoScu")?,
                &json!(expected * 2.0),
                "the same closed grid contributes once at each original port"
            );
            proof::present(capacity, "scu", &json!(expected), Some(true))?
        } else {
            assert_eq!(
                proof::at(capacity, "/status")?,
                "invalidCapacity",
                "{authored:?} retains its invalid disposition"
            );
            proof::cell(capacity, "scu", "invalid")?
        };
        check_closed_origins(&catalog, scu, member)?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

fn check_closed_origins(catalog: &Value, scu: &Value, member: &str) -> TestResult {
    let [container] = proof::rows(catalog, INVENTORY_CONTAINERS)? else {
        return Err("one closed container definition".into());
    };
    let contributors = proof::rows(scu, "/sourceUsesRefs")?;
    let unit_amount = format!("/inventoryType/value/capacity/value/{member}");
    for path in [
        "/inventoryType",
        "/inventoryType/value/capacity",
        &unit_amount,
    ] {
        let used = proof::rows(proof::at(container, path)?, "/sourceUsesRefs")?;
        assert!(
            !used.is_empty() && used.iter().all(|entry| contributors.contains(entry)),
            "the closed calculation retains its exact {path} evidence"
        );
    }
    Ok(())
}

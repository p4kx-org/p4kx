// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{ship_store_pledges, with_ship_store_slugs};
use p4kx_extraction::Archive;
use serde_json::Value;

use super::{STORE, report_scoped_rows, store_selection};
use crate::real_archive_catalog_table_parity::{
    TestResult, first_difference, wire_row, with_bound_measurement_from_producer,
};
use crate::real_archive_mining_sdk_parity::verify_mining_and_row_arrays;
use crate::real_archive_verification::print_line;

const PLEDGES: &str = "/shipStorePledges";
const SHIPS: &str = "/ships";

const JAVELIN_ROWS: [(&str, &str); 4] = [
    ("AEGS_Javelin", "special"),
    ("AEGS_Javelin_PU_AI_UEE", "ai"),
    ("AEGS_Javelin_PU_AI_UEE_FleetWeek", "ai"),
    ("AEGS_Javelin_PU_AI_UEE_FleetWeek_2021", "ai"),
];

struct StoreBinding {
    id: u32,
    slug: &'static str,
    class: &'static str,
    guid: &'static str,
}

const FLIGHT_READY: [StoreBinding; 2] = [
    StoreBinding {
        id: 224,
        slug: "Liberator",
        class: "ANVL_Liberator",
        guid: "609ce4ca-0f46-44a5-b6d3-2c2f87417126",
    },
    StoreBinding {
        id: 244,
        slug: "Galaxy",
        class: "RSI_Galaxy",
        guid: "01051d26-084e-4b4f-9566-ebd2251db0aa",
    },
];

fn reviewed_ship(class: &str) -> bool {
    JAVELIN_ROWS
        .iter()
        .any(|(candidate, _)| class.eq_ignore_ascii_case(candidate))
        || FLIGHT_READY
            .iter()
            .any(|candidate| class.eq_ignore_ascii_case(candidate.class))
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_store_slugs_match_retained_complete_catalog() -> TestResult {
    let revision = std::env::var("P4KX_STORE_SLUG_CATALOG_PRODUCER_REVISION")
        .map_err(|_| "configure the exact Store Catalog producer revision")?;
    with_bound_measurement_from_producer(&revision, |archive, measurement| {
        let rows = store_rows(archive)?;
        measurement.verify_row_array(STORE, &rows)?;
        report_scoped_rows("Store slug", "retained-parity", &rows)
    })
}

#[test]
#[ignore = "same Build: configure the installed Archive and the exact completed Normal Catalog"]
fn installed_mining_and_store_slugs_match_retained_complete_catalog() -> TestResult {
    let revision = std::env::var("P4KX_STORE_SLUG_CATALOG_PRODUCER_REVISION")
        .map_err(|_| "configure the exact Store Catalog producer revision")?;
    with_bound_measurement_from_producer(&revision, |archive, measurement| {
        let rows = store_rows(archive)?;
        verify_mining_and_row_arrays(measurement, archive, &[(STORE, &rows)])?;
        report_scoped_rows("Store slug", "retained-parity", &rows)
    })
}

#[test]
#[ignore = "same Build: configure LIVE 13220821 and its exact completed Normal Catalog"]
fn installed_reviewed_pledges_and_store_slugs_match_retained_live_catalog() -> TestResult {
    let revision = std::env::var("P4KX_STORE_SLUG_CATALOG_PRODUCER_REVISION")
        .map_err(|_| "configure the exact Store Catalog producer revision")?;
    with_bound_measurement_from_producer(&revision, |archive, measurement| {
        if archive
            .build()
            .is_none_or(|build| build.change_number() != "13220821")
        {
            return Err(
                "the reviewed Store cohort requires the original LIVE 13220821 Build".into(),
            );
        }
        let rows = store_rows(archive)?;
        let store_count = rows.len();
        let store = Value::Array(rows);
        let pledges = ship_store_pledges()?;
        let curated = serde_json::to_value(&pledges)?;
        let catalog =
            measurement.read_selected_ship_identities(&[STORE, PLEDGES], reviewed_ship)?;
        for (path, expected) in [(STORE, &store), (PLEDGES, &curated)] {
            let actual = catalog
                .pointer(path)
                .ok_or_else(|| format!("missing Catalog section {path}"))?;
            if let Some(field) = first_difference(expected, actual) {
                return Err(format!("Catalog and SDK differ at {path}{field}").into());
            }
        }
        let store_rows = store.as_array().ok_or("Store row array")?;
        verify_live_ship_joins(&catalog, store_rows)?;
        verify_reviewed_pledge_examples(&catalog, store_rows)?;
        print_line(&format!(
            "LIVE Store and pledge parity store_rows={store_count} pledge_rows={}",
            pledges.rows().len()
        ));
        Ok(())
    })
}

fn verify_live_ship_joins(catalog: &Value, store: &[Value]) -> TestResult {
    let ships = catalog
        .pointer(SHIPS)
        .and_then(Value::as_array)
        .ok_or("physical Ship rows")?;
    for (class, playability) in JAVELIN_ROWS {
        let ship = selected_ship(ships, class)?;
        assert_eq!(
            ship.pointer("/playability/value").and_then(Value::as_str),
            Some(playability),
            "physical Ship {class}"
        );
    }
    for StoreBinding {
        id,
        slug,
        class,
        guid,
    } in FLIGHT_READY
    {
        let row = store
            .iter()
            .find(|row| row["rsiId"] == id)
            .ok_or("mapped Store identity")?;
        assert_eq!(row["slug"], slug, "Store slug for RSI identity {id}");
        assert_eq!(row["className"], class, "Store class for RSI identity {id}");
        assert_eq!(row["shipGuid"], guid, "Store GUID for RSI identity {id}");
        assert_eq!(row["retired"], false, "active Store identity {id}");
        let ship = selected_ship(ships, class)?;
        assert_eq!(
            ship.pointer("/className/value").and_then(Value::as_str),
            Some(class),
            "original class for {id}"
        );
        assert_eq!(
            ship.pointer("/guid/value").and_then(Value::as_str),
            Some(guid),
            "original GUID for {id}"
        );
        assert_eq!(
            ship.pointer("/playability/value").and_then(Value::as_str),
            Some("player"),
            "original Ship availability for {id}"
        );
    }
    Ok(())
}

fn verify_reviewed_pledge_examples(catalog: &Value, store: &[Value]) -> TestResult {
    let pledge_rows = catalog
        .pointer("/shipStorePledges/rows")
        .and_then(Value::as_array)
        .ok_or("curated pledge rows")?;
    for (id, slug) in [
        (63, "Javelin"),
        (86, "Hull-D"),
        (170, "Kraken"),
        (317, "Odin"),
    ] {
        let pledge = pledge_rows
            .iter()
            .find(|row| row["rsiId"] == id)
            .ok_or_else(|| format!("missing reviewed pledge {id}"))?;
        assert_eq!(pledge["slug"], slug, "pledge identity {id}");
        assert_eq!(
            pledge["productionStatus"], "in-concept",
            "production status for pledge {id}"
        );
    }
    for id in [202, 203] {
        assert!(
            store.iter().all(|row| row["rsiId"] != id),
            "unresolved MOLE edition {id} gained a binding"
        );
        assert!(
            pledge_rows.iter().all(|row| row["rsiId"] != id),
            "unresolved MOLE edition {id} gained a concept status"
        );
    }
    Ok(())
}

fn selected_ship<'catalog>(ships: &'catalog [Value], class: &str) -> TestResult<&'catalog Value> {
    let mut matching = ships.iter().filter(|ship| {
        ship.pointer("/className/value")
            .and_then(Value::as_str)
            .is_some_and(|value| value.eq_ignore_ascii_case(class))
    });
    let ship = matching
        .next()
        .ok_or_else(|| format!("missing physical Ship {class}"))?;
    if matching.next().is_some()
        || ship
            .pointer("/recordIndex/value")
            .and_then(Value::as_u64)
            .is_none()
    {
        return Err(
            format!("ambiguous or missing physical Record identity for Ship {class}").into(),
        );
    }
    Ok(ship)
}

fn store_rows(archive: &Archive) -> TestResult<Vec<Value>> {
    with_ship_store_slugs(archive, None, None, |snapshot| {
        store_selection(archive, &snapshot, None)?;
        let rows = snapshot.rows();
        if !rows
            .iter()
            .zip(rows.iter().skip(1))
            .all(|(left, right)| left.slug() < right.slug())
        {
            return Err("the complete Store array must have a strict slug order".into());
        }
        rows.iter().map(wire_row).collect::<TestResult<Vec<_>>>()
    })
    .map_err(|_| "read the unfiltered same-Build SDK Store rows")?
}

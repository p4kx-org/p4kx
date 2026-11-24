// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog};
use p4kx_testkit::DataForgeBuilder;
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::health_origin_assertions as proof;

#[path = "catalog_item_port_axis_sdk.rs"]
mod axis_sdk;
#[path = "catalog_item_alignment_display_names.rs"]
mod display_names;
#[path = "catalog_item_alignment_links.rs"]
mod links;
#[path = "catalog_item_port_sdk.rs"]
mod sdk;
#[path = "catalog_item_port_visit.rs"]
mod visit;

#[path = "catalog_item_alignment_fixture.rs"]
mod fixture;

pub(crate) use fixture::OWNER;
use fixture::{CHILD, Layout, PORT_KEY, database};
const PORTS: &str = "/configurations/itemPorts/rows";
const ITEMS: &str = "/configurations/itemAttachmentConstraints/rows";

#[test]
fn physical_port_and_item_declarations_keep_their_independent_values() -> TestResult {
    let database = database(Layout::Complete);
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&database, origin)?;
        let catalog: Value =
            serde_json::from_slice(build_catalog(&archive, &common::options())?.json())?;
        validator
            .validate(&catalog)
            .map_err(|error| format!("catalog v3 alignment schema: {error}"))?;
        check_port_declarations(&catalog, encrypted)?;
        check_item_declarations(&catalog, encrypted)?;
        proof::closure(&catalog)?;
    }
    Ok(())
}

fn check_port_declarations(catalog: &Value, encrypted: bool) -> TestResult {
    let [ports, child_ports] = proof::rows(catalog, PORTS)? else {
        return Err("one physical port container from each selected Entity".into());
    };
    proof::present(
        ports,
        "ownerGuid",
        &json!(common::guid(OWNER)),
        Some(encrypted),
    )?;
    proof::present(ports, "componentOrdinal", &json!(1), Some(encrypted))?;
    let [first, second] = proof::rows(ports, "/ports/value")? else {
        return Err("both physical ports in their authored order".into());
    };
    let first = proof::at(first, "/value")?;
    let second = proof::at(second, "/value")?;
    check_axis(
        first,
        "attachAxesToAlign",
        "FixturePortAxis",
        "declared",
        encrypted,
    )?;
    check_axis(
        second,
        "attachAxesToAlign",
        "FixtureFutureAxis",
        "undeclared",
        encrypted,
    )?;
    proof::present(first, "portOrdinal", &json!(0), Some(encrypted))?;
    proof::present(second, "portOrdinal", &json!(1), Some(encrypted))?;
    proof::present(
        child_ports,
        "ownerGuid",
        &json!(common::guid(CHILD)),
        Some(encrypted),
    )?;

    Ok(())
}

fn check_item_declarations(catalog: &Value, encrypted: bool) -> TestResult {
    let [_, child] = proof::rows(catalog, ITEMS)? else {
        return Err("the owner and child each have a physical attachable component".into());
    };
    let child = proof::at(child, "/definition/value")?;
    check_axis(
        child,
        "attachAxesToAlign",
        "FixtureItemAxis",
        "declared",
        encrypted,
    )?;
    check_axis(
        child,
        "ignoredAttachAxis",
        "FixtureStoredDefault",
        "declared",
        encrypted,
    )?;
    proof::present(
        child,
        "attachCheckCenterOfMass",
        &json!(false),
        Some(encrypted),
    )?;
    Ok(())
}

#[test]
fn undeclared_alignment_members_do_not_become_defaults() -> TestResult {
    let (_built, catalog) = common::build(&database(Layout::Absent))?;
    let item = proof::at(&catalog, &format!("{ITEMS}/0/definition/value"))?;
    let axis = proof::at(item, "/attachAxesToAlign")?;
    proof::cell(axis, "label", "absent")?;
    assert_eq!(proof::at(axis, "/optionStatus")?, &Value::Null);
    proof::cell(item, "attachCheckCenterOfMass", "absent")?;
    let port = proof::at(
        &catalog,
        &format!("{PORTS}/0/ports/value/0/value/attachAxesToAlign"),
    )?;
    proof::cell(port, "label", "absent")?;
    assert_eq!(proof::at(port, "/optionStatus")?, &Value::Null);
    Ok(())
}

#[test]
fn unreadable_original_axis_keeps_its_physical_port_and_failure_state() -> TestResult {
    let (_folder, archive) = common::open(&database(Layout::Unreadable), DcbOrigin::Encrypted)?;
    let mut options = common::options();
    assert!(
        matches!(
            build_catalog(&archive, &options),
            Err(CatalogError::IncompleteExport(_))
        ),
        "the invalid original label prevents a complete export"
    );
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let future = proof::at(&catalog, &format!("{PORTS}/0/ports/value/1/value"))?;
    proof::present(future, "portOrdinal", &json!(1), Some(true))?;
    let axis = proof::at(future, "/attachAxesToAlign")?;
    let label = proof::cell(axis, "label", "unreadable")?;
    assert_eq!(proof::at(label, "/sourceSetComplete")?, &json!(false));
    assert_eq!(proof::at(axis, "/optionStatus")?, &Value::Null);
    proof::closure(&catalog)
}

fn check_axis(
    owner: &Value,
    field: &str,
    label: &str,
    status: &str,
    encrypted: bool,
) -> TestResult {
    let axis = owner.get(field).ok_or("alignment axis")?;
    proof::present(axis, "label", &json!(label), Some(encrypted))?;
    proof::present(axis, "optionStatus", &json!(status), Some(encrypted))?;
    Ok(())
}

pub(super) fn consumer_ports() -> DataForgeBuilder {
    database(Layout::Absent)
}

#[path = "equipment_configuration_sdk.rs"]
mod equipment_sdk;

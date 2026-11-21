// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForge, StructValue, Value as Original};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::flow_root_positions::{Step, enumerate};
use crate::health_origin_assertions as proof;

#[path = "catalog_emp_fixture.rs"]
mod fixture;

use fixture::Layout;

const TABLE: &str = "/configurations/emp";
const ROWS: &str = "/configurations/emp/rows";

#[test]
fn catalog_emp_preserves_exact_parameters_links_and_per_field_origins() -> TestResult {
    let source = fixture::database(Layout::Full);
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.record(0).ok_or("fixture entity")?.tree()?;
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("original component array".into());
    };
    let positions = enumerate(tree.root());
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&source, origin)?;
        let built = build_catalog(&archive, &common::options())?;
        let catalog: Value = serde_json::from_slice(built.json())?;
        let rows = proof::rows(&catalog, ROWS)?;
        assert_eq!(rows.len(), 2, "two physical EMP occurrences");
        let trace = Trace {
            catalog: &catalog,
            positions: &positions,
            encrypted,
        };
        for (slot, row) in (1_usize..).zip(rows) {
            let Some(Original::Struct(component)) = components.get(slot) else {
                return Err("original EMP occurrence".into());
            };
            trace.owner(row, slot)?;
            trace.numbers(row, component, slot)?;
            trace.presentation(row, component, slot)?;
            trace.links(row)?;
            assert_eq!(arena_names(row)?, ["fixtureOwnedState", "fixtureProfile"]);
        }
        assert_ne!(
            proof::at(&rows[0], "/idleState/value/target/location/value")?,
            proof::at(&rows[1], "/idleState/value/target/location/value")?,
            "the two physical state targets remain independently addressable",
        );
        proof::closure(&catalog)?;
        let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
        assert!(
            validator.is_valid(&catalog),
            "the actual EMP wire is valid schema v3"
        );
    }
    Ok(())
}

#[test]
fn catalog_emp_distinguishes_missing_family_field_and_readable_wrong_storage() -> TestResult {
    for (layout, availability, count) in [
        (Layout::NoEmp, "absent", 0),
        (Layout::Reduced, "available", 1),
    ] {
        let (_, catalog) = common::build(&fixture::database(layout))?;
        assert_eq!(
            proof::at(proof::at(&catalog, TABLE)?, "/availability")?,
            availability
        );
        let rows = proof::rows(&catalog, ROWS)?;
        assert_eq!(
            rows.len(),
            count,
            "table availability and row count are independent"
        );
        if let [row] = rows {
            proof::cell(row, "cooldownTime", "absent")?;
            proof::cell(row, "startUnleashTrigger", "null")?;
        }
    }
    let source = fixture::database(Layout::WrongDeclaration);
    let (_folder, archive) = common::open(&source, DcbOrigin::Encrypted)?;
    assert!(matches!(
        build_catalog(&archive, &common::options()),
        Err(CatalogError::IncompleteExport(_))
    ));
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let row = proof::at(&catalog, "/configurations/emp/rows/0")?;
    proof::cell(row, "pressure", "invalid")?;
    assert_eq!(
        arena_names(row)?,
        ["fixtureOwnedState", "fixtureProfile", "pressure"]
    );
    Ok(())
}

struct Trace<'a> {
    catalog: &'a Value,
    positions: &'a [Vec<Step>],
    encrypted: bool,
}

impl Trace<'_> {
    fn owner(&self, row: &Value, slot: usize) -> TestResult {
        for (name, value) in [
            ("ownerGuid", json!(common::guid(fixture::OWNER))),
            ("ownerRecordIndex", json!(0)),
            ("componentOrdinal", json!(slot)),
            ("sourceType", json!(fixture::EMP)),
        ] {
            proof::present(row, name, &value, Some(self.encrypted))?;
        }
        Ok(())
    }

    fn numbers(&self, row: &Value, component: &StructValue<'_>, slot: usize) -> TestResult {
        for name in fixture::PARAMETERS {
            let Some(Original::Single(value)) = component.field(name) else {
                return Err("original numeric field".into());
            };
            let finite = value.is_finite().then_some(*value);
            let expected = json!({"rawBits": format!("{:08x}",value.to_bits()), "finite": finite});
            let cell = proof::present(row, name, &expected, Some(self.encrypted))?;
            self.payload(
                cell,
                &[Step::Field(0), Step::Element(slot), field(component, name)?],
            )?;
        }
        Ok(())
    }

    fn presentation(&self, row: &Value, component: &StructValue<'_>, slot: usize) -> TestResult {
        for (name, wire) in [
            ("ChargedParticle", "chargedParticle"),
            ("ChargingParticle", "chargingParticle"),
        ] {
            let Some(Original::Struct(particle)) = component.field(name) else {
                return Err("original particle Class".into());
            };
            let Some(Original::String(path)) = particle.field("path") else {
                return Err("original particle path".into());
            };
            let block = proof::at(proof::cell(row, wire, "present")?, "/value")?;
            let cell = proof::present(block, "path", &json!(path), Some(self.encrypted))?;
            proof::present(
                block,
                "sourceType",
                &json!(fixture::PARTICLE),
                Some(self.encrypted),
            )?;
            assert_eq!(
                arena_names(block)?,
                ["fixtureVariation"],
                "unprojected particle member survives"
            );
            self.payload(
                cell,
                &[
                    Step::Field(0),
                    Step::Element(slot),
                    field(component, name)?,
                    field(particle, "path")?,
                ],
            )?;
        }
        Ok(())
    }

    fn links(&self, row: &Value) -> TestResult {
        for name in ["idleState", "chargedState"] {
            let state = proof::cell(row, name, "present")?;
            let target = proof::at(state, "/value/target")?;
            assert_eq!(
                proof::at(target, "/type")?,
                "instance",
                "the original pointer holds an owned instance"
            );
            proof::present(
                target,
                "sourceType",
                &json!(fixture::STATE),
                Some(self.encrypted),
            )?;
            proof::present(
                proof::at(state, "/value")?,
                "recordGuid",
                &json!(common::guid(fixture::OWNER)),
                Some(self.encrypted),
            )?;
        }
        for name in ["chargingState", "releasingState"] {
            proof::cell(row, name, "null")?;
        }
        self.reference(row, "chargedTag", fixture::CHARGED_TAG, "present")?;
        self.reference(row, "chargingTag", fixture::CHARGING_TAG, "present")?;
        for (index, (_, name)) in (0_u128..).zip(fixture::TRIGGERS) {
            match index {
                4 => {
                    proof::cell(row, name, "null")?;
                }
                5 => self.reference(row, name, fixture::MISSING_TRIGGER, "dangling")?,
                _ => self.reference(row, name, fixture::FIRST_TRIGGER + index, "present")?,
            }
        }
        Ok(())
    }

    fn reference(&self, row: &Value, name: &str, guid: u128, status: &str) -> TestResult {
        let link = proof::at(proof::cell(row, name, "present")?, "/value")?;
        assert_eq!(
            proof::at(link, "/status")?,
            status,
            "original reference status"
        );
        proof::present(
            link,
            "guid",
            &json!(common::guid(guid)),
            Some(self.encrypted),
        )?;
        Ok(())
    }

    fn payload(&self, cell: &Value, path: &[Step]) -> TestResult {
        let ordinal = self
            .positions
            .iter()
            .position(|actual| actual == path)
            .ok_or("original physical field ordinal")?;
        assert!(
            proof::uses(self.catalog, cell)?.iter().any(|usage| usage
                .get("role")
                .and_then(Value::as_str)
                == Some("payload")
                && usage.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeValue")
                && usage.pointer("/location/field").and_then(Value::as_str) == Some("$value")
                && usage.pointer("/location/indices") == Some(&json!(["0", ordinal.to_string()]))),
            "each projected field keeps its own physical payload receipt"
        );
        Ok(())
    }
}

fn field(owner: &StructValue<'_>, name: &str) -> TestResult<Step> {
    let ordinal = owner
        .fields()
        .iter()
        .position(|field| field.name() == name)
        .ok_or("original field declaration")?;
    Ok(Step::Field(ordinal))
}

fn arena_names(row: &Value) -> TestResult<Vec<&str>> {
    proof::rows(row, "/additionalFields/roots")?
        .iter()
        .map(|root| {
            proof::at(root, "/name/value")?
                .as_str()
                .ok_or_else(|| "original member name".into())
        })
        .collect()
}

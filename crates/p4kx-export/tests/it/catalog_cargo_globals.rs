// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_export::{
    CatalogError, CatalogOptions, CatalogPublicationMode, ExportDiagnosticCode, Filter, Kind,
    build_catalog,
};
use p4kx_extraction::{DataForge, StructValue, Value as Original};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::flow_root_positions::{Step, enumerate};
use crate::health_origin_assertions as proof;

#[path = "catalog_cargo_globals_fixture.rs"]
mod fixture;
#[path = "catalog_cargo_globals_sdk.rs"]
mod sdk;

const LOADING_ROWS: &str = "/configurations/cargoLoadingGlobals/rows";
const SHOP_ROWS: &str = "/configurations/shopCommodityGlobals/rows";
const BOX_LOADING: &str = "autoLoadingBoxSizeLoadingTime";
const BOX_UNLOADING: &str = "autoLoadingBoxSizeUnloadingTime";
const BOX_PRICES: &str = "autoLoadingBoxSizePrices";
const SCU_MEMBERS: [(&str, &str); 10] = [
    ("0.125", "one_eighthSCU"),
    ("0.25", "one_quarterSCU"),
    ("0.5", "one_halfSCU"),
    ("1", "oneSCU"),
    ("2", "twoSCU"),
    ("4", "fourSCU"),
    ("8", "eightSCU"),
    ("16", "sixteenSCU"),
    ("24", "twentyFourSCU"),
    ("32", "thirtyTwoSCU"),
];

#[test]
fn cargo_globals_keep_original_numbers_box_names_and_physical_source_uses() -> TestResult {
    let source = fixture::database(fixture::Layout::Authored);
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&source, origin)?;
        let catalog: Value = serde_json::from_slice(
            build_catalog(&archive, common::options().with_full_provenance())?.json(),
        )?;
        assert_eq!(
            proof::at(&catalog, "/schemaVersion")?,
            &json!(3),
            "catalog v3"
        );
        let [first, second] = proof::rows(&catalog, LOADING_ROWS)? else {
            return Err("two physical cargo Records with distinct GUIDs".into());
        };
        let [shop] = proof::rows(&catalog, SHOP_ROWS)? else {
            return Err("one independent shop commodity Record".into());
        };
        for (index, row) in [(0, first), (1, second), (2, shop)] {
            let record = forge.record(index).ok_or("original cargo or shop Record")?;
            let tree = record.tree()?;
            let witness = SourceWitness {
                catalog: &catalog,
                root: tree.root(),
                positions: enumerate(tree.root()),
                record_index: index,
                encrypted,
            };
            witness.header(row, &record.guid().to_string(), tree.type_name())?;
            if tree.type_name() == fixture::LOADING_TYPE {
                witness.loading(row)?;
            } else {
                witness.scalar(row, "autoLoadingBasePrice")?;
                witness.box_sizes(row, BOX_PRICES, fixture::PRICE_TYPE)?;
            }
        }
        proof::closure(&catalog)?;
    }
    Ok(())
}

#[test]
fn cargo_globals_reports_both_physical_records_when_a_guid_is_ambiguous() -> TestResult {
    let (_folder, archive) = common::open(
        &fixture::database(fixture::Layout::DuplicateIdentity),
        DcbOrigin::Encrypted,
    )?;
    let Err(CatalogError::IncompleteExport(refusal)) =
        build_catalog(&archive, common::options().with_full_provenance())
    else {
        return Err("normal publication must refuse ambiguous cargo identity".into());
    };
    let indices: BTreeSet<_> = refusal
        .health()
        .failed_records()
        .iter()
        .filter(|failure| {
            failure.kind == Some(Kind::CargoLoadingGlobals)
                && failure.code == ExportDiagnosticCode::AmbiguousIdentity
        })
        .map(|failure| {
            failure
                .record_index()
                .value()
                .copied()
                .ok_or("physical cargo Record index")
        })
        .collect::<Result<_, _>>()?;
    assert_eq!(
        indices,
        BTreeSet::from([0, 1]),
        "both physical Records are reported"
    );
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, options.with_full_provenance())?.json())?;
    let [first, second] = proof::rows(&catalog, LOADING_ROWS)? else {
        return Err("diagnostic publication must retain both cargo Records".into());
    };
    let guid = json!(common::guid(fixture::SHARED_LOADING_GUID));
    for (index, row) in [(0, first), (1, second)] {
        proof::present(row, "recordIndex", &json!(index), Some(true))?;
        let cell = proof::present(row, "guid", &guid, Some(true))?;
        proof::header(&catalog, cell, index, "guid", "payload", "archiveEntry")?;
    }
    proof::closure(&catalog)
}

#[test]
fn cargo_globals_filter_selects_physical_record_and_preserves_family_availability() -> TestResult {
    let (_folder, archive) = common::open(
        &fixture::database(fixture::Layout::DuplicateIdentity),
        DcbOrigin::Encrypted,
    )?;
    let filter = Filter::new(fixture::LOADING_FILTER);
    let options = CatalogOptions {
        filter: Some(&filter),
        ..common::options()
    };
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, options.with_full_provenance())?.json())?;
    let [row] = proof::rows(&catalog, LOADING_ROWS)? else {
        return Err("the filter must select just the physical matching Record".into());
    };
    proof::present(row, "recordIndex", &json!(1), Some(true))?;
    proof::present(
        row,
        "guid",
        &json!(common::guid(fixture::SHARED_LOADING_GUID)),
        Some(true),
    )?;
    let cell = proof::present(
        row,
        "autoLoadingBaseLoadingTime",
        &json!({"rawBits":"40000000", "finite":2.0}),
        Some(true),
    )?;
    assert!(
        proof::uses(&catalog, cell)?
            .iter()
            .any(|usage| usage.pointer("/location/indices/0") == Some(&json!("1"))),
        "the selected timing retains the original physical Record index"
    );
    assert!(
        proof::rows(&catalog, SHOP_ROWS)?.is_empty(),
        "no shop Record matched the path"
    );
    assert_eq!(
        proof::at(
            &catalog,
            "/configurations/shopCommodityGlobals/availability"
        )?,
        "available",
        "a declared shop family stays available even with no selected rows"
    );
    Ok(())
}

#[test]
fn cargo_globals_keeps_an_invalid_box_member_in_the_diagnostic_source_arena() -> TestResult {
    let (_folder, archive) = common::open(
        &fixture::database(fixture::Layout::InvalidTimeMember),
        DcbOrigin::Encrypted,
    )?;
    assert!(
        matches!(
            build_catalog(&archive, common::options().with_full_provenance()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "ordinary publication rejects a wrong box timing storage type"
    );
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..common::options()
    };
    let catalog: Value =
        serde_json::from_slice(build_catalog(&archive, options.with_full_provenance())?.json())?;
    let row = proof::rows(&catalog, LOADING_ROWS)?
        .first()
        .ok_or("first cargo Record")?;
    let block = proof::at(row, "/autoLoadingBoxSizeLoadingTime/value")?;
    let map = proof::at(block, "/byScu")?;
    let invalid = proof::cell(map, "0.5", "invalid")?;
    verify_invalid_source(&catalog, block, invalid)?;
    assert_eq!(
        authored_payload(block, "one_halfSCU")?,
        &json!({"type":"number", "value":{"type":"uint32", "value":42}}),
        "the mismatched half-SCU cell retains the original UInt32"
    );
    assert_eq!(
        authored_payload(block, "futureTimeFlag")?,
        &json!({"type":"boolean", "value":true}),
        "the same diagnostic row retains independent unknown members"
    );
    proof::closure(&catalog)
}

fn verify_invalid_source(catalog: &Value, block: &Value, invalid: &Value) -> TestResult {
    let arena = proof::at(block, "/additionalFields")?;
    let raw = authored_field(arena, "one_halfSCU")?;
    let declaration = proof::present(
        proof::at(raw, "/declaration")?,
        "dataType",
        &json!("uint32"),
        Some(true),
    )?;
    let observed_value = selector(catalog, invalid, "$value")?;
    let declared_type = selector(catalog, declaration, "$dataType")?;
    assert_eq!(
        proof::at(observed_value, "/location/indices")?,
        proof::at(declared_type, "/location/indices")?,
        "the typed rejection and raw UInt32 declaration identify the same physical source field"
    );
    Ok(())
}

fn selector<'a>(catalog: &'a Value, carrier: &Value, field: &str) -> TestResult<&'a Value> {
    proof::uses(catalog, carrier)?
        .into_iter()
        .find(|entry| {
            entry.get("role").and_then(Value::as_str) == Some("selector")
                && entry.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeValue")
                && entry.pointer("/location/field").and_then(Value::as_str) == Some(field)
        })
        .ok_or_else(|| format!("original {field} selector").into())
}

struct SourceWitness<'a, 'source> {
    catalog: &'a Value,
    root: &'a StructValue<'source>,
    positions: Vec<Vec<Step>>,
    record_index: u32,
    encrypted: bool,
}

impl SourceWitness<'_, '_> {
    fn header(&self, output: &Value, guid: &str, source_type: &str) -> TestResult {
        let marker = Some(self.encrypted);
        proof::present(output, "recordIndex", &json!(self.record_index), marker)?;
        proof::present(output, "sourceType", &json!(source_type), marker)?;
        let cell = proof::present(output, "guid", &json!(guid), marker)?;
        proof::header(
            self.catalog,
            cell,
            self.record_index,
            "guid",
            "payload",
            "archiveEntry",
        )
    }

    fn loading(&self, output: &Value) -> TestResult {
        for name in [
            "autoLoadingBaseLoadingTime",
            "autoLoadingBaseUnloadingTime",
            "cargoDeckLoadingTimePerSCU",
        ] {
            self.scalar(output, name)?;
        }
        self.box_sizes(output, BOX_LOADING, fixture::TIME_TYPE)?;
        self.box_sizes(output, BOX_UNLOADING, fixture::TIME_TYPE)?;
        assert_eq!(
            authored_payload(output, "futureCargoFlag")?,
            &json!({"type":"boolean", "value":false}),
            "the loading row retains the original unknown root field"
        );
        Ok(())
    }

    fn scalar(&self, output: &Value, name: &str) -> TestResult {
        let original = self.root.field(name).ok_or("original scalar")?;
        let cell = proof::present(output, name, &number(original)?, Some(self.encrypted))?;
        self.payload(cell, &[field_position(self.root, name)?])
    }

    fn box_sizes(&self, output: &Value, name: &str, source_type: &str) -> TestResult {
        let Some(Original::Struct(original)) = self.root.field(name) else {
            return Err("original box-size Class".into());
        };
        let block = proof::at(proof::cell(output, name, "present")?, "/value")?;
        proof::present(
            block,
            "sourceType",
            &json!(source_type),
            Some(self.encrypted),
        )?;
        let map = proof::at(block, "/byScu")?;
        assert_eq!(
            map.as_object().ok_or("SCU lookup object")?.len(),
            SCU_MEMBERS.len(),
            "{name} exposes all supported canonical keys"
        );
        let root_step = field_position(self.root, name)?;
        for (scu, member) in SCU_MEMBERS {
            if let Some(value) = original.field(member) {
                let cell = proof::present(map, scu, &number(value)?, Some(self.encrypted))?;
                let path = [root_step.clone(), field_position(original, member)?];
                self.payload(cell, &path)?;
            } else {
                proof::cell(map, scu, "absent")?;
            }
        }
        Ok(())
    }

    fn payload(&self, cell: &Value, path: &[Step]) -> TestResult {
        let ordinal = self
            .positions
            .iter()
            .position(|actual| actual == path)
            .ok_or("original physical value ordinal")?;
        let indices = json!([self.record_index.to_string(), ordinal.to_string()]);
        let uses = proof::uses(self.catalog, cell)?;
        let original = uses
            .iter()
            .find(|usage| {
                usage.get("role") == Some(&json!("payload"))
                    && usage.pointer("/location/namespace") == Some(&json!("dataforgeValue"))
                    && usage.pointer("/location/indices") == Some(&indices)
                    && usage.pointer("/location/field") == Some(&json!("$value"))
            })
            .ok_or("per-cell payload references its exact physical DataForge ordinal")?;
        assert_eq!(
            proof::at(proof::source(self.catalog, original)?, "/kind")?,
            "archiveEntry",
            "the original source is the DCB Entry"
        );
        Ok(())
    }
}

fn number(original: &Original<'_>) -> TestResult<Value> {
    match original {
        Original::Single(value) => Ok(json!({
            "rawBits": format!("{:08x}", value.to_bits()),
            "finite": value.is_finite().then_some(value),
        })),
        Original::Int32(value) => Ok(json!(value)),
        _ => Err("an original Single or Int32 cargo input".into()),
    }
}

fn field_position(owner: &StructValue<'_>, name: &str) -> TestResult<Step> {
    owner
        .fields()
        .iter()
        .position(|field| field.name() == name)
        .map(Step::Field)
        .ok_or_else(|| "original field declaration".into())
}

fn authored_payload<'a>(owner: &'a Value, name: &str) -> TestResult<&'a Value> {
    let arena = proof::at(owner, "/additionalFields")?;
    let root = authored_field(arena, name)?;
    let index: usize = proof::at(root, "/value/valueIndex")?
        .as_str()
        .ok_or("arena index")?
        .parse()?;
    let node = proof::rows(arena, "/values")?
        .get(index)
        .ok_or("original arena node")?;
    proof::at(proof::cell(node, "value", "present")?, "/value")
}

fn authored_field<'a>(arena: &'a Value, name: &str) -> TestResult<&'a Value> {
    proof::rows(arena, "/roots")?
        .iter()
        .find(|root| root.pointer("/name/value").and_then(Value::as_str) == Some(name))
        .ok_or_else(|| format!("original authored arena field {name}").into())
}

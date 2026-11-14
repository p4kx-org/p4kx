// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForge, StructValue, Value as Original};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::flow_root_positions::{Step, enumerate};
use crate::health_origin_assertions as proof;

#[path = "catalog_quantum_fixture.rs"]
mod fixture;

use fixture::{BOOST_FIELDS, HEAT_FIELDS, Layout, PROFILE_FIELDS};

const ROWS: &str = "/configurations/quantumDrives/rows";
const ROOT_FIELDS: [(&str, &str); 3] = [
    ("quantumFuelRequirement", "fuelRequirement"),
    ("jumpRange", "jumpRange"),
    ("disconnectRange", "disconnectRange"),
];

#[test]
fn quantum_drive_profiles_keep_distinct_source_values_and_physical_owners() -> TestResult {
    let source = fixture::database(Layout::Full);
    let bytes = source.build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.record(0).ok_or("original fixture owner")?.tree()?;
    let Some(Original::Array(components)) = tree.field("Components") else {
        return Err("original fixture Components array".into());
    };
    let positions = enumerate(tree.root());
    let field = field_position(tree.root(), "Components")?;
    for (origin, encrypted) in [(DcbOrigin::Plain, false), (DcbOrigin::Encrypted, true)] {
        let (_folder, archive) = common::open(&source, origin)?;
        let built = build_catalog(&archive, &common::options())?;
        assert!(
            built.json() == build_catalog(&archive, &common::options())?.json(),
            "fixed source and export time produce byte-identical output"
        );
        let catalog: Value = serde_json::from_slice(built.json())?;
        let rows = proof::rows(&catalog, ROWS)?;
        assert_eq!(rows.len(), 2, "two physical components on the same owner");
        for (slot, row) in (1_usize..).zip(rows) {
            let Some(Original::Struct(drive)) = components.get(slot) else {
                return Err("original physical quantum component".into());
            };
            proof::present(row, "sourceType", &json!(fixture::DRIVE), Some(encrypted))?;
            proof::present(
                row,
                "ownerGuid",
                &json!(common::guid(fixture::OWNER)),
                Some(encrypted),
            )?;
            proof::present(row, "ownerRecordIndex", &json!(0), Some(encrypted))?;
            proof::present(row, "componentOrdinal", &json!(slot), Some(encrypted))?;
            let original = SourceTrace {
                catalog: &catalog,
                positions: &positions,
                encrypted,
            };
            let path = [Step::Field(field), Step::Element(slot)];
            original.fields(drive, &path, row, &ROOT_FIELDS)?;
            verify_blocks(&original, drive, &path, row)?;
            verify_extra(row)?;
        }
    }
    Ok(())
}

#[test]
fn quantum_drive_absence_empty_class_and_invalid_storage_remain_distinct() -> TestResult {
    for (layout, missing, present) in [
        (Layout::WithoutSpline, "spline", "standard"),
        (Layout::WithoutStandard, "standard", "spline"),
    ] {
        let (_folder, archive) = common::open(&fixture::database(layout), DcbOrigin::Encrypted)?;
        let built = build_catalog(&archive, &common::options())?;
        let catalog: Value = serde_json::from_slice(built.json())?;
        let row = proof::rows(&catalog, ROWS)?.first().ok_or("quantum row")?;
        proof::cell(row, missing, "absent")?;
        proof::cell(row, present, "present")?;
        proof::cell(row, "boost", "absent")?;
        if matches!(layout, Layout::WithoutSpline) {
            let heat = proof::at(proof::cell(row, "heat", "present")?, "/value")?;
            proof::present(heat, "sourceType", &json!(fixture::EMPTY_HEAT), Some(true))?;
            for (_, key) in HEAT_FIELDS {
                proof::cell(heat, key, "absent")?;
            }
            assert!(
                proof::rows(heat, "/additionalFields/roots")?.is_empty(),
                "empty authored heat type"
            );
        } else {
            proof::cell(row, "heat", "absent")?;
        }
    }

    let (_folder, archive) = common::open(
        &fixture::database(Layout::InvalidSpline),
        DcbOrigin::Encrypted,
    )?;
    assert!(
        matches!(
            build_catalog(&archive, &common::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "wrong declared storage makes the ordinary export incomplete"
    );
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let row = proof::rows(&catalog, ROWS)?.first().ok_or("quantum row")?;
    let invalid = proof::cell(row, "spline", "invalid")?;
    assert!(
        proof::uses(&catalog, invalid)?.iter().any(|entry| entry
            .pointer("/location/field")
            .and_then(Value::as_str)
            == Some("$dataType")),
        "wrong storage retains its physical declaration selector"
    );
    assert_eq!(
        authored_payload(proof::at(row, "/additionalFields")?, "splineJumpParams")?,
        &json!({"type":"number","value":{"type":"uint32","value":42}}),
        "diagnostic publication keeps the original malformed field alongside the readable item"
    );
    Ok(())
}

fn verify_blocks(
    original: &SourceTrace<'_>,
    drive: &StructValue<'_>,
    path: &[Step],
    row: &Value,
) -> TestResult {
    for (name, key, fields) in [
        ("params", "standard", PROFILE_FIELDS.as_slice()),
        ("splineJumpParams", "spline", PROFILE_FIELDS.as_slice()),
        ("heatParams", "heat", HEAT_FIELDS.as_slice()),
        ("quantumBoostParams", "boost", BOOST_FIELDS.as_slice()),
    ] {
        let Some(Original::Struct(block)) = drive.field(name) else {
            return Err("original fixture block".into());
        };
        let cell = proof::cell(row, key, "present")?;
        let block_path = extend(path, Step::Field(field_position(drive, name)?));
        original.receipt(cell, &block_path, "$type", "selector")?;
        original.receipt(cell, &block_path, "$dataType", "selector")?;
        original.receipt(cell, &block_path, "$target", "selector")?;
        let exported = proof::at(cell, "/value")?;
        proof::present(
            exported,
            "sourceType",
            &json!(block.type_name()),
            Some(original.encrypted),
        )?;
        original.fields(block, &block_path, exported, fields)?;
        if matches!(key, "standard" | "spline") {
            verify_members(original, block, &block_path, exported)?;
        }
    }
    Ok(())
}

fn verify_members(
    original: &SourceTrace<'_>,
    block: &StructValue<'_>,
    path: &[Step],
    output: &Value,
) -> TestResult {
    for name in ["idleState", "travellingState"] {
        let Some(Original::Struct(state)) = block.field(name) else {
            return Err("original fixture state struct".into());
        };
        let Some(Original::String(text)) = state.field("value") else {
            return Err("original fixture state label".into());
        };
        let selected = proof::cell(output, name, "present")?;
        let field_path = extend(path, Step::Field(field_position(block, name)?));
        original.receipt(selected, &field_path, "$value", "selector")?;
        let arena = proof::at(selected, "/value")?;
        let body = authored_payload(arena, name)?;
        let field = proof::rows(body, "/fields")?
            .first()
            .ok_or("state member")?;
        assert_eq!(
            proof::at(
                arena_node(arena, proof::at(field, "/value")?)?,
                "/value/value/value/sourceText"
            )?,
            &json!(text),
            "each mode retains its own authored state label"
        );
    }
    let Some(Original::Array(tags)) = block.field("effectTags") else {
        return Err("original fixture effect tags".into());
    };
    let arena = proof::at(proof::cell(output, "effectTags", "present")?, "/value")?;
    let links = proof::rows(authored_payload(arena, "effectTags")?, "/elements")?;
    assert_eq!(
        links.len(),
        tags.len(),
        "the original effect order is preserved"
    );
    for (link, original) in links.iter().zip(tags) {
        let Original::Struct(tag) = original else {
            return Err("original fixture effect tag slot".into());
        };
        let Some(Original::Reference(reference)) = tag.field("tagRecord") else {
            return Err("original fixture effect tag reference".into());
        };
        let fields = proof::rows(arena_node(arena, link)?, "/value/value/fields")?;
        let reference_field = fields.first().ok_or("effect tag reference field")?;
        let reference_node = arena_node(arena, proof::at(reference_field, "/value")?)?;
        assert_eq!(
            proof::at(reference_node, "/value/value/value/guid/value")?,
            &json!(reference.guid().to_string()),
            "the exact original reference survives per effect slot"
        );
    }
    assert_eq!(
        authored_payload(
            proof::at(output, "/additionalFields")?,
            "fixtureUnknownFlag"
        )?,
        &json!({"type":"boolean","value":false}),
        "readable unknown variant fields remain present"
    );
    Ok(())
}

fn verify_extra(row: &Value) -> TestResult {
    let arena = proof::at(row, "/additionalFields")?;
    assert_eq!(
        authored_payload(arena, "fixtureRootGuid")?,
        &json!({"type":"guid","value":common::guid(fixture::ROOT_GUID)}),
        "the unknown root GUID is preserved"
    );
    let node = authored_node(arena, "fixtureNullReference")?;
    proof::cell(node, "value", "null")?;
    Ok(())
}

struct SourceTrace<'a> {
    catalog: &'a Value,
    positions: &'a [Vec<Step>],
    encrypted: bool,
}

impl SourceTrace<'_> {
    fn fields(
        &self,
        source: &StructValue<'_>,
        path: &[Step],
        output: &Value,
        fields: &[(&str, &str)],
    ) -> TestResult {
        for &(name, key) in fields {
            let value = source.field(name).ok_or("fixture number")?;
            let expected = exact_number(value)?;
            let cell = proof::present(output, key, &expected, Some(self.encrypted))?;
            let ordinal = extend(path, Step::Field(field_position(source, name)?));
            self.receipt(cell, &ordinal, "$value", "payload")?;
        }
        Ok(())
    }

    fn receipt(&self, cell: &Value, path: &[Step], field: &str, role: &str) -> TestResult {
        let ordinal = self
            .positions
            .iter()
            .position(|actual| *actual == path)
            .ok_or("original field ordinal")?;
        assert!(
            proof::uses(self.catalog, cell)?.iter().any(|usage| usage
                .get("role")
                .and_then(Value::as_str)
                == Some(role)
                && usage.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeValue")
                && usage.pointer("/location/indices") == Some(&json!(["0", ordinal.to_string()]))
                && usage.pointer("/location/field").and_then(Value::as_str) == Some(field)),
            "the cell binds its own physical source and role"
        );
        Ok(())
    }
}

fn exact_number(original: &Original<'_>) -> TestResult<Value> {
    let (kind, bits, finite) = match original {
        Original::Single(value) => (
            "f32",
            format!("{:08x}", value.to_bits()),
            serde_json::from_slice::<Value>(&serde_json::to_vec(value)?)?,
        ),
        Original::Double(value) => (
            "f64",
            format!("{:016x}", value.to_bits()),
            serde_json::from_slice::<Value>(&serde_json::to_vec(value)?)?,
        ),
        _ => return Err("expected synthetic floating number".into()),
    };
    Ok(json!({"type":kind, "rawBits":bits, "finite":finite}))
}

fn authored_payload<'a>(arena: &'a Value, name: &str) -> TestResult<&'a Value> {
    proof::at(
        proof::cell(authored_node(arena, name)?, "value", "present")?,
        "/value",
    )
}

fn authored_node<'a>(arena: &'a Value, name: &str) -> TestResult<&'a Value> {
    let field = proof::rows(arena, "/roots")?
        .iter()
        .find(|root| root.pointer("/name/value").and_then(Value::as_str) == Some(name))
        .ok_or("original authored arena root")?;
    arena_node(arena, proof::at(field, "/value")?)
}

fn arena_node<'a>(arena: &'a Value, reference: &Value) -> TestResult<&'a Value> {
    let index = proof::at(reference, "/valueIndex")?
        .as_str()
        .ok_or("original local index")?
        .parse::<usize>()?;
    proof::rows(arena, "/values")?
        .get(index)
        .ok_or_else(|| "original arena node".into())
}

fn field_position(owner: &StructValue<'_>, name: &str) -> TestResult<usize> {
    owner
        .fields()
        .iter()
        .position(|field| field.name() == name)
        .ok_or_else(|| "original field declaration".into())
}

fn extend(path: &[Step], step: Step) -> Vec<Step> {
    let mut child = path.to_vec();
    child.push(step);
    child
}

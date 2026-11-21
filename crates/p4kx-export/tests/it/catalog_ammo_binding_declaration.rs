// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{DataForge, DataType, Value as Original};
use serde_json::{Value, json};

use super::{bindings, common, fixture, proof, slots};
use crate::commodity_v3_fixture::{DcbOrigin, TestResult};

#[test]
fn ammo_container_bindings_reject_references_declared_for_another_family() -> TestResult {
    let fixture = fixture::database(fixture::Case::WrongDeclaration);
    let declared_target = original_declared_target(&fixture.build())?;
    let (_folder, archive) = common::open(&fixture, DcbOrigin::Encrypted)?;
    assert!(
        matches!(
            build_catalog(&archive, &common::options()),
            Err(CatalogError::IncompleteExport(_))
        ),
        "an incompatible source declaration cannot publish a ready Ammo slot"
    );
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let catalog: Value = serde_json::from_slice(build_catalog(&archive, &options)?.json())?;
    let [row, _] = bindings(&catalog)? else {
        return Err("both original container rows".into());
    };
    let binding = slots(row, 1, true)?;
    for role in ["primaryAmmo", "secondaryAmmo"] {
        let cell = proof::cell(binding, role, "unsupported")?;
        assert_eq!(
            cell.get("sourceSetComplete"),
            Some(&json!(true)),
            "the wrong target declaration is fully readable"
        );
        assert_eq!(
            cell.get("containsEncryptedSource"),
            Some(&json!(true)),
            "the decision uses the actual encrypted DCB"
        );
        check_declaration(&catalog, cell, declared_target)?;
    }
    proof::closure(&catalog)
}

fn original_declared_target(bytes: &[u8]) -> TestResult<u16> {
    let forge = DataForge::open(bytes)?;
    let owner = forge.record(2).ok_or("original container owner")?.tree()?;
    let Some(Original::Array(components)) = owner.field("Components") else {
        return Err("original component slots".into());
    };
    let [_, Original::Struct(container), _] = components.as_ref() else {
        return Err("original first container occurrence".into());
    };
    let mut declared = None;
    for name in ["ammoParamsRecord", "secondaryAmmoParamsRecord"] {
        let field = container
            .fields()
            .iter()
            .find(|field| field.name() == name)
            .ok_or("the actual named reference declaration")?;
        assert_eq!(
            field.storage().data_type(),
            DataType::Reference,
            "both controls are true References"
        );
        let Original::Reference(target) = field.value() else {
            return Err("the original wrong declaration resolves a real Record".into());
        };
        assert_eq!(
            target.record_index(),
            2,
            "the Reference points to its actual declared family"
        );
        let index = field.storage().target();
        let structure = forge
            .structs()
            .nth(usize::from(index))
            .ok_or("stored target declaration")?;
        assert_eq!(
            structure.name()?,
            "EntityClassDefinition",
            "the declared target name is readable"
        );
        declared = Some(index);
    }
    declared.ok_or_else(|| "the observed target index".into())
}

fn check_declaration(catalog: &Value, cell: &Value, index: u16) -> TestResult {
    let uses = proof::uses(catalog, cell)?;
    let target = json!([index.to_string()]);
    assert!(
        uses.iter().any(|used| {
            used.get("role").and_then(Value::as_str) == Some("selector")
                && used.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("dataforgeStruct")
                && used.pointer("/location/field").and_then(Value::as_str) == Some("name")
                && used.pointer("/location/indices") == Some(&target)
        }),
        "the exact original declared Struct name proves the mismatch"
    );
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;

use p4kx_extraction::{
    FieldOrigin, MaterialSummaryDetailOrigin, SourceInventory, SourceLocation,
    SourceUseDescription, SourceUseRole,
};
use p4kx_gui::{Language, MaterialsInfo, ModelPreview, Unavailable, ViewerSupport};
use p4kx_testkit::{TestEntry, TestModel, TestMtl, TestMtlMaterial};
use serde_json::Value;

use crate::viewing::{Site, TestResult, model_preview, opened_at, select};

const COUNTED_LIBRARY: &str = "Data/Objects/counted.mtl";
const SINGLE_LIBRARY: &str = "Data/Objects/single.mtl";
const UNREADABLE_LIBRARY: &str = "Data/Objects/unreadable.mtl";

#[test]
fn loaded_material_details_use_the_selected_library_and_exact_summary_site() -> TestResult {
    let mut entries = Vec::from(model_entries("counted"));
    let [primary, geometry] = model_entries("single");
    entries.extend([primary.encrypted(), geometry]);
    let counted = TestMtl::multi(&[TestMtlMaterial::new("one"), TestMtlMaterial::new("two")]);
    let single = TestMtl::single(&TestMtlMaterial::new("only"));
    entries.extend([
        TestEntry::zstd(COUNTED_LIBRARY, counted.cryxml()).encrypted(),
        TestEntry::zstd("Data/counted.mtl", single.cryxml()),
        TestEntry::zstd(SINGLE_LIBRARY, single.cryxml()),
    ]);
    let site = Site {
        entries,
        installation: false,
        key: true,
    };
    let (_folder, mut harness) = opened_at(
        Language::BritishEnglish,
        ViewerSupport::Unavailable(Unavailable::NoDevice),
        site,
        |builder| builder,
    )?;
    for (stem, library, single) in [
        ("counted", COUNTED_LIBRARY, false),
        ("single", SINGLE_LIBRARY, true),
    ] {
        select(&mut harness, &format!("{stem}.cgf"))?;
        let preview = model_preview(harness.state()).ok_or("model preview")?;
        assert_loaded(preview, stem, library, single)?;
    }
    Ok(())
}

fn assert_loaded(preview: &ModelPreview, stem: &str, library: &str, single: bool) -> TestResult {
    let info = preview.info().ok_or("model info")?;
    assert_eq!(
        info.materials,
        MaterialsInfo::Loaded {
            materials: if single { 1 } else { 2 },
            single,
        },
        "the displayed summary follows the selected library"
    );
    let proof = preview
        .info_provenance()
        .ok_or("material provenance")?
        .map_err(|error| format!("material provenance: {error}"))?;
    let library_proof = proof.material_library();
    let inventory = proof.inventory();
    let primary_path = format!("Data/Objects/{stem}.cgf");
    let path = library_proof.path.as_ref().ok_or("decoded library name")?;
    assert_sources(path, inventory, &[&primary_path])?;
    assert_eq!(
        path.contains_encrypted_source(),
        single.then_some(true),
        "only the primary Entry supplies the displayed name"
    );
    assert_sources(&library_proof.status, inventory, &[&primary_path, library])?;
    assert_eq!(
        library_proof.status.contains_encrypted_source(),
        Some(true),
        "selector and loaded library both contribute to the status"
    );
    let (detail, field) = match (single, &library_proof.detail) {
        (true, Some(MaterialSummaryDetailOrigin::Single(origin))) => (origin, "SubMaterials"),
        (false, Some(MaterialSummaryDetailOrigin::Count(origin))) => (origin, "materialSlots"),
        _ => return Err("wrong material detail shown".into()),
    };
    assert_sources(detail, inventory, &[&primary_path, library])?;
    assert_summary_site(detail, inventory, library, field)?;
    Ok(())
}

#[test]
fn unavailable_material_details_retain_selectors_without_loaded_inputs() -> TestResult {
    let mut entries = Vec::from(model_entries("missing"));
    entries.extend(model_entries("unreadable"));
    entries.push(TestEntry::zstd(UNREADABLE_LIBRARY, b"invalid".to_vec()).encrypted());
    let site = Site {
        entries,
        installation: false,
        key: true,
    };
    let (_folder, mut harness) = opened_at(
        Language::BritishEnglish,
        ViewerSupport::Unavailable(Unavailable::NoDevice),
        site,
        |builder| builder,
    )?;
    for stem in ["missing", "unreadable"] {
        select(&mut harness, &format!("{stem}.cgf"))?;
        let preview = model_preview(harness.state()).ok_or("model preview")?;
        assert_unavailable(preview, stem)?;
    }
    Ok(())
}

fn assert_unavailable(preview: &ModelPreview, stem: &str) -> TestResult {
    let info = preview.info().ok_or("model info")?;
    assert!(
        matches!(
            (stem, &info.materials),
            ("missing", MaterialsInfo::Missing) | ("unreadable", MaterialsInfo::Unreadable(_))
        ),
        "missing and broken libraries remain distinguishable"
    );
    let proof = preview
        .info_provenance()
        .ok_or("material provenance")?
        .map_err(|error| format!("material provenance: {error}"))?;
    let library = proof.material_library();
    let path = library.path.as_ref().ok_or("known declaration")?;
    let primary = format!("Data/Objects/{stem}.cgf");
    assert_sources(path, proof.inventory(), &[&primary])?;
    assert_sources(&library.status, proof.inventory(), &[&primary])?;
    assert!(library.detail.is_none(), "no loaded count or single value");
    assert_eq!(
        library.status.contains_encrypted_source(),
        None,
        "unobserved or unsuccessful library data does not contribute"
    );
    let attempts = proof
        .inventory()
        .iter_uses()
        .filter(|used| used.role() == SourceUseRole::Attempted)
        .map(|used| physical_path(proof.inventory(), used));
    let attempts: Result<Vec<_>, _> = attempts.collect();
    assert_eq!(
        attempts?,
        if stem == "unreadable" {
            vec![UNREADABLE_LIBRARY.to_owned()]
        } else {
            Vec::new()
        },
        "only the original file that was found and tried is recorded as attempted"
    );
    Ok(())
}

fn model_entries(stem: &str) -> [TestEntry; 2] {
    let model = TestModel::cube().with_material_library(stem);
    [
        TestEntry::zstd(format!("Data/Objects/{stem}.cgf"), model.cgf()),
        TestEntry::zstd(format!("Data/Objects/{stem}.cgfm"), model.cgfm()),
    ]
}

fn assert_sources(origin: &FieldOrigin, inventory: &SourceInventory, paths: &[&str]) -> TestResult {
    let mut sources = BTreeSet::new();
    for reference in origin.source_uses_refs() {
        let used = inventory
            .iter_uses()
            .find(|used| used.id() == reference)
            .ok_or("original source use")?;
        sources.insert(physical_path(inventory, used)?);
    }
    assert_eq!(
        sources,
        paths.iter().map(|path| (*path).to_owned()).collect(),
        "only the selected physical inputs contribute"
    );
    assert!(
        !origin.source_set_complete(),
        "the library lookup decisions were not observed"
    );
    Ok(())
}

fn assert_summary_site(
    origin: &FieldOrigin,
    inventory: &SourceInventory,
    path: &str,
    field: &str,
) -> TestResult {
    let mut found = false;
    for used in inventory.iter_uses() {
        if let SourceLocation::Subresource {
            namespace,
            field: observed,
            ..
        } = used.location()
        {
            assert_ne!(namespace, "materialXmlAttribute", "no authored attributes");
            if namespace == "materialXmlElement" {
                assert_eq!(
                    observed, field,
                    "only the displayed detail has XML coordinates"
                );
                assert_eq!(
                    physical_path(inventory, used)?,
                    path,
                    "selected library Entry"
                );
                assert!(
                    origin.source_uses_refs().contains(used.id()),
                    "the displayed detail points to that XML source"
                );
                found = true;
            }
        }
    }
    assert!(found, "original material detail site");
    Ok(())
}

fn physical_path(
    inventory: &SourceInventory,
    used: &SourceUseDescription,
) -> Result<String, Box<dyn Error>> {
    let source = inventory
        .iter_sources()
        .find(|source| source.id() == used.source())
        .ok_or("physical source")?;
    Ok(serde_json::to_value(source)?
        .pointer("/path/value")
        .and_then(Value::as_str)
        .ok_or("physical Entry path")?
        .to_owned())
}

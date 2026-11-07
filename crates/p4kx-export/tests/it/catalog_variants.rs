// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The variant groups of ships in the Basetool catalog, through the public API.

use crate::variants_fixture;

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{BuiltCatalog, CatalogOptions, VariantCounts, build_catalog};
use p4kx_extraction::Archive;
use serde_json::Value;

type TestResult = Result<(), Box<dyn Error>>;

const EXPORTED_AT: Duration = Duration::from_secs(1_792_000_000);

fn build() -> Result<(BuiltCatalog, Value), Box<dyn Error>> {
    let dir = tempfile::tempdir()?;
    let archive = Archive::open_with_key(
        variants_fixture::write_archive(dir.path())?,
        Some(p4kx_testkit::test_key()),
    )?;
    let built = build_catalog(
        &archive,
        &CatalogOptions {
            publication_mode: p4kx_export::CatalogPublicationMode::DiagnosticPartial,
            ..CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT)
        },
    )?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

fn entry<'c>(catalog: &'c Value, kind: &str, class: &str) -> Result<&'c Value, String> {
    catalog[kind]
        .as_array()
        .and_then(|list| list.iter().find(|entry| entry["className"] == class))
        .ok_or_else(|| format!("{kind} {class} missing"))
}

fn base_of(catalog: &Value, kind: &str, class: &str) -> Result<Option<String>, String> {
    let base = &entry(catalog, kind, class)?["variantOf"];
    if base.is_null() {
        return Ok(None);
    }
    catalog[kind]
        .as_array()
        .and_then(|list| list.iter().find(|entry| entry["guid"] == *base))
        .and_then(|entry| entry["className"].as_str())
        .map(|name| Some(name.to_owned()))
        .ok_or_else(|| format!("variantOf of {class} resolves to no {kind} entry"))
}

fn base_class(catalog: &Value, class: &str) -> Result<Option<String>, String> {
    base_of(catalog, "ships", class)
}

#[test]
fn items_that_differ_only_in_tags_and_name_are_variants() -> TestResult {
    let (_, catalog) = build()?;
    for (class, base) in [
        ("POWR_Alpha", None),
        ("POWR_Alpha_Blue", Some("POWR_Alpha")),
    ] {
        assert_eq!(
            base_of(&catalog, "items", class)?.as_deref(),
            base,
            "{class}"
        );
    }
    Ok(())
}

#[test]
fn every_left_out_component_and_field_alone_keeps_items_in_one_group() -> TestResult {
    let (_, catalog) = build()?;
    for stem in [
        "POWR_Tags",
        "POWR_Carry",
        "POWR_Interact",
        "POWR_State",
        "POWR_Usable",
        "POWR_Open",
        "POWR_Loot",
        "PROP_Material",
        "PROP_Materials",
        "PROP_Palette",
    ] {
        let base = format!("{stem}_A");
        assert_eq!(base_of(&catalog, "items", &base)?, None, "{base}");
        assert_eq!(
            base_of(&catalog, "items", &format!("{stem}_B"))?.as_deref(),
            Some(base.as_str()),
            "{stem}_B"
        );
    }
    Ok(())
}

#[test]
fn items_that_differ_in_a_component_or_the_grade_are_not_variants() -> TestResult {
    let (_, catalog) = build()?;
    for class in ["POWR_Heavy", "POWR_Grade2"] {
        assert_eq!(base_of(&catalog, "items", class)?, None, "{class}");
    }
    Ok(())
}

#[test]
fn items_without_a_component_besides_the_cosmetic_ones_are_ungrouped() -> TestResult {
    let (built, catalog) = build()?;
    for class in ["POWR_Bare_One", "POWR_Bare_Two"] {
        assert_eq!(base_of(&catalog, "items", class)?, None, "{class}");
    }
    assert_eq!(
        built.summary().variants().items,
        VariantCounts {
            groups: 12,
            variants: 12
        }
    );
    Ok(())
}

#[test]
fn items_that_differ_only_in_material_and_palette_are_variants() -> TestResult {
    let (_, catalog) = build()?;
    assert_eq!(base_of(&catalog, "items", "PROP_Chair_Blue")?, None);
    assert_eq!(
        base_of(&catalog, "items", "PROP_Chair_Red")?.as_deref(),
        Some("PROP_Chair_Blue")
    );
    Ok(())
}

#[test]
fn items_that_differ_in_their_model_are_not_variants() -> TestResult {
    let (_, catalog) = build()?;
    for class in ["PROP_Table", "PROP_Bench"] {
        assert_eq!(base_of(&catalog, "items", class)?, None, "{class}");
    }
    Ok(())
}

#[test]
fn ships_with_the_same_definition_and_implementation_are_variants_of_the_smallest_class_name()
-> TestResult {
    let (_, catalog) = build()?;
    for (class, base) in [
        ("ACAS_Alpha", None),
        ("ACAS_Alpha_Edition", Some("ACAS_Alpha")),
        ("ACAS_Alpha_Zulu", Some("ACAS_Alpha")),
        ("ACAS_Alpha_Plated", Some("ACAS_Alpha")),
        ("ACAS_Alpha_Skin", Some("ACAS_Alpha")),
        ("ACAS_Alpha_Case", Some("ACAS_Alpha")),
    ] {
        assert_eq!(base_class(&catalog, class)?.as_deref(), base, "{class}");
    }
    Ok(())
}

#[test]
fn a_modification_that_changes_the_implementation_starts_a_group_of_its_own() -> TestResult {
    let (_, catalog) = build()?;
    for (class, base) in [
        ("ACAS_Beta", None),
        ("ACAS_Beta_Edition", Some("ACAS_Beta")),
    ] {
        assert_eq!(base_class(&catalog, class)?.as_deref(), base, "{class}");
    }
    Ok(())
}

#[test]
fn an_elem_override_of_a_nested_element_or_a_vehicle_child_splits_the_group() -> TestResult {
    let (_, catalog) = build()?;
    for (class, base) in [
        ("ACAS_Aim", None),
        ("ACAS_Aim_Edition", Some("ACAS_Aim")),
        ("ACAS_Move", None),
        ("ACAS_Move_Edition", Some("ACAS_Move")),
    ] {
        assert_eq!(base_class(&catalog, class)?.as_deref(), base, "{class}");
    }
    Ok(())
}

#[test]
fn a_ship_that_differs_in_definition_or_modification_is_not_a_variant() -> TestResult {
    let (_, catalog) = build()?;
    for class in ["ACAS_Other", "ACAS_Solo"] {
        assert_eq!(base_class(&catalog, class)?, None, "{class}");
    }
    Ok(())
}

#[test]
fn a_ship_without_a_readable_implementation_is_ungrouped() -> TestResult {
    let (_, catalog) = build()?;
    for class in ["ACAS_Lonely", "ACAS_Broken", "ACAS_Missing"] {
        assert_eq!(base_class(&catalog, class)?, None, "{class}");
    }
    Ok(())
}

#[test]
fn every_ship_carries_the_field_and_it_is_a_guid_or_null() -> TestResult {
    let (_, catalog) = build()?;
    let ships = catalog["ships"].as_array().ok_or("ships")?;
    assert_eq!(ships.len(), 17);
    for ship in ships {
        let value = ship.get("variantOf").ok_or("variantOf missing")?;
        assert!(
            value.is_null() || value.is_string(),
            "{}",
            ship["className"]
        );
    }
    Ok(())
}

#[test]
fn the_summary_counts_groups_and_variants() -> TestResult {
    let (built, _) = build()?;
    assert_eq!(
        built.summary().variants().ships,
        VariantCounts {
            groups: 4,
            variants: 8
        }
    );
    Ok(())
}

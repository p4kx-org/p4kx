// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The variant groups of ships in the P4KX catalog, through the public API.

#[path = "../catalog_variant_value/mod.rs"]
pub(crate) mod decision_reader;

use crate::variants_fixture;
use crate::variants_fixture::read_rows::{base_of, entry};

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{
    BuiltCatalog, CatalogOptions, CatalogPublicationMode, VariantCounts, build_catalog,
};
use p4kx_extraction::{Archive, DataForgeEntry, Value as OriginalValue, ValueError};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin};
use crate::health_origin_assertions as proof;

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
        CatalogOptions {
            publication_mode: CatalogPublicationMode::DiagnosticPartial,
            ..CatalogOptions::new(UNIX_EPOCH + EXPORTED_AT)
        }
        .with_full_provenance(),
    )?;
    let json = serde_json::from_slice(built.json())?;
    Ok((built, json))
}

fn base_class(catalog: &Value, class: &str) -> Result<Option<String>, String> {
    base_of(catalog, "ships", class)
}

#[path = "catalog_ship_variants_filter.rs"]
mod filtered_ships;

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
        let row = entry(&catalog, "ships", class)?;
        let state = if class == "ACAS_Broken" {
            "unreadable"
        } else {
            "absent"
        };
        decision_reader::variant_cell(row, state)?;
    }
    Ok(())
}

#[test]
fn every_ship_carries_a_typed_variant_decision() -> TestResult {
    let (_, catalog) = build()?;
    let ships = catalog["ships"].as_array().ok_or("ships")?;
    assert_eq!(ships.len(), 17, "all original Ship rows survive");
    for ship in ships {
        let value = ship.get("variantOf").ok_or("variantOf missing")?;
        assert!(
            matches!(
                value.get("state").and_then(Value::as_str),
                Some("present" | "absent" | "unreadable")
            ),
            "{} retains a typed physical variant outcome",
            ship["className"]["value"]
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

fn check_readable_item_keys(catalog: &Value) -> TestResult {
    for class in ["POWR_Key_Empty", "POWR_Key_Excluded"] {
        assert_eq!(
            base_of(catalog, "items", class)?.as_deref(),
            Some("POWR_Key_Base"),
            "{class} keeps its proven variant link"
        );
        let components = item_components(catalog, class)?;
        assert_eq!(
            proof::at(components, "/sourceSetComplete")?.as_bool(),
            Some(true),
            "{class} retains a complete source survey"
        );
    }
    Ok(())
}

fn item_components<'a>(catalog: &'a Value, class: &str) -> Result<&'a Value, Box<dyn Error>> {
    let item = entry(catalog, "items", class)?;
    let index = proof::at(item, "/recordIndex")?;
    let definition = proof::rows(catalog, "/definitions/items/rows")?
        .iter()
        .find(|definition| definition.get("recordIndex") == Some(index))
        .ok_or("the same physical Item definition")?;
    proof::cell(definition, "components", "present")
}

#[test]
fn item_components_with_null_or_excluded_values_keep_the_normal_variant_key() -> TestResult {
    let (_, catalog) = common::build(&variants_fixture::item_key_database(false))?;
    check_readable_item_keys(&catalog)
}

#[test]
fn an_unreadable_item_component_cannot_prove_a_variant_link() -> TestResult {
    let database = variants_fixture::item_key_database(true);
    let (_folder, archive) = common::open(&database, DcbOrigin::Encrypted)?;
    let ordinal = unreadable_component_ordinal(&archive)?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, options.with_full_provenance())?;
    let catalog = serde_json::from_slice(built.json())?;
    let class = "POWR_Key_Unreadable";
    for candidate in [
        "POWR_Key_Base",
        "POWR_Key_Empty",
        "POWR_Key_Excluded",
        class,
    ] {
        let decision =
            decision_reader::variant_cell(entry(&catalog, "items", candidate)?, "unreadable")?;
        assert_eq!(
            decision.get("sourceSetComplete").and_then(Value::as_bool),
            Some(false),
            "{candidate} cannot prove a winner while a possible peer remains unreadable"
        );
    }
    let components = item_components(&catalog, class)?;
    verify_component_attempt(&catalog, components, ordinal)
}

const UNREADABLE_ITEM_INDEX: u32 = 3;

fn unreadable_component_ordinal(archive: &Archive) -> Result<usize, Box<dyn Error>> {
    let source = DataForgeEntry::read(archive)?;
    let forge = source.open()?;
    let tree = forge
        .record(UNREADABLE_ITEM_INDEX)
        .ok_or("actual unreadable Item Record")?
        .tree()?;
    let Some(OriginalValue::Array(slots)) = tree.field("Components") else {
        return Err("actual original Components array".into());
    };
    assert!(
        matches!(
            slots.last(),
            Some(OriginalValue::Error(ValueError::InvalidPointer {
                struct_index: variants_fixture::INVALID_COMPONENT_TYPE,
                instance_index: 3,
            }))
        ),
        "the final source slot has an actual invalid pointer"
    );

    let mut ordinals = Vec::new();
    tree.root().visit_owned_values(|visit| {
        if visit.field_name() == "Components"
            && matches!(
                visit.value(),
                OriginalValue::Error(ValueError::InvalidPointer { .. })
            )
        {
            ordinals.push(visit.ordinal());
        }
    });
    let [ordinal] = ordinals.as_slice() else {
        return Err("one unreadable occurrence in the original Components array".into());
    };
    Ok(*ordinal)
}

fn verify_component_attempt(catalog: &Value, components: &Value, ordinal: usize) -> TestResult {
    assert_eq!(
        proof::at(components, "/sourceSetComplete")?.as_bool(),
        Some(false),
        "the original unavailable component retains its incomplete proof"
    );
    assert!(
        proof::uses(catalog, components)?
            .iter()
            .all(|used| used.get("role").and_then(Value::as_str) != Some("attempted")),
        "the failed read does not contribute a decoded component value"
    );
    let coordinate = json!([UNREADABLE_ITEM_INDEX.to_string(), ordinal.to_string()]);
    assert!(
        proof::rows(catalog, "/provenance/uses")?
            .iter()
            .any(|used| {
                used.get("role").and_then(Value::as_str) == Some("attempted")
                    && used.get("scope").and_then(Value::as_str) == Some("payload")
                    && used.pointer("/location/namespace").and_then(Value::as_str)
                        == Some("dataforgeValue")
                    && used.pointer("/location/field").and_then(Value::as_str) == Some("$value")
                    && used.pointer("/location/indices") == Some(&coordinate)
            }),
        "the inventory retains the attempted read at its exact original physical Component slot"
    );
    proof::closure(catalog)
}

#[path = "catalog_variant_identity_fixture.rs"]
mod physical_fixture;
#[path = "catalog_variant_physical_identity.rs"]
mod physical_identity;

#[path = "catalog_variants_filter.rs"]
mod filtered_items;

#[path = "catalog_item_variant_headers.rs"]
mod item_headers;

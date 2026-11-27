// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{ArchiveBuilder, GlobalIniBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use super::ReducedError;
use super::record_targets::RecordTargets;
use super::selected_texts::SelectedTexts;
use super::starmap_definitions;
use super::starmap_objects::{self, Dependencies, PATH};
use crate::build::original_input::test_support::TestResult;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession, ParsedCatalogQueries};

#[path = "starmap_objects_fixture.rs"]
mod fixture;

#[test]
fn reduced_starmap_objects_close_physical_links_and_only_selected_text() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let archive = archive(scratch.path())?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
        None,
    )?;
    validate(&session.parsed(None)?, scratch.path())
}

fn validate(parsed: &ParsedCatalogQueries<'_>, scratch: &Path) -> TestResult {
    let mut texts = SelectedTexts::default();
    let types = starmap_definitions::collect_types(parsed, scratch, None, &mut texts)?;
    let amenities = starmap_definitions::collect_amenities(parsed, scratch, None)?;
    let dependencies = Dependencies {
        types: &types.targets,
        amenities: &amenities.targets,
    };
    let staged = starmap_objects::collect(parsed, &dependencies, scratch, None, &mut texts)?;
    let wire = serde_json::to_value(staged.table)?;
    let Some([child, parent]) = wire
        .get("rows")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    else {
        return Err("the child and later physical parent".into());
    };
    verify_links(child, parent)?;
    verify_numbers(child, parent)?;
    verify_texts(child, parent, &serde_json::to_value(texts)?)?;
    let parent_guid = parent
        .pointer("/guid/value")
        .and_then(Value::as_str)
        .ok_or("parent GUID")?
        .parse()?;
    staged.targets.verify(PATH, 5, parent_guid)?;

    let physically_missing = Dependencies {
        amenities: &RecordTargets::default(),
        ..dependencies
    };
    assert!(
        matches!(
            starmap_objects::collect(
                parsed,
                &physically_missing,
                scratch,
                None,
                &mut SelectedTexts::default()
            ),
            Err(ReducedError::UnresolvedRecordLink {
                path: PATH,
                index: 2
            })
        ),
        "a resolved Amenity must exist in the staged matching family"
    );
    Ok(())
}

fn verify_links(child: &Value, parent: &Value) -> TestResult {
    for (field, index, kind) in [
        ("type", 1, "StarMapObjectType"),
        ("parent", 5, "StarMapObject"),
        ("jurisdiction", 3, "Jurisdiction"),
        ("affiliation", 4, "Faction_LEGACY"),
    ] {
        let reference = child
            .pointer(&format!("/{field}/value"))
            .ok_or("original link")?;
        assert_eq!(
            reference.get("status"),
            Some(&json!("present")),
            "resolved {field}"
        );
        assert_eq!(
            reference.pointer("/targetType/value"),
            Some(&json!(kind)),
            "actual target type for {field}"
        );
        assert_eq!(
            reference.pointer("/targetRecordIndex/value"),
            Some(&json!(index)),
            "original physical target for {field}"
        );
    }
    assert_eq!(
        child.pointer("/parent/value/guid/value"),
        parent.pointer("/guid/value"),
        "the later Object parent belongs to this same result"
    );
    let Some([first, null, repeated]) = child
        .pointer("/amenities/value")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
    else {
        return Err("all ordered original Amenity slots".into());
    };
    assert_eq!(
        first.pointer("/value/targetRecordIndex/value"),
        Some(&json!(2)),
        "exact Amenity Record"
    );
    assert_eq!(
        repeated, first,
        "duplicate Amenity values retain separate slots"
    );
    assert_eq!(
        null.get("state"),
        Some(&json!("null")),
        "explicit empty Amenity remains null"
    );
    Ok(())
}

fn verify_numbers(child: &Value, parent: &Value) -> TestResult {
    let size = child.pointer("/size/value").ok_or("original f64 size")?;
    assert_eq!(size.get("type"), Some(&json!("f64")), "double storage");
    let finite = size.get("finite").ok_or("finite size")?;
    assert_eq!(serde_json::to_string(finite)?, "-0.0", "double signed zero");
    assert_eq!(
        parent.pointer("/size/value"),
        Some(&json!({"type":"f64","finite":null,"nonFinite":"negativeInfinity"})),
        "nonfinite size remains distinct from null state"
    );
    verify_travel_radii(child, parent)?;
    for (field, expected) in [
        ("hideInStarmap", true),
        ("hideInWorld", false),
        ("blockTravel", true),
    ] {
        let cell = child.get(field).ok_or("visibility or travel policy")?;
        assert_eq!(
            cell.get("value"),
            Some(&json!(expected)),
            "authored {field}"
        );
    }
    assert_eq!(
        child.pointer("/hideWhenInAdoptionRadius/state"),
        Some(&json!("absent")),
        "undeclared visibility flag"
    );

    assert!(
        child.get("minimumDisplaySize").is_none(),
        "only the selected object size"
    );
    assert!(
        child.get("authoredFields").is_none(),
        "typed selected fields require no raw arena"
    );
    Ok(())
}

fn verify_travel_radii(child: &Value, parent: &Value) -> TestResult {
    let travel = child
        .pointer("/quantumTravelData/value")
        .ok_or("original travel declaration")?;
    for (field, expected) in [
        ("obstructionRadius", json!({"type":"f32","finite":20.5})),
        ("arrivalRadius", json!({"type":"f32","finite":5.0})),
        (
            "adoptionRadius",
            json!({"type":"f32","finite":null,"nonFinite":"positiveInfinity"}),
        ),
    ] {
        let cell = travel.get(field).ok_or("original radius")?;
        assert_eq!(cell.get("value"), Some(&expected), "exact {field}");
        assert_eq!(
            cell.get("containsEncryptedSource"),
            Some(&json!(true)),
            "original {field} marker"
        );
    }
    assert_eq!(
        parent.pointer("/quantumTravelData/state"),
        Some(&json!("null")),
        "original null travel declaration"
    );
    assert!(
        travel.get("arrivalPointDetectionOffset").is_none(),
        "radius output excludes unrelated offset"
    );
    Ok(())
}

fn verify_texts(child: &Value, parent: &Value, texts: &Value) -> TestResult {
    let selected = texts
        .get("gameTexts")
        .and_then(Value::as_array)
        .ok_or("protected text closure")?;
    assert_eq!(
        selected.len(),
        3,
        "child name and description plus parent name"
    );
    for (row, path, en, de) in [
        (child, "/name/value", "Child", "Kind"),
        (child, "/description/value", "Description", "Beschreibung"),
        (parent, "/name/value", "Parent", "Elternobjekt"),
    ] {
        let key = row.pointer(path).ok_or("retained exact reference")?;
        let text = selected
            .iter()
            .find(|text| text.pointer("/id/value") == Some(key))
            .ok_or("referenced protected text")?;
        for (language, expected) in [("en", en), ("de", de)] {
            assert_eq!(
                text.pointer(&format!("/translations/value/{language}/displayText/value")),
                Some(&json!(expected)),
                "{path} exact {language} text"
            );
        }
    }
    assert!(
        child.get("callout1").is_none(),
        "callouts are outside the selected object text"
    );
    assert_eq!(
        texts.get("publicNames"),
        Some(&json!([])),
        "object labels retain their protected domain"
    );
    Ok(())
}

fn archive(scratch: &Path) -> TestResult<Archive> {
    let path = scratch.join("Data.p4k");
    let locale = |child, parent, description| {
        GlobalIniBuilder::new()
            .text("fixture_object_child", child)
            .text("fixture_object_parent", parent)
            .text("fixture_object_description", description)
            .text("fixture_unused_object_callout", "Unused")
            .build()
    };
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", fixture::database().build()).encrypted())
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            locale("Child", "Parent", "Description"),
        ))
        .entry(TestEntry::stored(
            Language::German.global_ini_path(),
            locale("Kind", "Elternobjekt", "Beschreibung"),
        ))
        .write_to(&path)?;
    Ok(Archive::open_with_key(path, Some(test_key()))?)
}

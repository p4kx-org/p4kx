// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, test_key};
use serde_json::Value;

use super::resource_types;
use super::selected_texts::SelectedTexts;
use crate::build::api::{OverrideEntryTarget, resource_fixture};
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn resource_types_retain_units_quality_and_member_links() -> TestResult {
    for target in [OverrideEntryTarget::Base, OverrideEntryTarget::Concrete] {
        verify(target)?;
    }
    Ok(())
}

fn verify(target: OverrideEntryTarget) -> TestResult {
    let database = resource_fixture::database(target);
    let (scratch, archive) = open_fixture(&database)?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
        None,
    )?;
    let mut texts = SelectedTexts::default();
    let result = resource_types::collect(&session.parsed(None)?, scratch.path(), None, &mut texts)?;
    let resources = serde_json::to_value(result.definitions)?;
    let groups = serde_json::to_value(result.groups.members)?;
    let globals = serde_json::to_value(result.crafting_global)?;
    let components = serde_json::to_value(result.components)?;
    assert_eq!(
        components.get("availability").and_then(Value::as_str),
        Some("absent"),
        "absence of a resource component declaration preserves the selected empty table"
    );
    assert_eq!(
        globals.get("availability").and_then(Value::as_str),
        Some("absent")
    );
    let rows = resources
        .get("rows")
        .and_then(Value::as_array)
        .ok_or("ResourceType rows")?;
    let [alpha, beta, unused] = rows.as_slice() else {
        return Err("three canonical ResourceTypes".into());
    };

    assert_density_cells(alpha, beta, unused)?;
    assert_selected_properties(alpha, beta)?;
    assert!(
        groups
            .get("rows")
            .and_then(Value::as_array)
            .is_some_and(|groups| groups.len() == 2)
    );

    let wire = serde_json::to_string(&resources)?;
    assert!(!wire.contains("sourceUsesRefs"));
    assert!(!wire.contains("sourceSetComplete"));
    assert!(!wire.contains("rawBits"));
    assert!(!wire.contains("defaultThumbnailPath"));
    assert!(!wire.contains("description"));
    assert_selected_names(&serde_json::to_value(texts)?);
    Ok(())
}

fn assert_density_cells(alpha: &Value, beta: &Value, unused: &Value) -> TestResult {
    for (row, expected_kind, expected_value) in [
        (alpha, "gramsPerCubicCentimeter", 3.25),
        (beta, "kilogramsPerCubicMeter", 12.5),
    ] {
        let unit = row
            .pointer("/density/value/payload/value/densityUnit/value/payload/value")
            .ok_or("selected density unit")?;
        assert_eq!(
            unit.get("type").and_then(Value::as_str),
            Some(expected_kind)
        );
        assert_eq!(
            unit.pointer("/value/value/finite").and_then(Value::as_f64),
            Some(expected_value)
        );
        assert_eq!(
            unit.pointer("/value/containsEncryptedSource")
                .and_then(Value::as_bool),
            Some(true)
        );
    }
    assert_eq!(
        unused
            .pointer("/density/value/payload/value/molarMass/value/finite")
            .and_then(Value::as_f64),
        Some(6.75)
    );
    Ok(())
}

fn assert_selected_properties(alpha: &Value, beta: &Value) -> TestResult {
    assert_eq!(
        alpha
            .pointer("/refinedVersion/value/targetRecordIndex/value")
            .and_then(Value::as_u64),
        beta.pointer("/recordIndex/value").and_then(Value::as_u64)
    );
    assert_eq!(
        alpha
            .pointer("/properties/value/0/value/payload/value/volatility/value/finite")
            .and_then(Value::as_f64),
        Some(0.0)
    );
    assert_eq!(
        alpha
            .pointer("/properties/value/2/state")
            .and_then(Value::as_str),
        Some("null")
    );
    let crafting = alpha
        .pointer("/properties/value/1/value/payload/value")
        .ok_or("authored crafting data")?;
    assert_eq!(
        crafting
            .pointer("/qualityDistribution/value/payload/value/value/value/min/value/value")
            .and_then(Value::as_i64),
        Some(16_777_217)
    );
    assert_eq!(
        crafting
            .pointer("/qualityQuantization/value/payload/value/type")
            .and_then(Value::as_str),
        Some("recordRef")
    );
    let slots = alpha
        .pointer("/defaultCargoContainers/value")
        .and_then(Value::as_array)
        .ok_or("original cargo slots")?;
    assert!(
        slots.iter().any(|slot| {
            slot.pointer("/value/slot/value").and_then(Value::as_str) == Some("one_eighthSCU")
                && slot
                    .pointer("/value/entityClass/value/targetType/value")
                    .and_then(Value::as_str)
                    == Some("EntityClassDefinition")
        }),
        "the authored cargo location retains the physical Entity link"
    );
    Ok(())
}

fn assert_selected_names(texts: &Value) {
    assert!(
        texts
            .get("publicNames")
            .and_then(Value::as_array)
            .is_some_and(|names| {
                names.iter().any(|name| {
                    name.pointer("/translations/value/en/displayText/value")
                        .and_then(Value::as_str)
                        == Some("Resource")
                        && name
                            .pointer("/translations/value/de/displayText/value")
                            .and_then(Value::as_str)
                            == Some("Ressource")
                })
            }),
        "the same selected public name retains its two language values"
    );
    assert!(
        texts
            .get("publicNames")
            .and_then(Value::as_array)
            .is_some_and(|names| names.len() == 2)
    );
    assert_eq!(
        texts
            .get("gameTexts")
            .and_then(Value::as_array)
            .map(Vec::len),
        Some(0)
    );
}

pub(super) fn open_fixture(
    database: &DataForgeBuilder,
) -> Result<(tempfile::TempDir, Archive), Box<dyn Error>> {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    let forge = database.build();
    let english = GlobalIniBuilder::new()
        .text("fixture_resource_name", "Resource")
        .text("fixture_group_name", "Materials")
        .text("fixture_resource_description", "Protected")
        .build();
    let german = GlobalIniBuilder::new()
        .text("fixture_resource_name", "Ressource")
        .text("fixture_group_name", "Materialien")
        .text("fixture_resource_description", "Geschuetzter Text")
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", forge).encrypted())
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            english,
        ))
        .entry(TestEntry::stored(
            Language::German.global_ini_path(),
            german,
        ))
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    Ok((scratch, archive))
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{BuiltCatalog, TextOwner, build_catalog, catalog_schema};
use p4kx_extraction::{Archive, DataForge, Language, Value as Raw};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

use super::{Layout, OWNER, PORTS, database};
use crate::commodity_v3_fixture::{TestResult, options};
use crate::health_origin_assertions as proof;
use crate::loadouts_support::{loadout, port};

pub(super) const PORT_KEY: &str = "@fixture_port_display";

#[test]
fn item_ports_supply_canonical_display_names_at_the_original_locale_occurrence() -> TestResult {
    let bytes = database(Layout::Localized).build();
    let ordinal = original_locale_ordinal(&bytes)?;
    let (_fixture, archive) = archive(bytes)?;
    let built = build_catalog(&archive, &options())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    jsonschema::validator_for(&catalog_schema())?
        .validate(&catalog)
        .map_err(|error| format!("item-port catalog schema: {error}"))?;

    let physical = proof::at(&catalog, &format!("{PORTS}/0/ports/value/0/value"))?;
    let name = proof::cell(physical, "displayName", "present")?;
    check_original(&catalog, name, ordinal)?;
    check_loadout_join(&catalog)?;
    check_translations(&catalog, proof::at(name, "/value")?, ordinal)?;
    check_counts(&built)?;
    proof::closure(&catalog)
}

fn original_locale_ordinal(bytes: &[u8]) -> TestResult<usize> {
    let forge = DataForge::open(bytes)?;
    let tree = forge.record(0).ok_or("physical owner Record")?.tree()?;
    let mut ordinals = Vec::new();
    tree.root().visit_owned_values(|visit| {
        if visit.value() == &Raw::Locale(PORT_KEY) {
            ordinals.push(visit.ordinal());
        }
    });
    let [ordinal] = ordinals.as_slice() else {
        return Err("one original authored port Locale".into());
    };
    Ok(*ordinal)
}

fn archive(database: Vec<u8>) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("ports.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", database).encrypted())
        .entry(TestEntry::stored(
            Language::English.global_ini_path(),
            b"fixture_port_display=  Fixture\\nPort  \n",
        ))
        .entry(TestEntry::stored(
            Language::French.global_ini_path(),
            b"other_key=Autre\n",
        ))
        .entry(TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ))
        .write_to(&path)?;
    Ok((
        folder,
        Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
    ))
}

fn check_original(catalog: &Value, carrier: &Value, ordinal: usize) -> TestResult {
    assert_eq!(
        proof::at(carrier, "/containsEncryptedSource")?,
        &json!(true),
        "the physical port key comes from the encrypted DCB"
    );
    assert_eq!(
        proof::at(carrier, "/sourceSetComplete")?,
        &json!(true),
        "the original Locale selection is complete"
    );
    let inputs = proof::uses(catalog, carrier)?;
    let converted: Vec<_> = inputs
        .iter()
        .copied()
        .filter(|used| used.get("role").and_then(Value::as_str) == Some("conversion"))
        .collect();
    let [used] = converted.as_slice() else {
        return Err("one exact original key conversion".into());
    };
    assert_eq!(
        proof::at(used, "/location")?,
        &original_location(ordinal),
        "the reference uses the original owned body occurrence"
    );
    for input in inputs {
        assert_eq!(
            proof::at(proof::source(catalog, input)?, "/path/value")?,
            "Data\\Game2.dcb",
            "the declaration and Locale are bound to the actual DataForge Entry"
        );
    }
    Ok(())
}

fn original_location(ordinal: usize) -> Value {
    json!({"type":"subresource", "namespace":"dataforgeValue", "indices":["0", ordinal.to_string()], "field":"$value"})
}

fn check_loadout_join(catalog: &Value) -> TestResult {
    let owner = loadout(catalog, OWNER)?;
    let mount = port(proof::at(owner, "/ports")?, "mount")?;
    assert_eq!(
        proof::at(mount, "/displayNameKey")?,
        PORT_KEY,
        "the raw Loadout key stays available"
    );
    let link = proof::at(mount, "/sourceConstraints/portDefinition")?;
    assert_eq!(
        proof::at(link, "/state")?,
        "projected",
        "the contextual port links to its physical definition"
    );
    proof::present(link, "ownerRecordIndex", &json!(0), Some(true))?;
    proof::present(link, "componentOrdinal", &json!(1), Some(true))?;
    proof::present(link, "portOrdinal", &json!(0), Some(true))?;
    Ok(())
}

fn check_translations(catalog: &Value, reference: &Value, ordinal: usize) -> TestResult {
    let text = proof::rows(catalog, "/texts/publicNames")?
        .iter()
        .find(|row| row.pointer("/id/value") == Some(reference))
        .ok_or("referenced canonical public port name")?;
    proof::present(text, "key", &json!(PORT_KEY), Some(true))?;
    let english = proof::at(text, "/translations/value/en")?;
    let source = proof::present(
        english,
        "sourceText",
        &json!("  Fixture\\nPort  "),
        Some(true),
    )?;
    let display = proof::present(english, "displayText", &json!("Fixture\nPort"), Some(true))?;
    check_locale_input(catalog, source, ordinal, "payload")?;
    check_locale_input(catalog, display, ordinal, "conversion")?;
    let french = proof::at(text, "/translations/value/fr")?;
    proof::cell(french, "sourceText", "absent")?;
    proof::cell(french, "displayText", "absent")?;
    assert!(
        text.pointer("/translations/value/de").is_none(),
        "an unavailable language has no translation member"
    );
    let language_sources = proof::at(catalog, "/texts/languageSources")?;
    proof::cell(language_sources, "de", "absent")?;
    Ok(())
}

fn check_locale_input(catalog: &Value, form: &Value, ordinal: usize, role: &str) -> TestResult {
    let inputs = proof::uses(catalog, form)?;
    let selector = inputs
        .iter()
        .copied()
        .find(|used| {
            used.get("role").and_then(Value::as_str) == Some("selector")
                && used.get("location") == Some(&original_location(ordinal))
        })
        .ok_or("actual original Locale selector")?;
    let lookup = inputs
        .into_iter()
        .find(|used| {
            used.get("role").and_then(Value::as_str) == Some(role)
                && used.pointer("/location/selector") == selector.get("id")
                && used.pointer("/location/language").and_then(Value::as_str) == Some("en")
        })
        .ok_or("actual lookup and original selector")?;
    assert_eq!(
        proof::at(proof::source(catalog, lookup)?, "/path/value")?,
        Language::English.global_ini_path(),
        "the form uses the exact English Entry"
    );
    Ok(())
}

fn check_counts(built: &BuiltCatalog) -> TestResult {
    let counts = built.summary().text_kind_coverage();
    let ports = |language| {
        counts
            .iter()
            .find(|count| count.owner == TextOwner::ItemPorts && count.language == language)
            .ok_or("physical port text counters")
    };
    assert_eq!(
        ports(Language::English)?.resolved,
        1,
        "one named physical port resolves"
    );
    assert_eq!(
        ports(Language::French)?.missing_key,
        1,
        "the French file lacks the key"
    );
    assert_eq!(
        ports(Language::German)?.missing_language,
        1,
        "the absent German Entry has its own outcome"
    );
    assert_eq!(
        ports(Language::English)?.placeholder,
        2,
        "physical placeholders count separately"
    );
    Ok(())
}

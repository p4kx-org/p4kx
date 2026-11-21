// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::error::Error;
use std::time::SystemTime;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, Kind, build_catalog_from};
use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, DataType, RecordTree, StructValue,
    TreeBuilder, Value as Original,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

use super::{Blueprint, Blueprints, array, occurrence};
use crate::real_archive_verification::print_line;

const RESEARCH: &str = "CraftingResearch";
const MEMBERS: [(&str, &str); 2] = [
    ("researchCosts", "CraftingRecipeCosts_Base"),
    ("unlockRequirements", "CraftingResearchUnlock_Base"),
];

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn crafting_research_matches_original_slots_and_declared_bases() -> TestResult {
    let path = real_archive_path().map_err(|_| "configured Archive path")?;
    let key = real_archive_key().map_err(|_| "configured Archive key")?;
    let archive = Archive::open_with_key(path, Some(key)).map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let selected = select(&forge, &watch)?;
    let mut states = BTreeMap::new();
    for tree in selected.values() {
        let catalog = build_selected(&archive, &entry, tree)?;
        let blueprints = Blueprints::read(&catalog)?;
        let mut rows = blueprints.rows().iter().filter(|blueprint| {
            blueprint
                .definition
                .pointer("/recordIndex/value")
                .and_then(Value::as_u64)
                == Some(u64::from(tree.record_index()))
        });
        let blueprint = rows.next().ok_or("missing original Blueprint definition")?;
        if rows.next().is_some()
            || blueprint
                .definition
                .pointer("/guid/value")
                .and_then(Value::as_str)
                != Some(tree.guid().to_string().as_str())
        {
            return Err("original Blueprint physical identity differs".into());
        }
        check_blueprint(blueprint, tree, &mut states)?;
    }
    if states.is_empty() {
        return Err("the configured Build has no observable research tier states".into());
    }
    watch
        .check()
        .map_err(|_| "Archive changed during research verification")?;
    print_line(&format!("crafting research envelope states: {states:?}"));
    Ok(())
}

fn select<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
) -> TestResult<BTreeMap<ResearchState, RecordTree<'source>>> {
    let rule = Kind::Blueprints.rule().ok_or("selected Blueprint rule")?;
    let mut trees = TreeBuilder::new();
    let mut selected = BTreeMap::new();
    for record in forge.records() {
        watch
            .poll()
            .map_err(|_| "Archive changed during research selection")?;
        if record.type_name()? != rule.type_name || !rule.matches_path(record.record_path()?) {
            continue;
        }
        let tree = trees
            .build(&record)
            .map_err(|_| "read original Blueprint Record")?;
        if let Some(state) = research_state(&tree) {
            selected.entry(state).or_insert(tree);
        }
        if selected.len() == ResearchState::COUNT {
            break;
        }
    }
    if selected.is_empty() {
        return Err("no original Blueprint Record contains a tier".into());
    }
    Ok(selected)
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ResearchState {
    Empty,
    Null,
    Unavailable,
    Populated,
}

impl ResearchState {
    const COUNT: usize = 4;
}

fn research_state(tree: &RecordTree<'_>) -> Option<ResearchState> {
    let Some(Original::Struct(body)) = tree.field("blueprint") else {
        return None;
    };
    let Some(Original::Array(tiers)) = body.field("tiers") else {
        return None;
    };
    tiers
        .iter()
        .filter_map(|tier| {
            let Original::Struct(tier) = tier else {
                return None;
            };
            Some(match tier.field("research") {
                None | Some(Original::Null) => ResearchState::Null,
                Some(Original::Struct(body)) if body.type_name() == RESEARCH => {
                    if MEMBERS
                        .iter()
                        .any(|(member, _)| matches!(body.field(member), Some(Original::Error(_))))
                    {
                        ResearchState::Unavailable
                    } else if MEMBERS.iter().any(|(member, _)| {
                        !matches!(body.field(member), None | Some(Original::Null))
                    }) {
                        ResearchState::Populated
                    } else {
                        ResearchState::Empty
                    }
                }
                _ => ResearchState::Unavailable,
            })
        })
        .max()
}

fn build_selected(
    archive: &Archive,
    entry: &DataForgeEntry,
    tree: &RecordTree<'_>,
) -> TestResult<Value> {
    let filter = Filter::new(tree.record_path());
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(SystemTime::now())
    };
    let built =
        build_catalog_from(archive, entry, &options).map_err(|_| "build research catalog")?;
    let catalog = serde_json::from_slice(built.json())?;
    crate::real_archive_shop_inventory_v3::verify_build(archive, &catalog)?;
    Ok(catalog)
}

fn check_blueprint(
    blueprint: &Blueprint<'_>,
    tree: &RecordTree<'_>,
    states: &mut BTreeMap<&'static str, usize>,
) -> TestResult {
    let original_tiers = match tree.field("blueprint") {
        Some(Original::Struct(blueprint)) => blueprint.field("tiers"),
        _ => None,
    };
    let Some(Original::Array(original_tiers)) = original_tiers else {
        assert!(
            blueprint.tiers.is_empty(),
            "no tier row exists without an original tier"
        );
        return Ok(());
    };
    let arena = array(blueprint.definition, "/source/values")?;
    let blueprint_ref = member_ref(array(blueprint.definition, "/source/roots")?, "blueprint")
        .ok_or("original Blueprint body reference")?;
    let tiers_ref = member_ref(
        array(node(arena, blueprint_ref)?, "/value/value/fields")?,
        "tiers",
    )
    .ok_or("original Blueprint tier array reference")?;
    let slots = array(node(arena, tiers_ref)?, "/value/value/elements")?;
    assert_eq!(
        slots.len(),
        original_tiers.len(),
        "the retained tier array preserves source slots"
    );
    let mut rows = blueprint.tiers.iter();
    for (original, reference) in original_tiers.iter().zip(slots) {
        let Original::Struct(original) = original else {
            continue;
        };
        let row = rows.next().ok_or("one projection of the original tier")?;
        assert!(
            row.get("source") == Some(reference),
            "the tier uses the exact original slot"
        );
        let state = check_envelope(arena, reference, row, original, tree)?;
        *states.entry(state).or_default() += 1;
    }
    assert!(
        rows.next().is_none(),
        "every projected tier has one original source"
    );
    Ok(())
}

fn check_envelope(
    arena: &[Value],
    tier: &Value,
    row: &Value,
    original_tier: &StructValue<'_>,
    tree: &RecordTree<'_>,
) -> TestResult<&'static str> {
    let source = member_ref(
        array(node(arena, tier)?, "/value/value/fields")?,
        "research",
    );
    let research = row.get("research").ok_or("research row")?;
    assert!(
        research.get("source") == Some(source.unwrap_or(&Value::Null)),
        "original research occurrence"
    );
    let original = original_tier.field("research");
    let expected = match original {
        None => "absent",
        Some(Original::Null) => "null",
        Some(Original::Error(_)) => "unreadable",
        Some(Original::Struct(body)) if body.type_name() == RESEARCH => "present",
        Some(_) => "unsupported",
    };
    assert!(
        research.pointer("/envelope/state").and_then(Value::as_str) == Some(expected),
        "original research envelope state"
    );
    if let Some(Original::Struct(body)) = original
        && body.type_name() == RESEARCH
    {
        let source = source.ok_or("original research body occurrence")?;
        let fields = array(node(arena, source)?, "/value/value/fields")?;
        let envelope = research
            .pointer("/envelope/value")
            .ok_or("sourced research envelope")?;
        let [costs, unlock] =
            MEMBERS.map(|member| check_member(tree, body, fields, envelope, member));
        return Ok(match (costs?, unlock?) {
            ("unreadable", _) | (_, "unreadable") => "present-unreadable",
            ("unsupported", _) | (_, "unsupported") => "present-unsupported",
            ("present", _) | (_, "present") => "present-known",
            _ => "present-empty",
        });
    }
    Ok(expected)
}

fn check_member(
    tree: &RecordTree<'_>,
    body: &StructValue<'_>,
    fields: &[Value],
    envelope: &Value,
    (name, expected_base): (&str, &str),
) -> TestResult<&'static str> {
    let reference = member_ref(fields, name);
    let projected = envelope.get(name).ok_or("sourced research member")?;
    assert!(
        projected.get("source") == Some(reference.unwrap_or(&Value::Null)),
        "original research member occurrence"
    );
    let Some(field) = body.fields().iter().find(|field| field.name() == name) else {
        assert!(
            projected.pointer("/value/state").and_then(Value::as_str) == Some("absent"),
            "undeclared research member is absent"
        );
        return Ok("absent");
    };
    let storage = field.storage();
    let declared = if storage.data_type() == DataType::StrongPointer && !storage.is_array() {
        tree.struct_at(u32::from(storage.target()))
            .and_then(|definition| definition.name().ok())
    } else {
        None
    };
    assert!(
        projected
            .pointer("/declaredBase/value")
            .and_then(Value::as_str)
            == declared,
        "original declared pointer base"
    );
    let expected = match field.value() {
        Original::Error(_) => "unreadable",
        _ if declared != Some(expected_base) => "unsupported",
        Original::Null => "null",
        Original::Struct(body) if body.type_name() == expected_base && body.fields().is_empty() => {
            "present"
        }
        _ => "unsupported",
    };
    assert!(
        projected.pointer("/value/state").and_then(Value::as_str) == Some(expected),
        "original research member state"
    );
    if expected == "present" {
        assert!(
            projected.pointer("/value/value") == reference,
            "known research refers to the original value"
        );
    }
    Ok(expected)
}

fn node<'arena>(arena: &'arena [Value], reference: &Value) -> TestResult<&'arena Value> {
    let ordinal = occurrence(reference)?.parse::<usize>()?;
    arena
        .get(ordinal)
        .ok_or_else(|| "original Blueprint source value".into())
}

fn member_ref<'fields>(fields: &'fields [Value], name: &str) -> Option<&'fields Value> {
    fields
        .iter()
        .find(|field| field.pointer("/name/value").and_then(Value::as_str) == Some(name))
        .and_then(|field| field.get("value"))
}

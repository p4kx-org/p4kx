// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::{Figures, PartsResult, linked, nullable, required, selected, structs};
use crate::catalog_v3_consumer as reader;
use crate::real_archive_contract_templates::mission_graph::{self as graph, Mission};

pub(super) struct English<'catalog> {
    display: HashMap<&'catalog str, bool>,
}

impl<'catalog> English<'catalog> {
    pub(super) fn read(catalog: &'catalog Value) -> PartsResult<Self> {
        let mut display = HashMap::new();
        for row in graph::array(required(catalog, "texts")?, "gameTexts")? {
            let id = reader::present(required(row, "id")?)?
                .as_str()
                .ok_or("canonical protected text id")?;
            let translations = reader::present(required(row, "translations")?)?;
            let has_text = translations
                .get("en")
                .map(|forms| {
                    let cell = reader::source_value(required(forms, "displayText")?)?;
                    Ok::<_, Box<dyn std::error::Error>>(
                        cell.and_then(Value::as_str)
                            .is_some_and(|text| !text.is_empty()),
                    )
                })
                .transpose()?
                .unwrap_or_default();
            if display.insert(id, has_text).is_some() {
                return Err("one row per canonical GameTextRef".into());
            }
        }
        Ok(Self { display })
    }

    fn resolved(&self, lookup: &Value) -> PartsResult<bool> {
        let Some(text) = reader::source_value(required(lookup, "text")?)? else {
            return Ok(false);
        };
        let id = text
            .as_str()
            .ok_or("original localization's canonical GameTextRef")?;
        self.display
            .get(id)
            .copied()
            .ok_or_else(|| "the original GameTextRef must resolve".into())
    }
}

type ByOccurrence<'catalog> = HashMap<&'catalog str, &'catalog Value>;

pub(super) fn count<'catalog>(
    mission: &Mission<'catalog>,
    name: &str,
    items: &HashSet<&str>,
    english: &English<'catalog>,
    figures: &mut Figures,
) -> PartsResult {
    let text = index(mission, "localizations", "source")?;
    let properties = index(mission, "propertyBindings", "caller")?;
    let mut objectives = HashSet::new();
    for objective in graph::array(mission.projection, "objectives")? {
        let source = required(objective, "source")?;
        let node = mission.follow(source)?;
        if !objectives.insert(graph::position(source, "/valueIndex")?) {
            return Err("one interpretation for each actual selected objective".into());
        }
        figures.comms += structs(mission, mission.member(node, "commsNotifications")?)?.len();
        figures.resolved_texts += display(
            mission,
            selected(mission, node, "displayInfo")?,
            &text,
            english,
        )?;
        if let Some(handler) = selected(mission, node, "objectiveHandler")? {
            figures.handlers += 1;
            for field in [
                "location",
                "organization",
                "haulingLimitProperty",
                "targetObject",
            ] {
                figures.handler_property_links += usize::from(property_link(
                    mission,
                    mission.member(handler, field)?,
                    &properties,
                )?);
            }
            check_handler(mission, handler, name, items, (&text, english), figures)?;
        }
    }
    count_properties(mission, &objectives, &properties, figures)
}

fn check_handler<'catalog>(
    mission: &Mission<'catalog>,
    handler: &'catalog Value,
    name: &str,
    items: &HashSet<&str>,
    (text, english): (&ByOccurrence<'catalog>, &English<'catalog>),
    figures: &mut Figures,
) -> PartsResult {
    if let Some(nav) = selected(mission, handler, "navPointSpawnInfo")? {
        if let Some(guid) = linked(mission.member(nav, "entityClassDefinition")?)? {
            assert!(items.contains(guid), "{name}: marker entity {guid}");
        }
        figures.resolved_texts +=
            usize::from(resolved(mission.member(nav, "name")?, text, english)?);
    }
    for field in ["travelObjectiveInfo", "returnObjectiveInfo"] {
        figures.resolved_texts +=
            display(mission, selected(mission, handler, field)?, text, english)?;
    }
    if let Some(settings) = selected(mission, handler, "objectiveSettings")? {
        for field in [
            "andObjective",
            "orObjective",
            "collectObjective",
            "collectAndDeliverObjective",
            "resourceDeliverObjective",
            "itemDeliverObjective",
            "listItemObjective",
        ] {
            figures.resolved_texts +=
                display(mission, selected(mission, settings, field)?, text, english)?;
        }
    }
    if mission.member(handler, "instanceTagIdentifier")?.is_some() {
        figures.resolved_texts += usize::from(resolved(
            mission.member(handler, "instanceHUDObjectiveText")?,
            text,
            english,
        )?);
    }
    Ok(())
}

fn count_properties(
    mission: &Mission<'_>,
    objectives: &HashSet<usize>,
    bindings: &ByOccurrence<'_>,
    figures: &mut Figures,
) -> PartsResult {
    for property in graph::array(mission.projection, "properties")? {
        let Some(objective) = nullable(property, "objective")? else {
            continue;
        };
        assert!(
            objectives.contains(&graph::position(objective, "/valueIndex")?),
            "objective properties retain the actual selected source objective"
        );
        mission.follow(required(property, "source")?)?;
        figures.properties += 1;
        if let Some(value) = nullable(property, "value")? {
            mission.follow(value)?;
            figures.property_values += 1;
        }
        let caller = nullable(property, "contractProperty")?
            .map(|reference| mission.follow(reference))
            .transpose()?;
        figures.referenced_links += usize::from(property_link(mission, caller, bindings)?);
    }
    Ok(())
}

fn property_link(
    mission: &Mission<'_>,
    caller: Option<&Value>,
    bindings: &ByOccurrence<'_>,
) -> PartsResult<bool> {
    let Some(caller) = caller else {
        return Ok(false);
    };
    if caller.pointer("/value/value/type").and_then(Value::as_str) != Some("weakReference") {
        return Ok(false);
    }
    let original = graph::text(caller, "/sourceOrdinal")?;
    let binding = bindings
        .get(original)
        .ok_or("actual interpretation of the owned weak caller")?;
    if graph::text(binding, "/target/interpretation")? != "value" {
        return Ok(false);
    }
    mission.field(
        binding
            .pointer("/target/field")
            .ok_or("actual variable declaration")?,
    )?;
    Ok(true)
}

fn display(
    mission: &Mission<'_>,
    display: Option<&Value>,
    text: &ByOccurrence<'_>,
    english: &English<'_>,
) -> PartsResult<usize> {
    let Some(display) = display else {
        return Ok(0);
    };
    let mut count = 0;
    for field in [
        "objectiveMarkerLabel",
        "shortDescription",
        "longDescription",
    ] {
        count += usize::from(resolved(mission.member(display, field)?, text, english)?);
    }
    Ok(count)
}

fn resolved(
    original: Option<&Value>,
    text: &ByOccurrence<'_>,
    english: &English<'_>,
) -> PartsResult<bool> {
    let Some(original) = original else {
        return Ok(false);
    };
    let ordinal = graph::text(original, "/sourceOrdinal")?;
    english.resolved(
        text.get(ordinal)
            .ok_or("actual selected localization occurrence")?,
    )
}

fn index<'catalog>(
    mission: &Mission<'catalog>,
    collection: &str,
    field: &str,
) -> PartsResult<ByOccurrence<'catalog>> {
    let mut by_occurrence = HashMap::new();
    for row in graph::array(mission.projection, collection)? {
        let node = mission.follow(required(row, field)?)?;
        let original = graph::text(node, "/sourceOrdinal")?;
        if by_occurrence.insert(original, row).is_some() {
            return Err(format!("{collection} has repeated original source occurrences").into());
        }
    }
    Ok(by_occurrence)
}

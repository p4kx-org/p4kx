// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_export::Kind;
use serde_json::Value;

use super::{
    Coverage, CoverageCounters, Oracle, TestResult, TextDomain, TextIndex, TextReferenceShape,
    TextTarget,
};
use crate::catalog_v3_consumer;

#[path = "real_archive_live_item_texts.rs"]
mod installed_items;

pub(super) fn location_templates(
    catalog: &Value,
    english: &Oracle,
    german: &Oracle,
) -> TestResult<(Coverage, Coverage)> {
    let index = TextIndex::new(catalog)?;
    let mut counters = CoverageCounters::new(english, german);
    for template in catalog_v3_consumer::rows(catalog, Kind::MissionLocationTemplates)? {
        let names = template
            .get("names")
            .and_then(Value::as_array)
            .ok_or("location names")?;
        for name in names {
            let target = index.reference(
                name.get("displayName").ok_or("location name reference")?,
                TextDomain::GameText,
                TextReferenceShape::Required,
            )?;
            counters.count(target)?;
        }
    }
    Ok(counters.finish())
}

pub(super) fn npc_names(
    catalog: &Value,
    english: &Oracle,
    german: &Oracle,
) -> TestResult<(Coverage, Coverage)> {
    let index = TextIndex::new(catalog)?;
    let mut counters = CoverageCounters::new(english, german);
    for pool in catalog_v3_consumer::rows(catalog, Kind::ContractNpcSpawns)? {
        for group in super::graph::array(pool, "groups")? {
            for option in super::graph::array(group, "options")? {
                let name = option.get("embeddedName").ok_or("NPC embedded name")?;
                if name.is_null() {
                    continue;
                }
                count_npc_name(&index, &mut counters, name)?;
            }
        }
    }
    Ok(counters.finish())
}

fn count_npc_name(
    index: &TextIndex<'_>,
    counters: &mut CoverageCounters<'_>,
    name: &Value,
) -> TestResult<()> {
    for field in ["givenName", "givenLastName", "givenNickName"] {
        let target = index.reference(
            name.get(field).ok_or("NPC name reference")?,
            TextDomain::GameText,
            TextReferenceShape::Required,
        )?;
        counters.count(target)?;
    }
    Ok(())
}

pub(super) fn compare(
    counters: &mut CoverageCounters<'_>,
    index: &TextIndex<'_>,
    kind: Kind,
    row: &Value,
) -> TestResult<()> {
    let (field, domain) = match kind {
        Kind::ResourceGroups | Kind::ReputationScopes => ("description", TextDomain::GameText),
        Kind::ReputationStandings => ("perkDescription", TextDomain::GameText),
        Kind::GameplayProperties => ("unitFormat", TextDomain::PublicName),
        Kind::FactionReputations => return compare_faction_description(counters, index, row),
        Kind::Ships => return compare_ship_labels(counters, index, row),
        _ => return Ok(()),
    };
    let target = index.reference(
        row.get(field).ok_or("authored text carrier")?,
        domain,
        TextReferenceShape::Required,
    )?;
    counters.count_supplemental(target)?;
    if kind == Kind::GameplayProperties {
        compare_overrides(counters, index, row)?;
    }
    Ok(())
}

fn compare_ship_labels(
    counters: &mut CoverageCounters<'_>,
    index: &TextIndex<'_>,
    row: &Value,
) -> TestResult<()> {
    for label in ["role", "career"] {
        let target = index.reference(
            row.get(label).ok_or("ship text carrier")?,
            TextDomain::PublicName,
            TextReferenceShape::Required,
        )?;
        counters.count_supplemental(target)?;
    }
    Ok(())
}

fn compare_faction_description(
    counters: &mut CoverageCounters<'_>,
    index: &TextIndex<'_>,
    row: &Value,
) -> TestResult<()> {
    let carrier = row
        .get("description")
        .ok_or("faction description carrier")?;
    let Some(description) = catalog_v3_consumer::source_value(carrier)? else {
        return Ok(());
    };
    match description.get("type").and_then(Value::as_str) {
        Some("localized") => {
            let id = description
                .get("reference")
                .and_then(Value::as_str)
                .ok_or("faction description reference")?;
            let target = index.target(id, TextDomain::GameText)?;
            counters.count_supplemental(Some(target))
        }
        Some("literal") if description.get("text").is_some_and(Value::is_string) => Ok(()),
        _ => Err("invalid faction description form".into()),
    }
}

fn compare_overrides(
    counters: &mut CoverageCounters<'_>,
    index: &TextIndex<'_>,
    row: &Value,
) -> TestResult<()> {
    let list = row
        .get("nameOverrides")
        .ok_or("authored name override array")?;
    let Some(list) = catalog_v3_consumer::source_value(list)? else {
        return Ok(());
    };
    let items = list.as_array().ok_or("ordered name override array")?;
    for item in items {
        let Some(item) = catalog_v3_consumer::source_value(item)? else {
            continue;
        };
        let body = item.get("payload").ok_or("authored name override body")?;
        let Some(body) = catalog_v3_consumer::source_value(body)? else {
            continue;
        };
        let target = index.reference(
            body.get("propertyName")
                .ok_or("authored property name override")?,
            TextDomain::PublicName,
            TextReferenceShape::Required,
        )?;
        counters.count_supplemental(target)?;
    }
    Ok(())
}

pub(crate) struct ItemPresentationIndex<'catalog> {
    definitions: HashMap<u64, &'catalog Value>,
}

impl<'catalog> ItemPresentationIndex<'catalog> {
    pub(crate) fn new(catalog: &'catalog Value) -> TestResult<Self> {
        let rows = catalog_v3_consumer::table(catalog, "/definitions/items")?;
        let mut definitions = HashMap::with_capacity(rows.len());
        for row in rows {
            if definitions.insert(record_index(row)?, row).is_some() {
                return Err("duplicate physical Item definition".into());
            }
        }
        Ok(Self { definitions })
    }

    fn components(&self, item: &Value) -> TestResult<&'catalog [Value]> {
        let row = self
            .definitions
            .get(&record_index(item)?)
            .ok_or("Item projection has no exact physical definition")?;
        if item.get("recordIndex") != row.get("recordIndex")
            || [("guid", "/guid/value"), ("recordPath", "/recordPath/value")]
                .into_iter()
                .any(|(field, path)| item.get(field) != row.pointer(path))
        {
            return Err("Item projection and physical definition disagree".into());
        }
        let carrier = row.get("components").ok_or("Item component carrier")?;
        catalog_v3_consumer::source_value(carrier)?.map_or_else(
            || Ok(&[][..]),
            |components| {
                components
                    .as_array()
                    .map(Vec::as_slice)
                    .ok_or_else(|| "ordered Item components".into())
            },
        )
    }

    pub(crate) fn description(&self, item: &Value) -> TestResult<Option<&'catalog Value>> {
        let Some(component) = self
            .components(item)?
            .iter()
            .find(|component| component.get("kind").and_then(Value::as_str) == Some("attachment"))
        else {
            return Ok(None);
        };
        localization(component)?.map_or(Ok(None), |localization| {
            Ok(Some(
                localization
                    .get("description")
                    .ok_or("Item description carrier")?,
            ))
        })
    }
}

fn record_index(row: &Value) -> TestResult<u64> {
    let carrier = row
        .get("recordIndex")
        .ok_or("Item physical Record carrier")?;
    catalog_v3_consumer::source_value(carrier)?
        .and_then(Value::as_u64)
        .ok_or_else(|| "present Item physical Record index".into())
}

fn localization(component: &Value) -> TestResult<Option<&Value>> {
    let carrier = component.get("value").ok_or("Item attachment carrier")?;
    let Some(attachment) = catalog_v3_consumer::source_value(carrier)? else {
        return Ok(None);
    };
    let carrier = attachment
        .get("localization")
        .ok_or("Item localization carrier")?;
    catalog_v3_consumer::source_value(carrier)
}

pub(super) fn items(catalog: &Value, counters: &mut CoverageCounters<'_>) -> TestResult<()> {
    let index = TextIndex::new(catalog)?;
    let presentations = ItemPresentationIndex::new(catalog)?;
    let mut selected = HashSet::new();
    for item in catalog_v3_consumer::rows(catalog, Kind::Items)? {
        if !selected.insert(record_index(item)?) {
            return Err("duplicate physical Item projection".into());
        }
        compare_item_slots(counters, &index, presentations.components(item)?)?;
    }
    if selected.len() != presentations.definitions.len() {
        return Err("selected Item projections and physical definitions differ".into());
    }
    Ok(())
}

#[derive(Default)]
enum Slot {
    #[default]
    First,
    Additional,
}

impl Slot {
    const fn next(&mut self) -> Self {
        std::mem::replace(self, Self::Additional)
    }

    fn count(
        &self,
        counters: &mut CoverageCounters<'_>,
        target: Option<&TextTarget<'_>>,
    ) -> TestResult<()> {
        match self {
            Self::First => counters.count(target.filter(|target| !target.key.is_empty())),
            Self::Additional => counters.count_supplemental(target),
        }
    }
}

fn compare_item_slots(
    counters: &mut CoverageCounters<'_>,
    index: &TextIndex<'_>,
    components: &[Value],
) -> TestResult<()> {
    let mut attachment = Slot::default();
    let mut purchasable = Slot::default();
    for component in components {
        match component.get("kind").and_then(Value::as_str) {
            Some("attachment") => {
                let slot = attachment.next();
                let Some(localization) = localization(component)? else {
                    continue;
                };
                for (field, domain) in [
                    ("name", TextDomain::PublicName),
                    ("description", TextDomain::GameText),
                ] {
                    let target = index.reference(
                        localization
                            .get(field)
                            .ok_or("localized Item text carrier")?,
                        domain,
                        TextReferenceShape::Required,
                    )?;
                    slot.count(counters, target)?;
                }
            }
            Some("purchasable") => {
                let target = index.reference(
                    component
                        .pointer("/value/displayType")
                        .ok_or("Item display type carrier")?,
                    TextDomain::GameText,
                    TextReferenceShape::Required,
                )?;
                purchasable.next().count(counters, target)?;
            }
            Some("physics") => {}
            _ => return Err("unknown Item component kind".into()),
        }
    }
    Ok(())
}

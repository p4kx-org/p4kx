// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{ArchiveWatch, DataForge, GlobalIni, Guid, Record, StructValue, Value};
use serde_json::{Value as Json, json};

use super::TestResult;
use crate::real_archive_auto_spawns::original::Locations;

const MISSION_ITEM: &str = "MissionPropertyValue_MissionItem";
const SPECIFIC: &str = "DataSetMatchCondition_SpecificItemsDef";
const PREFIX: &str = "~mission(";

#[derive(Clone, Copy)]
pub(super) enum Expected<'source> {
    StaticPreview { item: Guid, display: &'source str },
    Runtime,
}

pub(super) struct Case<'source> {
    pub(super) generator_path: &'source str,
    pub(super) contract: Json,
    pub(super) field: &'static str,
    pub(super) token: &'source str,
    pub(super) variant: Option<&'source str>,
    pub(super) key: &'source str,
    pub(super) expected: Expected<'source>,
}

#[derive(Default)]
pub(super) struct Cases<'source> {
    pub(super) singleton: Option<Case<'source>>,
    pub(super) runtime: Option<Case<'source>>,
}

pub(super) fn select<'source>(
    forge: &'source DataForge<'source>,
    watch: &ArchiveWatch,
    english: &GlobalIni<'source>,
) -> TestResult<Cases<'source>> {
    let mut cases = Cases::default();
    for record in forge.records() {
        watch.poll()?;
        if record.type_name()? != "ContractGenerator" {
            continue;
        }
        scan(forge, &record, english, &mut cases)?;
        if cases.singleton.is_some() && cases.runtime.is_some() {
            return Ok(cases);
        }
    }
    Ok(cases)
}

fn scan<'source>(
    forge: &'source DataForge<'source>,
    record: &Record<'source>,
    english: &GlobalIni<'source>,
    cases: &mut Cases<'source>,
) -> TestResult {
    let tree = record.tree()?;
    let locations = Locations::read(tree.root());
    for (raw_handler, handler) in structures(tree.field("generators")) {
        let handler_params = first_structure(handler.field("contractParams"));
        for (raw_contract, contract) in structures(handler.field("contracts")) {
            let contract_params = first_structure(contract.field("paramOverrides"));
            let template = target(forge, contract.field("template"))?;
            let broker = target(forge, contract.field("missionBrokerEntry"))?;
            let authored = Authored {
                contract: contract_params,
                handler: handler_params,
                template: template.as_ref().map(p4kx_extraction::RecordTree::root),
                broker: broker.as_ref().map(p4kx_extraction::RecordTree::root),
            };
            let selection = Selection {
                forge,
                english,
                record,
                locations: &locations,
                slots: [raw_handler, raw_contract],
            };
            if cases.singleton.is_none() {
                cases.singleton = selection.case(&authored, TextField::Title)?;
            }
            if cases.runtime.is_none() {
                cases.runtime = selection.case(&authored, TextField::Description)?;
            }
        }
    }
    Ok(())
}

struct Selection<'read, 'source> {
    forge: &'source DataForge<'source>,
    english: &'read GlobalIni<'source>,
    record: &'read Record<'source>,
    locations: &'read Locations<'source>,
    slots: [&'read Value<'source>; 2],
}

impl<'source> Selection<'_, 'source> {
    fn case(
        &self,
        authored: &Authored<'_, 'source>,
        field: TextField,
    ) -> TestResult<Option<Case<'source>>> {
        let Some(key) = authored.key(field) else {
            return Ok(None);
        };
        let Some(original) = self.english.resolve(key) else {
            return Ok(None);
        };
        for (name, variant) in tokens(original) {
            let Some(property) = authored.token(name) else {
                continue;
            };
            if property.type_name() != MISSION_ITEM {
                continue;
            }
            let expected = match field {
                TextField::Title if variant.is_none() => {
                    let Some(item) = singleton(property) else {
                        continue;
                    };
                    let Some(display) = self.static_display(item)? else {
                        continue;
                    };
                    Expected::StaticPreview { item, display }
                }
                TextField::Description if variant.is_some() && is_tag_runtime(property) => {
                    Expected::Runtime
                }
                _ => continue,
            };
            let [handler, contract] = self.slots;
            return Ok(Some(Case {
                generator_path: self.record.record_path()?,
                contract: json!({
                    "recordIndex": self.record.index(),
                    "handler": {"valueIndex": self.locations.ordinal(handler)?.to_string()},
                    "contract": {"valueIndex": self.locations.ordinal(contract)?.to_string()},
                    "subContract": null,
                }),
                field: field.name(),
                token: name,
                variant,
                key,
                expected,
            }));
        }
        Ok(None)
    }

    fn static_display(&self, item: Guid) -> TestResult<Option<&'source str>> {
        let Some(item) = self.forge.record_by_guid(item) else {
            return Ok(None);
        };
        if item.type_name()? != "MissionItem" {
            return Ok(None);
        }
        let item = item.tree()?;
        let variants = first_structure(item.field("stringVariants"));
        let Some(Value::Array(slots)) = variants.and_then(|body| body.field("variants")) else {
            return Ok(None);
        };
        let [Value::Struct(variant)] = slots.as_ref() else {
            return Ok(None);
        };
        let Some(key) = key(variant.field("string")) else {
            return Ok(None);
        };
        Ok(self
            .english
            .resolve(key)
            .filter(|value| !value.is_empty() && !value.contains(PREFIX)))
    }
}

#[derive(Clone, Copy)]
enum TextField {
    Title,
    Description,
}

impl TextField {
    const fn name(self) -> &'static str {
        match self {
            Self::Title => "title",
            Self::Description => "description",
        }
    }

    const fn override_name(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::Description => "Description",
        }
    }
}

struct Authored<'read, 'source> {
    contract: Option<&'read StructValue<'source>>,
    handler: Option<&'read StructValue<'source>>,
    template: Option<&'read StructValue<'source>>,
    broker: Option<&'read StructValue<'source>>,
}

impl<'read, 'source> Authored<'read, 'source> {
    fn key(&self, field: TextField) -> Option<&'source str> {
        self.contract
            .into_iter()
            .chain(self.handler)
            .find_map(|owner| override_key(owner, field))
            .or_else(|| {
                if matches!(field, TextField::Title) {
                    self.template.and_then(template_title)
                } else {
                    None
                }
            })
            .or_else(|| key(self.broker.and_then(|broker| broker.field(field.name()))))
    }

    fn token(&self, name: &str) -> Option<&'read StructValue<'source>> {
        let mut scopes = [
            (self.contract, "propertyOverrides"),
            (self.handler, "propertyOverrides"),
            (self.template, "contractProperties"),
        ]
        .into_iter();
        scopes
            .find_map(|(owner, field)| {
                owner.and_then(|owner| {
                    structures(owner.field(field))
                        .find(|(_, slot)| {
                            text(slot.field("extendedTextToken"))
                                .is_some_and(|key| key.eq_ignore_ascii_case(name))
                        })
                        .map(|(_, slot)| first_structure(slot.field("value")))
                })
            })
            .flatten()
    }
}

fn singleton(source: &StructValue<'_>) -> Option<Guid> {
    if !matches!(source.field("minItemsToFind"), Some(Value::Int32(1)))
        || !matches!(source.field("maxItemsToFind"), Some(Value::Int32(1)))
        || !matches!(source.field("debugForceChosenItem"), Some(Value::Null))
    {
        return None;
    }
    let Value::Array(conditions) = source.field("matchConditions")? else {
        return None;
    };
    let [Value::Struct(condition)] = conditions.as_ref() else {
        return None;
    };
    if condition.type_name() != SPECIFIC {
        return None;
    }
    let Value::Array(items) = condition.field("items")? else {
        return None;
    };
    let [item] = items.as_ref() else {
        return None;
    };
    reference(Some(item))
}

fn is_tag_runtime(source: &StructValue<'_>) -> bool {
    let Some(Value::Array(conditions)) = source.field("matchConditions") else {
        return false;
    };
    !conditions.is_empty()
        && conditions.iter().all(|condition| {
            matches!(condition, Value::Struct(body) if matches!(body.type_name(),
            "DataSetMatchCondition_TagSearch" | "DataSetMatchCondition_PropertyTags"))
        })
}

fn template_title<'source>(root: &StructValue<'source>) -> Option<&'source str> {
    let display = first_structure(root.field("contractDisplayInfo"))?;
    let Value::Array(strings) = display.field("displayString")? else {
        return None;
    };
    let first = strings.first()?;
    key(match first {
        Value::Struct(slot) => slot.field("value"),
        value => Some(value),
    })
}

fn override_key<'source>(owner: &StructValue<'source>, field: TextField) -> Option<&'source str> {
    let (_, slot) = structures(owner.field("stringParamOverrides"))
        .find(|(_, slot)| text(slot.field("param")) == Some(field.override_name()))?;
    key(slot.field("value"))
}

fn target<'source>(
    forge: &'source DataForge<'source>,
    value: Option<&Value<'source>>,
) -> TestResult<Option<p4kx_extraction::RecordTree<'source>>> {
    reference(value)
        .and_then(|guid| forge.record_by_guid(guid))
        .map(|record| record.tree())
        .transpose()
        .map_err(Into::into)
}

fn reference(value: Option<&Value<'_>>) -> Option<Guid> {
    match value? {
        Value::Reference(value) if value.location().is_none() => Some(value.guid()),
        _ => None,
    }
}

fn structures<'read, 'source>(
    value: Option<&'read Value<'source>>,
) -> impl Iterator<Item = (&'read Value<'source>, &'read StructValue<'source>)> {
    let values = match value {
        Some(Value::Array(values)) => values.as_ref(),
        Some(value @ Value::Struct(_)) => std::slice::from_ref(value),
        _ => &[],
    };
    values.iter().filter_map(|value| match value {
        Value::Struct(body) => Some((value, body)),
        _ => None,
    })
}

fn first_structure<'read, 'source>(
    value: Option<&'read Value<'source>>,
) -> Option<&'read StructValue<'source>> {
    structures(value).next().map(|(_, body)| body)
}

fn key<'source>(value: Option<&Value<'source>>) -> Option<&'source str> {
    match value? {
        Value::Locale(key) | Value::String(key)
            if key.starts_with('@') && !p4kx_extraction::is_placeholder_key(key) =>
        {
            Some(key)
        }
        _ => None,
    }
}

fn text<'source>(value: Option<&Value<'source>>) -> Option<&'source str> {
    match value? {
        Value::String(text) | Value::Enum(text) if !text.is_empty() => Some(text),
        _ => None,
    }
}

fn tokens(mut text: &str) -> impl Iterator<Item = (&str, Option<&str>)> {
    std::iter::from_fn(move || {
        let (_, tail) = text.split_once(PREFIX)?;
        let (content, tail) = tail.split_once(')')?;
        text = tail;
        Some(
            content
                .split_once('|')
                .map_or((content, None), |(name, variant)| {
                    (name, Some(variant).filter(|variant| !variant.is_empty()))
                }),
        )
    })
}

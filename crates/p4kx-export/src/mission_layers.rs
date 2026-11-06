// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, StructValue, Value, is_placeholder_key};

use crate::blueprint::as_struct;
use crate::mission_hauling::{HaulingProperty, property_orders};
use crate::mission_index::Broker;
use crate::record_fields::{boolean, integer, reference, text};

const STRING_HASH: &str = "MissionPropertyValue_StringHash";
const ORGANIZATION: &str = "MissionPropertyValue_Organization";
const TITLE: &str = "Title";
const DESCRIPTION: &str = "Description";
const CONTRACTOR: &str = "Contractor";

/// What a mission property says a `~mission(Token)` in a text stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Prop<'f> {
    /// The localisation key of the text of the first option of a string-hash property.
    Key(&'f str),
    /// The first organisation of an organisation property.
    Organization(Guid),
    /// A value the game picks when the mission starts; the text names what it is.
    Placeholder(&'static str),
    /// A property type whose value the catalog does not read.
    Unknown,
}

/// A mission property: the token a text refers to it by and its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Property<'f> {
    pub(crate) token: &'f str,
    pub(crate) value: Prop<'f>,
}

/// What one level of a contract overrides: the title, the description, the mission type, the
/// contractor, the boolean and integer parameters and the properties.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Layer<'f> {
    pub(crate) title: Option<&'f str>,
    pub(crate) description: Option<&'f str>,
    pub(crate) mission_type: Option<Guid>,
    pub(crate) contractor: Option<&'f str>,
    pub(crate) properties: Vec<Property<'f>>,
    pub(crate) hauling: Vec<HaulingProperty<'f>>,
    bools: Vec<(&'f str, bool)>,
    ints: Vec<(&'f str, i64)>,
}

impl<'f> Layer<'f> {
    /// Reads the overrides in `params`, the `paramOverrides` of a contract or sub contract or
    /// the `contractParams` of a handler.
    pub(crate) fn read(params: Option<&StructValue<'f>>) -> Self {
        let string_param = |name: &str| {
            items(params, "stringParamOverrides")
                .iter()
                .filter_map(as_struct)
                .find(|param| text(Some(param), "param") == Some(name))
                .and_then(|param| key(Some(param), "value"))
        };
        let named = |field: &str| {
            items(params, field)
                .iter()
                .filter_map(as_struct)
                .filter_map(|param| Some((text(Some(param), "param")?, param)))
                .collect::<Vec<_>>()
        };
        Self {
            title: string_param(TITLE),
            description: string_param(DESCRIPTION),
            contractor: string_param(CONTRACTOR),
            mission_type: reference(params, "missionTypeOverride").map(|guid| guid.0),
            properties: properties(params, "propertyOverrides"),
            hauling: property_orders(params, "propertyOverrides"),
            bools: named("boolParamOverrides")
                .into_iter()
                .filter_map(|(name, param)| Some((name, boolean(Some(param), "value")?)))
                .collect(),
            ints: named("intParamOverrides")
                .into_iter()
                .filter_map(|(name, param)| Some((name, integer(Some(param), "value")?)))
                .collect(),
        }
    }

    /// Returns the boolean parameter `name` this level overrides, matched without regard to case.
    pub(crate) fn bool_param(&self, name: &str) -> Option<bool> {
        self.bools
            .iter()
            .find(|(param, _)| param.eq_ignore_ascii_case(name))
            .map(|(_, value)| *value)
    }

    /// Returns the integer parameter `name` this level overrides, matched without regard to case.
    pub(crate) fn int_param(&self, name: &str) -> Option<i64> {
        self.ints
            .iter()
            .find(|(param, _)| param.eq_ignore_ascii_case(name))
            .map(|(_, value)| *value)
    }
}

/// Returns the localisation key of the description: the first level that states one, else the
/// broker entry's.
pub(crate) fn description_key<'a, 'f: 'a>(
    levels: impl IntoIterator<Item = &'a Layer<'f>>,
    broker: Option<&Broker<'f>>,
) -> Option<&'f str> {
    levels
        .into_iter()
        .find_map(|level| level.description)
        .or_else(|| broker.and_then(|broker| broker.description))
}

/// Reads the mission properties the array field `name` of `parent` holds.
pub(crate) fn properties<'f>(parent: Option<&StructValue<'f>>, name: &str) -> Vec<Property<'f>> {
    items(parent, name)
        .iter()
        .filter_map(as_struct)
        .filter_map(property)
        .collect()
}

/// Returns the elements of the array the field `name` of `parent` holds, or the one struct the
/// field holds.
pub(crate) fn items<'t, 'f>(parent: Option<&'t StructValue<'f>>, name: &str) -> &'t [Value<'f>] {
    match parent.and_then(|parent| parent.field(name)) {
        Some(Value::Array(items)) => items,
        Some(single @ Value::Struct(_)) => std::slice::from_ref(single),
        _ => &[],
    }
}

/// Returns the first struct among [`items`].
pub(crate) fn one<'t, 'f>(
    parent: Option<&'t StructValue<'f>>,
    name: &str,
) -> Option<&'t StructValue<'f>> {
    items(parent, name).iter().find_map(as_struct)
}

/// Returns the localisation key the field `name` of `parent` holds, as a locale value or as a
/// string that starts with `@`; a placeholder key is none.
pub(crate) fn key<'f>(parent: Option<&StructValue<'f>>, name: &str) -> Option<&'f str> {
    match parent?.field(name)? {
        Value::Locale(key) | Value::String(key) if key.starts_with('@') && !key.is_empty() => {
            Some(*key).filter(|key| !is_placeholder_key(key))
        }
        _ => None,
    }
}

fn property<'f>(property: &StructValue<'f>) -> Option<Property<'f>> {
    let token = text(Some(property), "extendedTextToken")?;
    let value = one(Some(property), "value").map_or(Prop::Unknown, value);
    Some(Property { token, value })
}

fn value<'f>(value: &StructValue<'f>) -> Prop<'f> {
    match value.type_name() {
        STRING_HASH => items(Some(value), "options")
            .iter()
            .filter_map(as_struct)
            .find_map(|option| key(Some(option), "textId"))
            .map_or(Prop::Unknown, Prop::Key),
        ORGANIZATION => items(Some(value), "matchConditions")
            .iter()
            .filter_map(as_struct)
            .flat_map(|condition| items(Some(condition), "organizations"))
            .find_map(|organization| match organization {
                Value::Reference(reference) if reference.location().is_none() => {
                    Some(Prop::Organization(reference.guid()))
                }
                _ => None,
            })
            .unwrap_or(Prop::Unknown),
        "MissionPropertyValue_Location"
        | "MissionPropertyValue_Locations"
        | "MissionPropertyValue_ScopedLocation" => Prop::Placeholder("LOCATION"),
        "MissionPropertyValue_AIName" | "MissionPropertyValue_NPCSpawnDescriptions" => {
            Prop::Placeholder("TARGET")
        }
        "MissionPropertyValue_ShipSpawnDescriptions" => Prop::Placeholder("SHIP"),
        _ => Prop::Unknown,
    }
}

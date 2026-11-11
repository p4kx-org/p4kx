// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, StructValue, Value, is_placeholder_key};

use crate::blueprint::as_struct;
use crate::mission_hauling::{HaulingProperty, property_orders};
use crate::record_fields::{reference, text};

const CONTRACTOR: &str = "Contractor";

/// The mission type, contractor and hauling properties that one contract level overrides.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Layer<'f> {
    pub(crate) mission_type: Option<Guid>,
    pub(crate) contractor: Option<&'f str>,
    pub(crate) hauling: Vec<HaulingProperty<'f>>,
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
        Self {
            contractor: string_param(CONTRACTOR),
            mission_type: reference(params, "missionTypeOverride").map(|guid| guid.0),
            hauling: property_orders(params, "propertyOverrides"),
        }
    }
}

/// Returns the elements of the array the field `name` of `parent` holds, or the one struct the
/// field holds.
pub(crate) fn items<'t, 'f>(parent: Option<&'t StructValue<'f>>, name: &str) -> &'t [Value<'f>] {
    parent
        .and_then(|parent| parent.field(name))
        .map_or(&[], items_value)
}

pub(crate) fn items_value<'read, 'source>(value: &'read Value<'source>) -> &'read [Value<'source>] {
    match value {
        Value::Array(items) => items,
        single @ Value::Struct(_) => std::slice::from_ref(single),
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
    key_value(parent?.field(name)?)
}

/// Returns a non-placeholder localization key from the actual locale or string value.
pub(crate) fn key_value<'source>(value: &Value<'source>) -> Option<&'source str> {
    match value {
        Value::Locale(key) | Value::String(key) if key.starts_with('@') && !key.is_empty() => {
            Some(*key).filter(|key| !is_placeholder_key(key))
        }
        _ => None,
    }
}

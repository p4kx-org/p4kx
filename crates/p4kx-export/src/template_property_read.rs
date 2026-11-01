// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::StructValue;

use crate::blueprint::as_struct;
use crate::contract::LocalisedText;
use crate::mission_layers::{items, key, one};
use crate::record_fields::{boolean, integer, number, reference, text};
use crate::record_read::ReadError;
use crate::spawn_model::LocationCondition;
use crate::spawn_pools::{Registered, SpawnPools, Target};
use crate::spawn_read::Reader;
use crate::template_issues::{TemplateError, Unknowns};
use crate::template_property::{
    LinkedOutput, NumberOption, ObjectiveProperty, PropertyKind, PropertyValue, StringOption,
};
use crate::texts::Texts;

const VALUE_PREFIX: &str = "MissionPropertyValue_";

/// What the properties of the objectives of one Record are read with: its tags and weak pointers,
/// the texts, the count of unknown names and the pools the spawn descriptions and location queries
/// they state are registered in.
pub(crate) struct PropertyReader<'r, 'x, 'f> {
    pub(crate) reader: &'r Reader<'x, 'f>,
    pub(crate) texts: &'r Texts<'f>,
    pub(crate) unknowns: &'r Unknowns,
    pub(crate) pools: &'r RefCell<SpawnPools<'f>>,
}

impl<'f> PropertyReader<'_, '_, 'f> {
    /// Reads the `properties` of an objective token.
    ///
    /// # Errors
    ///
    /// Returns [`TemplateError::UnknownStruct`] for a value of a struct the catalog does not know
    /// and the error of the serialisation that makes the identifier of a description.
    pub(crate) fn properties(
        &self,
        token: &StructValue<'f>,
    ) -> Result<Vec<ObjectiveProperty<'f>>, ReadError> {
        items(Some(token), "properties")
            .iter()
            .filter_map(as_struct)
            .map(|property| self.property(property))
            .collect()
    }

    fn property(&self, property: &StructValue<'f>) -> Result<ObjectiveProperty<'f>, ReadError> {
        let parent = Some(property);
        let embedded = one(parent, "property");
        let named = embedded.or(parent);
        let owned = |name: &str| text(named, name).map(str::to_owned);
        let value = one(embedded, "value")
            .map(|value| self.value(value))
            .transpose()?;
        Ok(ObjectiveProperty {
            kind: self.unknowns.parse(
                "objective property",
                Some(property.type_name()),
                PropertyKind::parse,
            ),
            variable: owned("missionVariableName"),
            token: owned("extendedTextToken"),
            contract_property: self.reader.property_variable(parent, "property"),
            value,
            linked_output: self.reader.output_link(parent, "linkedOutput").map(
                |(objective_id, variable)| LinkedOutput {
                    objective_id,
                    variable,
                },
            ),
        })
    }

    fn value(&self, value: &StructValue<'f>) -> Result<PropertyValue<'f>, ReadError> {
        let parent = Some(value);
        let name = value.type_name();
        Ok(match name.strip_prefix(VALUE_PREFIX).unwrap_or(name) {
            "Object" => PropertyValue::Object,
            "DeliveryOrder" => PropertyValue::DeliveryOrder,
            "Boolean" => PropertyValue::Boolean {
                value: boolean(parent, "value"),
            },
            "StringHash" => PropertyValue::StringHash {
                options: Self::options(value, |option| StringOption {
                    display_name: self.text(option),
                    value: text(Some(option), "value").map(str::to_owned),
                    weighting: integer(Some(option), "weighting"),
                    dependent_properties: self
                        .reader
                        .property_variables(Some(option), "dependentProperties"),
                }),
            },
            "Integer" => PropertyValue::Integer {
                options: Self::options(value, |option| self.number_option(option)),
            },
            "Float" => PropertyValue::Float {
                options: Self::options(value, |option| self.number_option(option)),
            },
            "Tags" => PropertyValue::Tags {
                tags: self.reader.tag_list(parent, "tags"),
                negative_tags: self.reader.tag_list(parent, "negativeTags"),
            },
            "Organization" => return self.organization(value),
            _ => return self.spawn(value),
        })
    }

    /// Reads an organisation value.
    ///
    /// # Errors
    ///
    /// Returns [`TemplateError::UnknownStruct`] for a condition of a struct the catalog does not
    /// know.
    fn organization(&self, value: &StructValue<'f>) -> Result<PropertyValue<'f>, ReadError> {
        let conditions = self.reader.conditions(Some(value));
        if let Some(LocationCondition::Other { struct_name }) = conditions
            .iter()
            .find(|condition| matches!(condition, LocationCondition::Other { .. }))
        {
            return Err(TemplateError::UnknownStruct {
                field: "organization condition",
                name: struct_name.clone(),
            }
            .into());
        }
        Ok(PropertyValue::Organization {
            conditions,
            forced_organization_guid: reference(Some(value), "debugForceChosenOrganization"),
        })
    }

    /// Reads a spawn description or a location query and registers it.
    ///
    /// # Errors
    ///
    /// Returns [`TemplateError::UnknownStruct`] for a value of any other struct.
    fn spawn(&self, value: &StructValue<'f>) -> Result<PropertyValue<'f>, ReadError> {
        let Some(spawn) = self.reader.spawn(value)? else {
            return Err(TemplateError::unknown("objective property value", value).into());
        };
        let Registered { target, empty } = self.pools.borrow_mut().register(spawn);
        let pool_guid = |guid| (!empty).then_some(guid);
        Ok(match target {
            Target::Ships(guid) => PropertyValue::ShipSpawnDescriptions {
                pool_guid: pool_guid(guid),
            },
            Target::Npcs(guid) => PropertyValue::NpcSpawnDescriptions {
                pool_guid: pool_guid(guid),
            },
            Target::Entities(guid) => PropertyValue::EntitySpawnDescriptions {
                pool_guid: pool_guid(guid),
            },
            Target::Location(guid) => PropertyValue::Location { query_guid: guid },
        })
    }

    fn options<'v, T>(
        value: &'v StructValue<'f>,
        read: impl Fn(&'v StructValue<'f>) -> T,
    ) -> Vec<T> {
        items(Some(value), "options")
            .iter()
            .filter_map(as_struct)
            .map(read)
            .collect()
    }

    fn number_option(&self, option: &StructValue<'f>) -> NumberOption<'f> {
        NumberOption {
            display_name: self.text(option),
            value: number(Some(option), "value"),
            variation: number(Some(option), "variation"),
            weighting: integer(Some(option), "weighting"),
            dependent_properties: self
                .reader
                .property_variables(Some(option), "dependentProperties"),
        }
    }

    fn text(&self, option: &StructValue<'f>) -> LocalisedText<'f> {
        self.texts.localised(key(Some(option), "textId"))
    }
}

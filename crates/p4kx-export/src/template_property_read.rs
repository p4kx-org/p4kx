// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{StructValue, Value};

use crate::broker_terms::mission_reward;
use crate::mission_hauling::value_requirements;
use crate::mission_layers::one;
use crate::mission_property_fields as source_fields;
use crate::mission_property_set::MissionPropertyDeclaration;
use crate::mission_property_value_read;
use crate::record_fields::{boolean, integer, reference};
use crate::record_read::ReadError;
use crate::source_fields::{FieldCapture, unknown_struct};
use crate::spawn_pools::{Registered, SpawnPools, Target};
use crate::spawn_read::Reader;
use crate::template_property::PropertyValue;
use crate::template_property_kind::PropertyValueKind;
use crate::texts::Texts;
use crate::{CatalogError, ExportAccountingError};

const VALUE_PREFIX: &str = "MissionPropertyValue_";

/// Reads the authored Generator property-set values through their original guards.
pub(crate) struct PropertyReader<'r, 'x, 'f> {
    reader: &'r Reader<'x, 'f>,
    texts: &'r Texts<'f>,
    pools: &'r RefCell<SpawnPools<'f>>,
}

/// Returns the struct name of a property value without its `MissionPropertyValue_` start.
fn value_type<'v>(value: &'v StructValue<'_>) -> &'v str {
    let name = value.type_name();
    name.strip_prefix(VALUE_PREFIX).unwrap_or(name)
}

impl<'r, 'x, 'f> PropertyReader<'r, 'x, 'f> {
    /// Creates the reader of the Record `reader` belongs to.
    pub(crate) const fn new(
        reader: &'r Reader<'x, 'f>,
        texts: &'r Texts<'f>,
        pools: &'r RefCell<SpawnPools<'f>>,
    ) -> Self {
        Self {
            reader,
            texts,
            pools,
        }
    }
}

impl<'f> PropertyReader<'_, '_, 'f> {
    /// Reads ordered source declarations without normalising empty variable or token strings.
    ///
    /// # Errors
    ///
    /// Returns a source-field or strict value error instead of dropping a declaration.
    pub(crate) fn source_properties(
        &self,
        parent: &StructValue<'f>,
        values: &[Value<'f>],
    ) -> Result<Vec<Option<MissionPropertyDeclaration<'f>>>, ReadError> {
        let capture = self
            .reader
            .capture()
            .ok_or_else(|| CatalogError::from(ExportAccountingError::ConflictingSourceDatum))?;
        capture.check_parent(parent)?;
        values
            .iter()
            .map(|value| {
                let Some(property) =
                    source_fields::structure(parent, value, "propertyOverrides[]", capture)?
                else {
                    return Ok(None);
                };
                let value = source_fields::parameter(property, "value", capture)?;
                Ok(Some(MissionPropertyDeclaration {
                    variable: source_fields::text(property, "missionVariableName", capture)?,
                    token: source_fields::text(property, "extendedTextToken", capture)?,
                    value_type: value.map(|value| value_type(value).to_owned()),
                    value: value.map(|value| self.value(value)).transpose()?,
                }))
            })
            .collect()
    }

    fn value(&self, value: &StructValue<'f>) -> Result<PropertyValue<'f>, ReadError> {
        let capture = self.reader.capture();
        if let Some(value) =
            mission_property_value_read::read(self.reader, value, self.texts, capture)?
        {
            return Ok(value);
        }
        let parent = Some(value);
        Ok(match PropertyValueKind::read(value.type_name()) {
            Some(PropertyValueKind::ScopedLocation) => PropertyValue::ScopedLocation(Box::new(
                crate::scoped_location_read::read(self.reader, value, capture)?,
            )),
            Some(PropertyValueKind::Object) => PropertyValue::Object,
            Some(PropertyValueKind::DeliveryOrder) => PropertyValue::DeliveryOrder,
            Some(PropertyValueKind::Boolean) => PropertyValue::Boolean {
                value: boolean(parent, "value"),
            },
            Some(PropertyValueKind::Organization) => PropertyValue::Organization {
                conditions: self.reader.conditions(parent, "organization condition")?,
                forced_organization_guid: reference(parent, "debugForceChosenOrganization"),
            },
            Some(PropertyValueKind::MissionItem) => PropertyValue::MissionItem {
                conditions: self.reader.conditions(parent, "item condition")?,
                forced_item_guid: reference(parent, "debugForceChosenItem"),
                min_items_to_find: integer(parent, "minItemsToFind"),
                max_items_to_find: integer(parent, "maxItemsToFind"),
            },
            Some(PropertyValueKind::AiName) => {
                let mut name = self.reader.ai_name(value)?;
                name.resolve(self.texts);
                PropertyValue::AiName(name)
            }
            Some(PropertyValueKind::CombinedDataSetEntries) => {
                PropertyValue::CombinedDataSetEntries {
                    entry_properties: self
                        .reader
                        .property_variables(parent, "dataSetEntryProperties"),
                }
            }
            Some(PropertyValueKind::HaulingOrders) => PropertyValue::HaulingOrders {
                requirements: value_requirements(value),
            },
            Some(PropertyValueKind::Reward) => PropertyValue::Reward {
                reward: one(parent, "rewardDef").map(mission_reward),
            },
            Some(PropertyValueKind::StarMapLocation) => PropertyValue::StarMapLocation,
            _ => return self.spawn(value, capture),
        })
    }

    /// Reads a spawn description or a location query and registers it.
    ///
    /// # Errors
    ///
    /// Returns a source or unsupported-value error for an unrepresentable spawn value.
    fn spawn(
        &self,
        value: &StructValue<'f>,
        capture: Option<&dyn FieldCapture<'f>>,
    ) -> Result<PropertyValue<'f>, ReadError> {
        let Some(spawn) = self.reader.spawn(value)? else {
            return Err(unknown_struct(value, "property value", capture));
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
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, TagTree, Value};

use crate::blueprint::as_struct;
use crate::entity_spawn_model::EntitySpawnPool;
use crate::guid_text::CatalogGuid;
use crate::mission_layers::{items, one};
use crate::npc_spawn_model::NpcSpawnPool;
use crate::record_fields::{boolean, guid_value, integer, nonnil_guid, number, reference, text};
use crate::spawn_model::{
    LocationCondition, LocationCount, LocationQuery, ShipSpawnGroup, ShipSpawnOption,
    ShipSpawnPool, ShipSpawnSlot, TagPaths, TagTerm, content_guid,
};

const LOCATION: &str = "MissionPropertyValue_Location";
const LOCATIONS: &str = "MissionPropertyValue_Locations";
const SHIPS: &str = "MissionPropertyValue_ShipSpawnDescriptions";
const NPCS: &str = "MissionPropertyValue_NPCSpawnDescriptions";
const ENTITIES: &str = "MissionPropertyValue_EntitySpawnDescriptions";
const CONDITION_PREFIX: &str = "DataSetMatchCondition_";

/// What a mission property states.
#[derive(Debug)]
pub(crate) enum Spawn<'f> {
    /// A ship spawn description.
    Ships(ShipSpawnPool),
    /// An NPC spawn description.
    Npcs(NpcSpawnPool<'f>),
    /// An entity spawn description.
    Entities(EntitySpawnPool),
    /// A location query.
    Location(LocationQuery),
}

/// Reads the values of the mission properties of one Record: tags by their path and weak
/// pointers to other properties of the Record by their variable name.
pub(crate) struct Reader<'x, 'f> {
    tags: &'x TagTree,
    root: &'x StructValue<'f>,
}

impl<'x, 'f> Reader<'x, 'f> {
    /// Creates the reader of the Record whose root struct is `root`.
    pub(crate) const fn new(tags: &'x TagTree, root: &'x StructValue<'f>) -> Self {
        Self { tags, root }
    }

    /// Reads the value of a property, `None` for a value that is no spawn description and no
    /// location.
    ///
    /// # Errors
    ///
    /// Returns the error of the serialisation that makes the identifier of the content.
    pub(crate) fn spawn(
        &self,
        value: &StructValue<'f>,
    ) -> Result<Option<Spawn<'f>>, serde_json::Error> {
        Ok(Some(match value.type_name() {
            SHIPS => Spawn::Ships(self.ships(value)?),
            NPCS => Spawn::Npcs(self.npcs(value)?),
            ENTITIES => Spawn::Entities(self.entities(value)?),
            LOCATION => Spawn::Location(self.location(value, LocationCount::One)?),
            LOCATIONS => Spawn::Location(self.location(value, LocationCount::Many)?),
            _ => return Ok(None),
        }))
    }

    fn ships(&self, value: &StructValue<'f>) -> Result<ShipSpawnPool, serde_json::Error> {
        let allowed = boolean(Some(value), "allowedForMissionRestrictedDeliveries");
        let groups: Vec<ShipSpawnGroup> = items(Some(value), "spawnDescriptions")
            .iter()
            .filter_map(as_struct)
            .map(|group| ShipSpawnGroup {
                name: text(Some(group), "Name").map(str::to_owned),
                slots: items(Some(group), "ships")
                    .iter()
                    .filter_map(as_struct)
                    .map(|slot| ShipSpawnSlot {
                        options: items(Some(slot), "options")
                            .iter()
                            .filter_map(as_struct)
                            .map(|option| self.ship_option(option))
                            .collect(),
                    })
                    .collect(),
            })
            .collect();
        Ok(ShipSpawnPool {
            guid: content_guid(&(allowed, &groups))?,
            allowed_for_mission_restricted_deliveries: allowed,
            groups,
        })
    }

    fn ship_option(&self, option: &StructValue<'f>) -> ShipSpawnOption {
        ShipSpawnOption {
            weight: number(Some(option), "weight"),
            concurrent_amount: integer(Some(option), "concurrentAmount"),
            include_location_ai_spawn_tags: boolean(Some(option), "includeLocationAISpawnTags"),
            tags: self.tag_list(Some(option), "tags"),
            negative_tags: self.tag_list(Some(option), "negativeTags"),
            markup_tags: self.tag_list(Some(option), "markupTags"),
            entity_tags: self.tag_list(Some(option), "entityTags"),
            initial_damage_guid: reference(Some(option), "initialDamageSettings"),
            name_property_variable: self.property_variable(Some(option), "nameProperty"),
        }
    }

    fn location(
        &self,
        value: &StructValue<'f>,
        count: LocationCount,
    ) -> Result<LocationQuery, serde_json::Error> {
        let many = matches!(count, LocationCount::Many);
        let log = boolean(Some(value), "logErrorOnSearchFail");
        let min = many
            .then(|| integer(Some(value), "minLocationsToFind"))
            .flatten();
        let max = many
            .then(|| integer(Some(value), "maxLocationsToFind"))
            .flatten();
        let fail = many
            .then(|| boolean(Some(value), "failIfMinAmountNotFound"))
            .flatten();
        let resource_tags = self.tag_array(Some(value), "resourceTags");
        let conditions = self.conditions(Some(value));
        let content = (count, min, max, fail, log, &resource_tags, &conditions);
        Ok(LocationQuery {
            guid: content_guid(&content)?,
            count,
            min_locations: min,
            max_locations: max,
            fail_if_min_not_found: fail,
            log_error_on_search_fail: log,
            resource_tags,
            conditions,
        })
    }

    /// Reads the `matchConditions` of `parent`, a location query or an organisation value.
    pub(crate) fn conditions(&self, parent: Option<&StructValue<'f>>) -> Vec<LocationCondition> {
        items(parent, "matchConditions")
            .iter()
            .filter_map(as_struct)
            .map(|condition| self.condition(condition))
            .collect()
    }

    fn condition(&self, condition: &StructValue<'f>) -> LocationCondition {
        let parent = Some(condition);
        let name = condition.type_name();
        let owned = |field: &str| text(parent, field).map(str::to_owned);
        match name.strip_prefix(CONDITION_PREFIX).unwrap_or(name) {
            "TagSearch" => LocationCondition::TagSearch {
                tag_type: owned("tagType"),
                terms: items(parent, "tagSearch")
                    .iter()
                    .filter_map(as_struct)
                    .map(|term| TagTerm {
                        positive_tags: self.tag_array(Some(term), "positiveTags"),
                        negative_tags: self.tag_array(Some(term), "negativeTags"),
                    })
                    .collect(),
            },
            "ExcludeNearbyLocationsDef" => LocationCondition::ExcludeNearbyLocations {
                range_km: integer(parent, "exclusionRangeKm"),
                location_property: self.property_variable(parent, "locationProperty"),
            },
            "ExcludeDistantLocationsDef" => LocationCondition::ExcludeDistantLocations {
                range_km: integer(parent, "exclusionRangeKm"),
                location_property: self.property_variable(parent, "locationProperty"),
            },
            "ExcludeProperty" => LocationCondition::ExcludeProperty {
                excluded_property: self.property_variable(parent, "excludedProperty"),
            },
            "PropertyTags" => LocationCondition::PropertyTags {
                property_tag_type: owned("propertyTagType"),
                target_tag_type: owned("targetTagType"),
                search_for_any_tag: boolean(parent, "searchForAnyTag"),
                search_property: self.property_variable(parent, "searchProperty"),
                tag_filters: self.tag_array(parent, "tagFilters"),
            },
            "SpecificOrganizationsDef" => LocationCondition::SpecificOrganizations {
                organization_guids: crate::record_fields::references(parent, "organizations"),
            },
            "SpecificItemsDef" => LocationCondition::SpecificItems {
                item_guids: crate::record_fields::references(parent, "items"),
            },
            _ => LocationCondition::Other {
                struct_name: name.to_owned(),
            },
        }
    }

    /// Returns the tag paths of the `TagList` struct in the field `name`.
    pub(crate) fn tag_list(&self, parent: Option<&StructValue<'f>>, name: &str) -> TagPaths {
        self.tag_array(one(parent, name), "tags")
    }

    /// Returns the tag paths of the array of tag References in the field `name`.
    pub(crate) fn tag_array(&self, parent: Option<&StructValue<'f>>, name: &str) -> TagPaths {
        items(parent, name)
            .iter()
            .filter_map(|tag| self.path(tag))
            .collect()
    }

    /// Returns the tag path of the tag Reference in the field `name`.
    pub(crate) fn tag_path(&self, parent: Option<&StructValue<'f>>, name: &str) -> Option<String> {
        self.path(parent?.field(name)?)
    }

    /// Returns the path of the tag `value` links to; the GUID text for a tag the tag database
    /// does not hold.
    fn path(&self, value: &Value<'f>) -> Option<String> {
        match value {
            Value::Reference(target) if target.location().is_none() => Some(
                self.tags
                    .path_of(target.guid())
                    .map_or_else(|| target.guid().to_string(), str::to_owned),
            ),
            _ => None,
        }
    }

    /// Returns the `missionVariableName` of the property the weak pointer in the field `name`
    /// points to inside this Record.
    pub(crate) fn property_variable(
        &self,
        parent: Option<&StructValue<'f>>,
        name: &str,
    ) -> Option<String> {
        self.variable_of(parent?.field(name)?)
    }

    /// Returns the `missionVariableName` of every property the weak pointers in the array `name`
    /// point to inside this Record, in stored order; a pointer to nothing is left out.
    pub(crate) fn property_variables(
        &self,
        parent: Option<&StructValue<'f>>,
        name: &str,
    ) -> Vec<String> {
        items(parent, name)
            .iter()
            .filter_map(|pointer| self.variable_of(pointer))
            .collect()
    }

    /// Returns the `id` of the objective token the weak pointer in the field `name` points to
    /// inside this Record.
    pub(crate) fn objective_id(
        &self,
        parent: Option<&StructValue<'f>>,
        name: &str,
    ) -> Option<CatalogGuid> {
        let token = self.target(Self::place(parent?.field(name)?)?)?;
        guid_value(Some(token), "id")
            .and_then(nonnil_guid)
            .map(CatalogGuid)
    }

    /// Returns the objective token that holds the property the weak pointer in the field `name`
    /// points to, and the `missionVariableName` of the property, which an output property keeps in
    /// its `property`.
    pub(crate) fn output_link(
        &self,
        parent: Option<&StructValue<'f>>,
        name: &str,
    ) -> Option<(Option<CatalogGuid>, Option<String>)> {
        let place = Self::place(parent?.field(name)?)?;
        let output = self.target(place)?;
        let (token, _) = place.rsplit_once(".properties[")?;
        let id = self
            .target(token)
            .and_then(|token| guid_value(Some(token), "id"))
            .and_then(nonnil_guid)
            .map(CatalogGuid);
        let variable = text(one(Some(output), "property"), "missionVariableName");
        Some((id, variable.map(str::to_owned)))
    }

    fn variable_of(&self, pointer: &Value<'f>) -> Option<String> {
        let property = self.target(Self::place(pointer)?)?;
        text(Some(property), "missionVariableName").map(str::to_owned)
    }

    /// Returns where inside this Record the weak pointer `pointer` points.
    fn place<'v>(pointer: &'v Value<'f>) -> Option<&'v str> {
        let Value::Reference(target) = pointer else {
            return None;
        };
        let location = target.location()?;
        Some(location.as_str())
    }

    /// Returns the struct at `place` inside this Record.
    fn target(&self, place: &str) -> Option<&'x StructValue<'f>> {
        match place
            .split('.')
            .try_fold(Cursor::Struct(self.root), Cursor::step)?
        {
            Cursor::Struct(found) => Some(found),
            Cursor::Array(_) => None,
        }
    }
}

/// A position while following the text of a location: a struct or an array of a field.
enum Cursor<'t, 'f> {
    Struct(&'t StructValue<'f>),
    Array(&'t [Value<'f>]),
}

impl Cursor<'_, '_> {
    /// Follows `step`, a field name followed by any number of `[index]`.
    fn step(self, step: &str) -> Option<Self> {
        let (field, mut indices) = step.split_once('[').map_or((step, ""), |(field, rest)| {
            (field, rest.trim_end_matches(']'))
        });
        let Self::Struct(parent) = self else {
            return None;
        };
        let mut at = match parent.field(field)? {
            Value::Struct(value) => Self::Struct(value),
            Value::Array(values) => Self::Array(values),
            _ => return None,
        };
        while !indices.is_empty() {
            let (index, rest) = indices.split_once("][").unwrap_or((indices, ""));
            let Self::Array(values) = at else {
                return None;
            };
            at = match values.get(index.parse::<usize>().ok()?)? {
                Value::Struct(value) => Self::Struct(value),
                Value::Array(values) => Self::Array(values),
                _ => return None,
            };
            indices = rest;
        }
        Some(at)
    }
}

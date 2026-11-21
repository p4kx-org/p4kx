// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use super::Reader;
use crate::location_condition_kind::LocationConditionKind;
use crate::record_read::ReadError;
use crate::spawn_model::{self, LocationCondition, LocationCount, LocationQuery, TagTerm};

impl<'source> Reader<'_, 'source> {
    pub(super) fn location(
        &self,
        value: &StructValue<'source>,
        count: LocationCount,
    ) -> Result<LocationQuery, ReadError> {
        let fields = self.fields(Some(value));
        let many = matches!(count, LocationCount::Many);
        let log = fields.boolean("logErrorOnSearchFail")?;
        let min = if many {
            fields.integer("minLocationsToFind")?
        } else {
            None
        };
        let max = if many {
            fields.integer("maxLocationsToFind")?
        } else {
            None
        };
        let fail = if many {
            fields.boolean("failIfMinAmountNotFound")?
        } else {
            None
        };
        let resource_tags = fields.tag_array("resourceTags")?;
        let conditions = self.conditions(Some(value), "location condition")?;
        let content = (count, min, max, fail, log, &resource_tags, &conditions);
        Ok(LocationQuery {
            guid: spawn_model::content_guid(&content)?,
            count,
            min_locations: min,
            max_locations: max,
            fail_if_min_not_found: fail,
            log_error_on_search_fail: log,
            resource_tags,
            conditions,
        })
    }

    /// Reads strict location conditions without evaluating the dataset.
    ///
    /// # Errors
    /// Returns original context, strict-field or unknown-condition failures.
    pub(crate) fn conditions(
        &self,
        parent: Option<&StructValue<'source>>,
        what: &'static str,
    ) -> Result<Vec<LocationCondition>, ReadError> {
        self.fields(parent)
            .structs("matchConditions")?
            .map(|condition| self.condition(condition?, what))
            .collect()
    }

    fn condition(
        &self,
        condition: &StructValue<'source>,
        what: &'static str,
    ) -> Result<LocationCondition, ReadError> {
        let fields = self.fields(Some(condition));
        let name = fields.type_name()?;
        let kind = LocationConditionKind::read(name).ok_or_else(|| fields.unknown_type(what))?;
        Ok(match kind {
            LocationConditionKind::TagSearch => LocationCondition::TagSearch {
                tag_type: fields.owned("tagType")?,
                terms: fields
                    .structs("tagSearch")?
                    .map(|term| self.tag_term(term?))
                    .collect::<Result<_, _>>()?,
            },
            LocationConditionKind::ExcludeNearbyLocations => {
                LocationCondition::ExcludeNearbyLocations {
                    range_km: fields.strict_number("exclusionRangeKm", "matchConditions")?,
                    location_property: fields.variable("locationProperty")?,
                }
            }
            LocationConditionKind::ExcludeDistantLocations => {
                LocationCondition::ExcludeDistantLocations {
                    range_km: fields.strict_number("exclusionRangeKm", "matchConditions")?,
                    location_property: fields.variable("locationProperty")?,
                }
            }
            LocationConditionKind::ExcludeProperty => LocationCondition::ExcludeProperty {
                excluded_property: fields.variable("excludedProperty")?,
            },
            LocationConditionKind::PropertyTags => LocationCondition::PropertyTags {
                property_tag_type: fields.owned("propertyTagType")?,
                target_tag_type: fields.owned("targetTagType")?,
                search_for_any_tag: fields.boolean("searchForAnyTag")?,
                search_property: fields.variable("searchProperty")?,
                tag_filters: fields.tag_array("tagFilters")?,
            },
            LocationConditionKind::SpecificOrganizations => {
                LocationCondition::SpecificOrganizations {
                    organization_guids: fields.references("organizations")?,
                }
            }
            LocationConditionKind::SpecificItems => LocationCondition::SpecificItems {
                item_guids: fields.references("items")?,
            },
            LocationConditionKind::FactionIdentity => LocationCondition::FactionIdentity {
                faction_identity: fields.strict_reference("factionIdentity", "matchConditions")?,
            },
        })
    }

    fn tag_term(&self, term: &StructValue<'source>) -> Result<TagTerm, ReadError> {
        let fields = self.fields(Some(term));
        Ok(TagTerm {
            positive_tags: fields.tag_array("positiveTags")?,
            negative_tags: fields.tag_array("negativeTags")?,
        })
    }
}

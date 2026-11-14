// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value};

use crate::mission_property_source;
use crate::record_read::ReadError;
use crate::source_fields::{FieldCapture, FieldChecks, unknown_struct};
use crate::template_property_kind::PropertyValueKind;

/// Checks supported shapes on the same original frame as the actual C interpretation.
pub(crate) fn validate<'source>(
    source: &StructValue<'source>,
    kind: PropertyValueKind,
    capture: &dyn FieldCapture<'source>,
) -> Result<(), ReadError> {
    match kind {
        PropertyValueKind::AutoSpawns => {
            crate::mission_property_auto_read::validate_observed(source, capture)
        }
        PropertyValueKind::Investigation => {
            crate::mission_property_investigation_read::validate_observed(source, capture)
        }
        PropertyValueKind::TimeTrialRace => {
            crate::mission_property_race::validate_observed(source, capture)
        }
        PropertyValueKind::StringHash => options(
            source,
            capture,
            crate::mission_property_option_read::validate_string_observed,
        ),
        PropertyValueKind::Integer => options(
            source,
            capture,
            crate::mission_property_option_read::validate_integer_observed,
        ),
        PropertyValueKind::Float => options(
            source,
            capture,
            crate::mission_property_option_read::validate_float_observed,
        ),
        PropertyValueKind::Tags => tags(source, capture),
        PropertyValueKind::ScopedLocation => {
            crate::scoped_location_shape::validate_observed(source, capture)
        }
        PropertyValueKind::Organization => conditions(source, "organization condition", capture),
        PropertyValueKind::MissionItem => conditions(source, "item condition", capture),
        PropertyValueKind::Object
        | PropertyValueKind::DeliveryOrder
        | PropertyValueKind::Boolean
        | PropertyValueKind::AiName
        | PropertyValueKind::CombinedDataSetEntries
        | PropertyValueKind::HaulingOrders
        | PropertyValueKind::Reward
        | PropertyValueKind::StarMapLocation
        | PropertyValueKind::ShipSpawnDescriptions
        | PropertyValueKind::NpcSpawnDescriptions
        | PropertyValueKind::EntitySpawnDescriptions
        | PropertyValueKind::Location
        | PropertyValueKind::Locations => Ok(()),
    }
}

fn options<'source>(
    source: &StructValue<'source>,
    capture: &dyn FieldCapture<'source>,
    mut read: impl FnMut(
        &StructValue<'source>,
        &str,
        &dyn FieldCapture<'source>,
    ) -> Result<(), ReadError>,
) -> Result<(), ReadError> {
    mission_property_source::check_observed(source, "", &["options"], capture)?;
    mission_property_source::visit_observed(source, "options", "", capture, |value, path| {
        if let Some(value) = value {
            read(value, path, capture)?;
        }
        Ok(())
    })?;
    Ok(())
}

fn tags<'source>(
    source: &StructValue<'source>,
    capture: &dyn FieldCapture<'source>,
) -> Result<(), ReadError> {
    mission_property_source::check_observed(
        source,
        "",
        &["tags", "negativeTags", "options"],
        capture,
    )?;
    let fields = FieldChecks::for_context(source, "", Some(capture))?;
    mission_property_source::tags_shape_observed(fields.structure("tags")?, "tags", capture)?;
    mission_property_source::tags_shape_observed(
        fields.structure("negativeTags")?,
        "negativeTags",
        capture,
    )?;
    mission_property_source::visit_observed(source, "options", "", capture, |value, path| {
        if let Some(value) = value {
            crate::mission_property_option_read::validate_tags_observed(value, path, capture)?;
        }
        Ok(())
    })?;
    Ok(())
}

fn conditions<'source>(
    source: &StructValue<'source>,
    what: &'static str,
    capture: &dyn FieldCapture<'source>,
) -> Result<(), ReadError> {
    for value in crate::mission_layers::items(Some(source), "matchConditions") {
        let Value::Struct(condition) = value else {
            continue;
        };
        let kind =
            crate::location_condition_kind::LocationConditionKind::read(condition.type_name())
                .ok_or_else(|| unknown_struct(condition, what, Some(capture)))?;
        let fields = FieldChecks::for_context(condition, "matchConditions", Some(capture))?;
        match kind {
            crate::location_condition_kind::LocationConditionKind::ExcludeNearbyLocations
            | crate::location_condition_kind::LocationConditionKind::ExcludeDistantLocations => {
                fields.number("exclusionRangeKm")?;
            }
            crate::location_condition_kind::LocationConditionKind::FactionIdentity => {
                fields.reference_shape("factionIdentity")?;
            }
            _ => {}
        }
    }
    Ok(())
}

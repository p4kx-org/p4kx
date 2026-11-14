// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree, SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::prepared_definition::PreparedDefinition;
use super::starmap_display::DisplayFields;
use super::starmap_object_links::StarmapObjectLinks;
use super::starmap_object_owned::StarmapObjectOwned;
use super::starmap_visibility::VisibilityFields;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_text_contract::GameTextRef;
use crate::contract_starmap::StarmapObject;
use crate::record_observation::RecordObservation;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;
use crate::{CatalogError, ExportAccountingError};

pub(crate) const SOURCE_TYPE: &str = "StarMapObject";
const BOOL_OVERRIDE: &str = "StarmapBoolOverride";

/// Authored and projected facets of one admitted starmap Record.
pub(crate) struct StarmapObjectRead<'source> {
    pub(crate) definition: PreparedDefinition<'source, StarmapObjectDefinition<'source>>,
    pub(crate) object: StarmapObject<'source>,
}

pub(crate) fn read<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
    object: StarmapObject<'source>,
) -> Result<(StarmapObjectRead<'source>, Vec<FieldError>), CatalogError> {
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    let (definition, notes) = PreparedDefinition::read(
        tree,
        projection,
        admission,
        cancel,
        StarmapObjectDefinition::read,
    )?;
    Ok((StarmapObjectRead { definition, object }, notes))
}

/// One physical starmap object with its authored settings and relationships.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct StarmapObjectDefinition<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    #[serde(flatten)]
    text: ObjectText,
    #[serde(flatten)]
    links: StarmapObjectLinks<'source>,
    #[serde(flatten)]
    visibility: VisibilityFields,
    #[serde(flatten)]
    display: DisplayFields<'source>,
    #[serde(flatten)]
    navigation: ObjectNavigation<'source>,
    #[serde(flatten)]
    owned: StarmapObjectOwned<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> StarmapObjectDefinition<'source> {
    pub(super) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let text = ObjectText::read(fields, projection, notes)?;
        let links = StarmapObjectLinks::read(fields, notes)?;
        let visibility = VisibilityFields::read(fields, notes)?;
        let display = DisplayFields::read(fields, projection, notes)?;
        let navigation = ObjectNavigation::read(fields, notes)?;
        let owned = StarmapObjectOwned::read(fields, notes)?;
        let projected = text
            .projected()
            .into_iter()
            .chain(links.projected())
            .chain(visibility.projected())
            .chain(display.projected())
            .chain(navigation.projected())
            .chain(owned.projected());
        let authored_fields = fields.authored_members_except(projected)?;
        Ok(Self {
            source,
            text,
            links,
            visibility,
            display,
            navigation,
            owned,
            authored_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ObjectText {
    #[schemars(extend("x-gameText" = true))]
    name: SourcedValue<GameTextRef>,
    #[schemars(extend("x-gameText" = true))]
    callout1: SourcedValue<GameTextRef>,
    #[schemars(extend("x-gameText" = true))]
    callout2: SourcedValue<GameTextRef>,
    #[schemars(extend("x-gameText" = true))]
    callout3: SourcedValue<GameTextRef>,
}

impl ObjectText {
    fn read<'source>(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            name: fields.protected_locale_or_legacy_key("name", projection, notes)?,
            callout1: fields.protected_locale("callout1", projection, notes)?,
            callout2: fields.protected_locale("callout2", projection, notes)?,
            callout3: fields.protected_locale("callout3", projection, notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 4] {
        [
            ("name", self.name.state()),
            ("callout1", self.callout1.state()),
            ("callout2", self.callout2.state()),
            ("callout3", self.callout3.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ObjectNavigation<'source> {
    respawn_location_type: SourcedValue<&'source str>,
    is_scannable: SourcedValue<bool>,
    override_spawn_nav_points: SourcedValue<&'source str>,
    override_show_in_all_zones: SourcedValue<&'source str>,
    override_show_in_route_only: SourcedValue<&'source str>,
    override_permanent: SourcedValue<&'source str>,
    skip_collision_check_if_in_arrival_radius: SourcedValue<bool>,
    override_rotation_speed: SourcedValue<bool>,
    override_rotation_speed_value: SourcedValue<ResourceFloat32>,
    show_orbit_line: SourcedValue<bool>,
    no_auto_body_recovery: SourcedValue<bool>,
    location_image_path: SourcedValue<&'source str>,
}

impl<'source> ObjectNavigation<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            respawn_location_type: fields.borrowed_enum_to(
                "respawnLocationType",
                "ERespawnLocationType",
                notes,
            )?,
            is_scannable: fields.boolean("isScannable", notes)?,
            override_spawn_nav_points: fields.borrowed_enum_to(
                "overrideSpawnNavPoints",
                BOOL_OVERRIDE,
                notes,
            )?,
            override_show_in_all_zones: fields.borrowed_enum_to(
                "overrideShowInAllZones",
                BOOL_OVERRIDE,
                notes,
            )?,
            override_show_in_route_only: fields.borrowed_enum_to(
                "overrideShowInRouteOnly",
                BOOL_OVERRIDE,
                notes,
            )?,
            override_permanent: fields.borrowed_enum_to(
                "overridePermanent",
                BOOL_OVERRIDE,
                notes,
            )?,
            skip_collision_check_if_in_arrival_radius: fields
                .boolean("skipCollisionCheckIfInArrivalRadius", notes)?,
            override_rotation_speed: fields.boolean("overrideRotationSpeed", notes)?,
            override_rotation_speed_value: fields.single("overrideRotationSpeedValue", notes)?,
            show_orbit_line: fields.boolean("showOrbitLine", notes)?,
            no_auto_body_recovery: fields.boolean("noAutoBodyRecovery", notes)?,
            location_image_path: fields.borrowed_string("locationImagePath", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 12] {
        [
            ("respawnLocationType", self.respawn_location_type.state()),
            ("isScannable", self.is_scannable.state()),
            (
                "overrideSpawnNavPoints",
                self.override_spawn_nav_points.state(),
            ),
            (
                "overrideShowInAllZones",
                self.override_show_in_all_zones.state(),
            ),
            (
                "overrideShowInRouteOnly",
                self.override_show_in_route_only.state(),
            ),
            ("overridePermanent", self.override_permanent.state()),
            (
                "skipCollisionCheckIfInArrivalRadius",
                self.skip_collision_check_if_in_arrival_radius.state(),
            ),
            (
                "overrideRotationSpeed",
                self.override_rotation_speed.state(),
            ),
            (
                "overrideRotationSpeedValue",
                self.override_rotation_speed_value.state(),
            ),
            ("showOrbitLine", self.show_orbit_line.state()),
            ("noAutoBodyRecovery", self.no_auto_body_recovery.state()),
            ("locationImagePath", self.location_image_path.state()),
        ]
    }
}

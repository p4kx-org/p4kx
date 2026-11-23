// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    CancelToken, FieldOrigin, RecordTree, SourceUseRole, SourceValueState, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::prepared_definition::PreparedDefinition;
pub use super::starmap_display::{DisplayFields, StarmapAssetManager};
pub use super::starmap_navigation::{StarmapLocation, StarmapQuantumTravel};
pub use super::starmap_object_links::StarmapObjectLinks;
pub use super::starmap_object_owned::{StarmapAsteroidRing, StarmapObjectOwned};
pub use super::starmap_radar::{RadarProperties, RadarRoomParams};
pub use super::starmap_visibility::VisibilityFields;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::GameTextRef;
use crate::contract_starmap::{ShopLinkHeader, StarmapObject};
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
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
    project_object: impl FnOnce(SourcedValue<GameTextRef>) -> StarmapObject<'source>,
) -> Result<(StarmapObjectRead<'source>, Vec<FieldError>), CatalogError> {
    let mut marker = None;
    let (definition, notes) = prepare(tree, projection, admission, cancel, Some(&mut marker))?;
    let mut object = project_object(definition.definition().text.name.clone());
    let marker = marker.ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    if marker.value().copied().unwrap_or(false) != object.mining_claim {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    object.location_origin = Some(location_origin(tree, projection, marker.origin())?);
    let header = &definition.definition().source;
    object.shop_link_header = Some(ShopLinkHeader {
        guid: header.guid.origin().clone(),
        class_name: header.class_name().origin().clone(),
    });
    Ok((StarmapObjectRead { definition, object }, notes))
}

fn location_origin(
    tree: &RecordTree<'_>,
    projection: &CatalogProjection<'_, '_>,
    marker: &FieldOrigin,
) -> Result<FieldOrigin, CatalogError> {
    let origins = projection.origins();
    let record = origins.record(tree.record_index());
    let mut uses = Vec::new();
    for field in [
        RecordHeaderField::Guid,
        RecordHeaderField::SourceType,
        RecordHeaderField::RecordName,
    ] {
        record.append_header_input(
            RecordHeaderInput::Field(field),
            SourceUseRole::Selector,
            &mut uses,
        )?;
    }
    record.append_header_input(
        RecordHeaderInput::NormalizedPath,
        SourceUseRole::Selector,
        &mut uses,
    )?;
    let membership = origins.uses.origin(&uses, true);
    Ok(origins
        .uses
        .origin_with_origins([&membership, marker], true)?)
}

pub(super) type PreparedStarmapObject<'source> =
    PreparedDefinition<'source, StarmapObjectDefinition<'source>>;

pub(super) fn prepare<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
    marker: Option<&mut Option<SourcedValue<bool>>>,
) -> Result<(PreparedStarmapObject<'source>, Vec<FieldError>), CatalogError> {
    if tree.type_name() != SOURCE_TYPE {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    PreparedDefinition::read(
        tree,
        projection,
        admission,
        cancel,
        |source, fields, projection, notes| {
            if let Some(marker) = marker {
                *marker = Some(fields.predicate("description", crate::starmap::is_mining_claim)?);
            }
            StarmapObjectDefinition::read(source, fields, projection, notes)
        },
    )
}

pub(crate) struct AuthoredNavigation<'source> {
    pub(crate) source: CanonicalRecordHeader<'source>,
    pub(crate) name: SourcedValue<GameTextRef>,
    pub(crate) parent: SourcedValue<CatalogRecordReference>,
    pub(crate) unreadable_name_fields: usize,
}

pub(crate) fn read_navigation<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(AuthoredNavigation<'source>, Vec<FieldError>), CatalogError> {
    let (prepared, notes) = PreparedDefinition::read(
        tree,
        projection,
        admission,
        cancel,
        |source, fields, projection, notes| {
            let name = ObjectText::name(fields, projection, notes)?;
            let unreadable_name_fields = notes.len();
            let parent =
                StarmapObjectLinks::parent(fields, notes)?.map(|link| (*link).into_catalog());
            Ok(AuthoredNavigation {
                source,
                name,
                parent,
                unreadable_name_fields,
            })
        },
    )?;
    Ok((prepared.into_definition(), notes))
}

/// One physical starmap object with its authored settings and relationships.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct StarmapObjectDefinition<'source> {
    #[serde(flatten)]
    /// The physical Record header.
    pub source: CanonicalRecordHeader<'source>,
    #[serde(flatten)]
    /// The original protected name and callout references.
    pub text: ObjectText,
    #[serde(flatten)]
    /// The original starmap Record references.
    pub links: StarmapObjectLinks<'source>,
    #[serde(flatten)]
    /// The authored visibility and travel flags.
    pub visibility: VisibilityFields,
    #[serde(flatten)]
    /// The authored starmap presentation settings.
    pub display: DisplayFields<'source>,
    #[serde(flatten)]
    /// The authored navigation settings.
    pub navigation: ObjectNavigation<'source>,
    #[serde(flatten)]
    /// The original settings defined inside this Record.
    pub owned: StarmapObjectOwned<'source>,
    /// Additional original fields.
    pub authored_fields: SourceArenaOutput<'source>,
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

/// Original starmap object text references.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct ObjectText {
    #[schemars(extend("x-gameText" = true))]
    /// The original protected object name.
    pub name: SourcedValue<GameTextRef>,
    #[schemars(extend("x-gameText" = true))]
    /// The first original protected callout.
    pub callout1: SourcedValue<GameTextRef>,
    #[schemars(extend("x-gameText" = true))]
    /// The second original protected callout.
    pub callout2: SourcedValue<GameTextRef>,
    #[schemars(extend("x-gameText" = true))]
    /// The third original protected callout.
    pub callout3: SourcedValue<GameTextRef>,
}

impl ObjectText {
    fn name<'source>(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<GameTextRef>, CatalogError> {
        fields.protected_locale_or_legacy_key("name", projection, notes)
    }

    fn read<'source>(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            name: Self::name(fields, projection, notes)?,
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

/// Authored navigation settings of one starmap object.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct ObjectNavigation<'source> {
    /// The original respawn type token.
    pub respawn_location_type: SourcedValue<&'source str>,
    /// Whether the object is authored as scannable.
    pub is_scannable: SourcedValue<bool>,
    /// The original nav-point spawn override token.
    pub override_spawn_nav_points: SourcedValue<&'source str>,
    /// The original all-zone visibility override token.
    pub override_show_in_all_zones: SourcedValue<&'source str>,
    /// The original route-only visibility override token.
    pub override_show_in_route_only: SourcedValue<&'source str>,
    /// The original permanence override token.
    pub override_permanent: SourcedValue<&'source str>,
    /// Whether to skip collision checks within the arrival radius.
    pub skip_collision_check_if_in_arrival_radius: SourcedValue<bool>,
    /// Whether to override rotation speed.
    pub override_rotation_speed: SourcedValue<bool>,
    /// The authored rotation speed override value.
    pub override_rotation_speed_value: SourcedValue<ResourceFloat32>,
    /// Whether to display an orbit line.
    pub show_orbit_line: SourcedValue<bool>,
    /// Whether to disable automatic body recovery.
    pub no_auto_body_recovery: SourcedValue<bool>,
    /// The authored location image path.
    pub location_image_path: SourcedValue<&'source str>,
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

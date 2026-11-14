// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_unit_source::UnitSource;
use super::interaction_conditions::InteractionConditions;
use super::interaction_source::{RecordLink, RecordLinks, original};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::GameTextRef;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const SOURCE_TYPE: &str = "SSharedInteractionParams";

/// One physical shared interaction and its authored selectors.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct SharedInteraction<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(flatten)]
    identity: SharedInteractionIdentity<'source>,
    #[serde(flatten)]
    flags: SharedInteractionFlags,
    condition_list: SourcedValue<InteractionConditions<'source>>,
    player_animated_interaction_base: SourcedValue<UnitSource<'source>>,
}

impl<'source> SharedInteraction<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let identity = SharedInteractionIdentity::read(source, projection, notes)?;
        let flags = SharedInteractionFlags::read(source, notes)?;
        let condition_list = source.strong_pointer_family_to(
            "conditionList",
            "InteractionConditionList",
            notes,
            InteractionConditions::read,
        )?;
        let player_animated_interaction_base = source.strong_pointer_family_to(
            "playerAnimatedInteractionBase",
            "PlayerAnimatedInteractionBase",
            notes,
            original,
        )?;
        let projected = identity
            .members()
            .into_iter()
            .chain(flags.members())
            .chain([
                ("conditionList", condition_list.state()),
                (
                    "playerAnimatedInteractionBase",
                    player_animated_interaction_base.state(),
                ),
            ]);
        let source = UnitSource::read(source, projected)?;
        Ok(Self {
            source,
            identity,
            flags,
            condition_list,
            player_animated_interaction_base,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct SharedInteractionIdentity<'source> {
    name: SourcedValue<&'source str>,
    #[schemars(extend("x-gameText" = true))]
    display_name: SourcedValue<GameTextRef>,
    display_type: SourcedValue<&'source str>,
    generic_cursor: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    query_tags: RecordLinks<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    room_tag: RecordLink<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    usable_tag: RecordLink<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    linking_tag: RecordLink<'source>,
}

impl<'source> SharedInteractionIdentity<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            name: source.borrowed_string("Name", notes)?,
            display_name: source.protected_locale("DisplayName", projection, notes)?,
            display_type: source.borrowed_enum_to(
                "DisplayType",
                "InnerThoughtDisplayType",
                notes,
            )?,
            generic_cursor: source.borrowed_enum_to(
                "GenericCursor",
                "InteractionGenericCursor",
                notes,
            )?,
            query_tags: source.record_link_array_to("QueryTags", "Tag", notes)?,
            room_tag: source.record_link_to("RoomTag", "Tag", notes)?,
            usable_tag: source.record_link_to("UsableTag", "Tag", notes)?,
            linking_tag: source.record_link_to("LinkingTag", "Tag", notes)?,
        })
    }

    const fn members(&self) -> [(&'static str, SourceValueState); 8] {
        [
            ("Name", self.name.state()),
            ("DisplayName", self.display_name.state()),
            ("DisplayType", self.display_type.state()),
            ("GenericCursor", self.generic_cursor.state()),
            ("QueryTags", self.query_tags.state()),
            ("RoomTag", self.room_tag.state()),
            ("UsableTag", self.usable_tag.state()),
            ("LinkingTag", self.linking_tag.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct SharedInteractionFlags {
    focus_mode_only: SourcedValue<bool>,
    sendable: SourcedValue<bool>,
    linkable: SourcedValue<bool>,
    locked_by_links: SourcedValue<bool>,
    requires_authorized_user: SourcedValue<bool>,
    available: SourcedValue<bool>,
    inherits_condition_displays_through_links: SourcedValue<bool>,
}

impl SharedInteractionFlags {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            focus_mode_only: source.boolean("FocusModeOnly", notes)?,
            sendable: source.boolean("Sendable", notes)?,
            linkable: source.boolean("Linkable", notes)?,
            locked_by_links: source.boolean("LockedByLinks", notes)?,
            requires_authorized_user: source.boolean("RequiresAuthorizedUser", notes)?,
            available: source.boolean("available", notes)?,
            inherits_condition_displays_through_links: source
                .boolean("InheritsConditionDisplaysThroughLinks", notes)?,
        })
    }

    const fn members(&self) -> [(&'static str, SourceValueState); 7] {
        [
            ("FocusModeOnly", self.focus_mode_only.state()),
            ("Sendable", self.sendable.state()),
            ("Linkable", self.linkable.state()),
            ("LockedByLinks", self.locked_by_links.state()),
            (
                "RequiresAuthorizedUser",
                self.requires_authorized_user.state(),
            ),
            ("available", self.available.state()),
            (
                "InheritsConditionDisplaysThroughLinks",
                self.inherits_condition_displays_through_links.state(),
            ),
        ]
    }
}

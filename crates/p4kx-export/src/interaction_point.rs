// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_unit_source::UnitSource;
use super::interaction_source::{InteractionList, RecordLink, RecordLinks, WeakLink, original};
use super::shared_interaction;
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::GameTextRef;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const SOURCE_TYPE: &str = "SInteractionPointParams";

/// One authored interaction point with its exact links and original presentation fields.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InteractionPoint<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(flatten)]
    links: PointLinks<'source>,
    #[serde(flatten)]
    geometry: PointGeometry<'source>,
}

impl<'source> InteractionPoint<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let links = PointLinks::read(source, projection, notes)?;
        let geometry = PointGeometry::read(source, notes)?;
        let source = UnitSource::read(
            source,
            links.members().into_iter().chain(geometry.members()),
        )?;
        Ok(Self {
            source,
            links,
            geometry,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct PointLinks<'source> {
    interactions: InteractionList<InteractionPointLink<'source>>,
    helper_name: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    tags: RecordLinks<'source>,
    player_animated_interaction_params: SourcedValue<UnitSource<'source>>,
    highlight_behaviors: InteractionList<UnitSource<'source>>,
    #[serde(rename = "interactionPointModifiers")]
    modifiers: InteractionList<UnitSource<'source>>,
    #[serde(rename = "interactionPointInternalSettingTemplateRef")]
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    internal_setting_template_ref: RecordLink<'source>,
}

impl<'source> PointLinks<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            interactions: source.strong_pointer_family_array_to(
                "Interactions",
                "SInteractionParams",
                notes,
                |source, notes| InteractionPointLink::read(source, projection, notes),
            )?,
            helper_name: source.borrowed_string("HelperName", notes)?,
            tags: source.record_link_array_to("Tags", "Tag", notes)?,
            player_animated_interaction_params: source.strong_pointer_family_to(
                "PlayerAnimatedInteractionParams",
                "PlayerAnimatedInteractionBase",
                notes,
                original,
            )?,
            highlight_behaviors: source.class_array_to(
                "HighlightBehaviors",
                "SHighlightBehaviorNode",
                notes,
                original,
            )?,
            modifiers: source.strong_pointer_family_array_to(
                "InteractionPointModifiers",
                "SInteractionPointModifier",
                notes,
                original,
            )?,
            internal_setting_template_ref: source.record_link_to(
                "InteractionPointInternalSettingTemplateRef",
                "InteractionPointInternalSettingTemplate",
                notes,
            )?,
        })
    }

    const fn members(&self) -> [(&'static str, SourceValueState); 7] {
        [
            ("Interactions", self.interactions.state()),
            ("HelperName", self.helper_name.state()),
            ("Tags", self.tags.state()),
            (
                "PlayerAnimatedInteractionParams",
                self.player_animated_interaction_params.state(),
            ),
            ("HighlightBehaviors", self.highlight_behaviors.state()),
            ("InteractionPointModifiers", self.modifiers.state()),
            (
                "InteractionPointInternalSettingTemplateRef",
                self.internal_setting_template_ref.state(),
            ),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct PointGeometry<'source> {
    primitive: SourcedValue<UnitSource<'source>>,
    offset: SourcedValue<UnitSource<'source>>,
    interaction_prompt_offset: SourcedValue<UnitSource<'source>>,
    angle_constraint: SourcedValue<UnitSource<'source>>,
}

impl<'source> PointGeometry<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            primitive: source.strong_pointer_family_to(
                "Primitive",
                "SInteractionPointPrimitiveParams",
                notes,
                original,
            )?,
            offset: source.declared_class_to("Offset", "QuatT", notes, original)?,
            interaction_prompt_offset: source.declared_class_to(
                "InteractionPromptOffset",
                "QuatT",
                notes,
                original,
            )?,
            angle_constraint: source.declared_class_to(
                "AngleConstraint",
                "SAngleConstraint",
                notes,
                original,
            )?,
        })
    }

    const fn members(&self) -> [(&'static str, SourceValueState); 4] {
        [
            ("Primitive", self.primitive.state()),
            ("Offset", self.offset.state()),
            (
                "InteractionPromptOffset",
                self.interaction_prompt_offset.state(),
            ),
            ("AngleConstraint", self.angle_constraint.state()),
        ]
    }
}

/// A point's original `WeakPointer` to a shared interaction.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct InteractionPointLink<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    interaction: WeakLink<'source>,
    #[schemars(extend("x-gameText" = true))]
    display_name: SourcedValue<GameTextRef>,
    is_primary: SourcedValue<bool>,
}

impl<'source> InteractionPointLink<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let interaction =
            source.weak_link_to("Interaction", shared_interaction::SOURCE_TYPE, notes)?;
        let display_name = source.protected_locale("DisplayName", projection, notes)?;
        let is_primary = source.boolean("IsPrimary", notes)?;
        let source = UnitSource::read(
            source,
            [
                ("Interaction", interaction.state()),
                ("DisplayName", display_name.state()),
                ("IsPrimary", is_primary.state()),
            ],
        )?;
        Ok(Self {
            source,
            interaction,
            display_name,
            is_primary,
        })
    }
}

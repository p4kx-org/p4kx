// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::{
    CatalogError, CatalogRecordReference, FieldError, Observed, ResourceNumber, Slots,
    SourceRecordReference, UnitSource, complete_slots, every_field_required,
};

type CategoryLinks<'source> = Slots<Box<SourceRecordReference<'source>>>;

/// Ordered category references of a queue whitelist or a modifier condition.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CrafterCategoryRule<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    kind: CrafterCategoryRuleKind,
    #[schemars(with = "Option<SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>>")]
    categories: Option<CategoryLinks<'source>>,
}

/// The interpretation of a concrete authored category rule.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum CrafterCategoryRuleKind {
    /// A queue accepts its listed categories.
    Whitelist,
    /// A modifier applies to its listed categories.
    BlueprintCategory,
    /// The concrete rule remains available in original form.
    Original,
}

/// One original crafting modifier and its independently authored condition.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CrafterModifier<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    modifier_type: SourcedValue<CrafterModifierType<'source>>,
    condition: SourcedValue<CrafterCategoryRule<'source>>,
}

/// The concrete original modifier and its understood interpretation.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CrafterModifierType<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(flatten)]
    kind: CrafterModifierKind,
}

/// The interpretation of the original modifier type.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum CrafterModifierKind {
    /// Applies the authored time multiplier.
    Time {
        /// The authored multiplier in its original numeric encoding.
        time_multiplier: SourcedValue<ResourceNumber>,
    },
    /// A different concrete modifier retained in original form.
    Original,
}

impl<'source> CrafterCategoryRule<'source> {
    /// Borrows the concrete rule type and additional original members.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    /// Returns the classification of the original rule.
    #[must_use]
    pub const fn kind(&self) -> CrafterCategoryRuleKind {
        self.kind
    }

    /// Borrows the original category slots for a known rule.
    #[must_use]
    pub const fn categories(&self) -> Option<&CategoryLinks<'source>> {
        self.categories.as_ref()
    }

    pub(super) fn read_whitelist(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Self::read(
            fields,
            notes,
            "BlueprintCategoryAvailability_Whitelist",
            CrafterCategoryRuleKind::Whitelist,
        )
    }

    fn read_condition(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Self::read(
            fields,
            notes,
            "CraftingModifierCondition_BlueprintCategory",
            CrafterCategoryRuleKind::BlueprintCategory,
        )
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
        expected: &str,
        kind: CrafterCategoryRuleKind,
    ) -> Result<Self, CatalogError> {
        if fields.source_type()? != expected {
            fields.note_retained_variant(notes)?;
            return Ok(Self {
                source: UnitSource::read(fields, [])?,
                kind: CrafterCategoryRuleKind::Original,
                categories: None,
            });
        }
        let categories = fields.record_link_array("categories", notes)?;
        let source = UnitSource::read(fields, [("categories", complete_slots(&categories))])?;
        Ok(Self {
            source,
            kind,
            categories: Some(categories),
        })
    }
}

impl<'source> CrafterModifier<'source> {
    /// Borrows the modifier's concrete type and additional authored members.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    /// Borrows the original modifier type and its source state.
    #[must_use]
    pub const fn modifier_type(&self) -> &SourcedValue<CrafterModifierType<'source>> {
        &self.modifier_type
    }

    /// Borrows the original modifier condition and its source state.
    #[must_use]
    pub const fn condition(&self) -> &SourcedValue<CrafterCategoryRule<'source>> {
        &self.condition
    }

    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let modifier_type =
            fields.declared_owned("modifierType", notes, CrafterModifierType::read)?;
        let condition = fields.declared_owned(
            "modifierCondition",
            notes,
            CrafterCategoryRule::read_condition,
        )?;
        let source = UnitSource::read(
            fields,
            [
                ("modifierType", modifier_type.state()),
                ("modifierCondition", condition.state()),
            ],
        )?;
        Ok(Self {
            source,
            modifier_type,
            condition,
        })
    }
}

impl<'source> CrafterModifierType<'source> {
    /// Borrows the concrete modifier type and additional original members.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    /// Borrows the supported interpretation of the concrete modifier.
    #[must_use]
    pub const fn kind(&self) -> &CrafterModifierKind {
        &self.kind
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        if fields.source_type()? == "CraftingModifierType_Time" {
            let time_multiplier = fields.authored_number("timeMultiplier", notes)?;
            let source = UnitSource::read(fields, [("timeMultiplier", time_multiplier.state())])?;
            Ok(Self {
                source,
                kind: CrafterModifierKind::Time { time_multiplier },
            })
        } else {
            fields.note_retained_variant(notes)?;
            Ok(Self {
                source: UnitSource::read(fields, [])?,
                kind: CrafterModifierKind::Original,
            })
        }
    }
}

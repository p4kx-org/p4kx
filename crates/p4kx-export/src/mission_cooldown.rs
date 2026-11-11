// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourceUses, SourceValue, SourcedValue, StructValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_read::original::OriginalFlowFields;
use crate::contract::every_field_required;
use crate::contract_facts::{FactLayer, Stated};
use crate::mission_facts::choice::{Choice, extend, no_value};
use crate::mission_facts::source::{Field, ParentLayers, Reader};

const PERSONAL_FLAG: Field<bool> = Field::flag("hasPersonalCooldown");

const PERSONAL_TIME: &str = "personalCooldownTime";
const PERSONAL_TIME_VARIATION: &str = "personalCooldownTimeVariation";
const ABANDONED_TIME: &str = "abandonedCooldownTime";
const ABANDONED_TIME_VARIATION: &str = "abandonedCooldownTimeVariation";

/// The selected source level of each cooldown field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct CooldownLayers {
    /// The level of `hasPersonalCooldown`.
    pub(crate) has_personal_cooldown: SourcedValue<FactLayer>,
    /// The level of `personalCooldownTime`.
    pub(crate) personal_cooldown_time: SourcedValue<FactLayer>,
    /// The level of `personalCooldownTimeVariation`.
    pub(crate) personal_cooldown_time_variation: SourcedValue<FactLayer>,
    /// The level of `abandonedCooldownTime`.
    pub(crate) abandoned_cooldown_time: SourcedValue<FactLayer>,
    /// The level of `abandonedCooldownTimeVariation`.
    pub(crate) abandoned_cooldown_time_variation: SourcedValue<FactLayer>,
}

/// The resolved cooldown values and level decisions for a contract occurrence.
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ContractCooldown {
    /// The nearest selected level among the five cooldown fields.
    pub(crate) cooldown_layer: SourcedValue<FactLayer>,
    /// The level that states each of the five fields.
    pub(crate) cooldown_layers: CooldownLayers,
    /// Whether the contract has a personal cooldown.
    pub(crate) has_personal_cooldown: SourcedValue<bool>,
    /// The personal cooldown time in the original unit.
    pub(crate) personal_cooldown_time: SourcedValue<f32>,
    /// The personal cooldown variation in the original unit.
    pub(crate) personal_cooldown_time_variation: SourcedValue<f32>,
    /// The cooldown after abandoning the contract, in the original unit.
    pub(crate) abandoned_cooldown_time: SourcedValue<f32>,
    /// The abandoned cooldown variation in the original unit.
    pub(crate) abandoned_cooldown_time_variation: SourcedValue<f32>,
}

/// The inherited choices of one original contract and the facts needed to read its sub contracts.
pub(crate) struct PreparedCooldown<'fields, 'scope, 'read, 'source> {
    reader: Reader<'fields, 'scope, 'read, 'source>,
    parent: Choices,
}

struct Choices {
    has_personal: Choice<bool>,
    personal: Choice<f32>,
    personal_variation: Choice<f32>,
    abandoned: Choice<f32>,
    abandoned_variation: Choice<f32>,
}

impl<'fields, 'scope, 'read, 'source> PreparedCooldown<'fields, 'scope, 'read, 'source> {
    pub(crate) fn read(
        fields: &'fields OriginalFlowFields<'scope, 'read, 'source>,
        handler: &StructValue<'source>,
        contract: &StructValue<'source>,
    ) -> Result<Self, CatalogError> {
        let reader = Reader::new(fields);
        let mut layers = ParentLayers::new(handler, contract);
        let has_personal = reader.parent(&PERSONAL_FLAG, &mut layers)?;
        let mut time = |name| reader.parent(&Field::duration(name), &mut layers);
        let parent = Choices {
            has_personal,
            personal: time(PERSONAL_TIME)?,
            personal_variation: time(PERSONAL_TIME_VARIATION)?,
            abandoned: time(ABANDONED_TIME)?,
            abandoned_variation: time(ABANDONED_TIME_VARIATION)?,
        };
        Ok(Self { reader, parent })
    }

    pub(crate) fn resolve(
        &self,
        sub: Option<&StructValue<'source>>,
    ) -> Result<ContractCooldown, CatalogError> {
        let sources = self.reader.sources();
        let Some(sub) = sub else {
            return self.parent.bind(sources);
        };
        let layer = self.reader.sub(sub)?;
        let parent = &self.parent;
        let time = |name, inherited| self.reader.child(&Field::duration(name), &layer, inherited);
        Choices {
            has_personal: self
                .reader
                .child(&PERSONAL_FLAG, &layer, &parent.has_personal)?,
            personal: time(PERSONAL_TIME, &parent.personal)?,
            personal_variation: time(PERSONAL_TIME_VARIATION, &parent.personal_variation)?,
            abandoned: time(ABANDONED_TIME, &parent.abandoned)?,
            abandoned_variation: time(ABANDONED_TIME_VARIATION, &parent.abandoned_variation)?,
        }
        .bind(sources)
    }
}

impl Choices {
    fn bind(&self, sources: &SourceUses) -> Result<ContractCooldown, CatalogError> {
        let mut summary = Summary::new();
        let has_personal = summary.bind_choice(&self.has_personal, sources)?;
        let personal = summary.bind_choice(&self.personal, sources)?;
        let personal_variation = summary.bind_choice(&self.personal_variation, sources)?;
        let abandoned = summary.bind_choice(&self.abandoned, sources)?;
        let abandoned_variation = summary.bind_choice(&self.abandoned_variation, sources)?;
        Ok(ContractCooldown {
            cooldown_layer: summary.bind(sources),
            cooldown_layers: CooldownLayers {
                has_personal_cooldown: has_personal.layer,
                personal_cooldown_time: personal.layer,
                personal_cooldown_time_variation: personal_variation.layer,
                abandoned_cooldown_time: abandoned.layer,
                abandoned_cooldown_time_variation: abandoned_variation.layer,
            },
            has_personal_cooldown: has_personal.value,
            personal_cooldown_time: personal.value,
            personal_cooldown_time_variation: personal_variation.value,
            abandoned_cooldown_time: abandoned.value,
            abandoned_cooldown_time_variation: abandoned_variation.value,
        })
    }
}

struct Summary {
    layer: Option<FactLayer>,
    selectors: Vec<SourceUse>,
    complete: bool,
}

impl Summary {
    const fn new() -> Self {
        Self {
            layer: None,
            selectors: Vec::new(),
            complete: true,
        }
    }

    fn bind_choice<T: Copy>(
        &mut self,
        choice: &Choice<T>,
        sources: &SourceUses,
    ) -> Result<Stated<T>, CatalogError> {
        let (layer, selectors, complete) = choice.decision();
        self.include(layer, complete, selectors)?;
        choice.bind(sources)
    }

    fn include(
        &mut self,
        layer: Option<FactLayer>,
        complete: bool,
        selectors: &[SourceUse],
    ) -> Result<(), CatalogError> {
        extend(&mut self.selectors, selectors)?;
        if let Some(layer) = layer {
            self.layer = Some(self.layer.map_or(layer, |other| other.min(layer)));
        }
        self.complete &= complete;
        Ok(())
    }

    fn bind(&self, sources: &SourceUses) -> SourcedValue<FactLayer> {
        let value = self
            .layer
            .map_or_else(|| no_value(self.complete), SourceValue::Present);
        sources.value(value, &self.selectors, self.complete)
    }
}

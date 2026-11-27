// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use serde::Serialize;

use super::super::ReducedError;
use super::super::cell::Cell;
use super::super::completeness;
use super::super::number::Float;
use super::PATH;
use crate::catalog_queries::{
    ContractCooldown, ContractFacts, CooldownLayers, FactLayer, LayeredFacts, Stated,
};
use crate::resource_integer::ResourceInt64;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Terms<'read> {
    #[serde(flatten)]
    cooldown: Cooldown<'read>,
    time_to_complete: Cell<Float<f32>>,
    buy_in_amount: Cell<ResourceInt64>,
    #[serde(flatten)]
    layered: Layered<'read>,
}

impl<'read> Terms<'read> {
    pub(super) fn read(
        cooldown: &'read ContractCooldown,
        facts: &'read ContractFacts,
    ) -> Result<Self, ReducedError> {
        Ok(Self {
            cooldown: Cooldown::read(cooldown)?,
            time_to_complete: duration(&facts.time_to_complete)?,
            buy_in_amount: completeness::cell(PATH, &facts.buy_in_amount)?
                .map(|value| ResourceInt64::new(*value)),
            layered: Layered::read(&facts.layered)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Cooldown<'read> {
    #[serde(rename = "cooldownLayer")]
    layer: Cell<&'read FactLayer>,
    #[serde(rename = "cooldownLayers")]
    layers: Layers<'read>,
    #[serde(rename = "hasPersonalCooldown")]
    has_personal_timer: Cell<&'read bool>,
    personal_cooldown_time: Cell<Float<f32>>,
    personal_cooldown_time_variation: Cell<Float<f32>>,
    abandoned_cooldown_time: Cell<Float<f32>>,
    abandoned_cooldown_time_variation: Cell<Float<f32>>,
}

impl<'read> Cooldown<'read> {
    fn read(value: &'read ContractCooldown) -> Result<Self, ReducedError> {
        Ok(Self {
            layer: completeness::cell(PATH, &value.cooldown_layer)?,
            layers: Layers::read(&value.cooldown_layers)?,
            has_personal_timer: completeness::cell(PATH, &value.has_personal_cooldown)?,
            personal_cooldown_time: duration(&value.personal_cooldown_time)?,
            personal_cooldown_time_variation: duration(&value.personal_cooldown_time_variation)?,
            abandoned_cooldown_time: duration(&value.abandoned_cooldown_time)?,
            abandoned_cooldown_time_variation: duration(&value.abandoned_cooldown_time_variation)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Layers<'read> {
    has_personal_cooldown: Cell<&'read FactLayer>,
    personal_cooldown_time: Cell<&'read FactLayer>,
    personal_cooldown_time_variation: Cell<&'read FactLayer>,
    abandoned_cooldown_time: Cell<&'read FactLayer>,
    abandoned_cooldown_time_variation: Cell<&'read FactLayer>,
}

impl<'read> Layers<'read> {
    fn read(value: &'read CooldownLayers) -> Result<Self, ReducedError> {
        Ok(Self {
            has_personal_cooldown: completeness::cell(PATH, &value.has_personal_cooldown)?,
            personal_cooldown_time: completeness::cell(PATH, &value.personal_cooldown_time)?,
            personal_cooldown_time_variation: completeness::cell(
                PATH,
                &value.personal_cooldown_time_variation,
            )?,
            abandoned_cooldown_time: completeness::cell(PATH, &value.abandoned_cooldown_time)?,
            abandoned_cooldown_time_variation: completeness::cell(
                PATH,
                &value.abandoned_cooldown_time_variation,
            )?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Layered<'read> {
    can_be_shared: Choice<'read, bool>,
    illegal: Choice<'read, bool>,
    once_only: Choice<'read, bool>,
    max_players_per_instance: Choice<'read, i64>,
    available_in_prison: Choice<'read, bool>,
    can_reaccept_after_abandoning: Choice<'read, bool>,
    can_reaccept_after_failing: Choice<'read, bool>,
    hide_in_mobi_glas: Choice<'read, bool>,
}

impl<'read> Layered<'read> {
    fn read(value: &'read LayeredFacts) -> Result<Self, ReducedError> {
        Ok(Self {
            can_be_shared: Choice::read(&value.can_be_shared)?,
            illegal: Choice::read(&value.illegal)?,
            once_only: Choice::read(&value.once_only)?,
            max_players_per_instance: Choice::read(&value.max_players_per_instance)?,
            available_in_prison: Choice::read(&value.available_in_prison)?,
            can_reaccept_after_abandoning: Choice::read(&value.can_reaccept_after_abandoning)?,
            can_reaccept_after_failing: Choice::read(&value.can_reaccept_after_failing)?,
            hide_in_mobi_glas: Choice::read(&value.hide_in_mobi_glas)?,
        })
    }
}

#[derive(Serialize)]
struct Choice<'read, T> {
    value: Cell<&'read T>,
    layer: Cell<&'read FactLayer>,
}

impl<'read, T> Choice<'read, T> {
    fn read(value: &'read Stated<T>) -> Result<Self, ReducedError> {
        Ok(Self {
            value: completeness::cell(PATH, &value.value)?,
            layer: completeness::cell(PATH, &value.layer)?,
        })
    }
}

fn duration(value: &SourcedValue<f32>) -> Result<Cell<Float<f32>>, ReducedError> {
    Ok(completeness::cell(PATH, value)?.map(|value| Float::from(*value)))
}

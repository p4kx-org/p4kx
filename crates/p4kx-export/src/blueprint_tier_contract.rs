// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::blueprint_definition::PhysicalBlueprintRef;
use crate::blueprint_research_contract::BlueprintResearch;
use crate::catalog_text_contract::GameTextRef;
use crate::guid_text::CatalogGuid;
use crate::resource_integer::ResourceInt64;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;

/// Contextual interpretation of one actual Blueprint tier and its authored recipe.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct BlueprintTier {
    pub(crate) blueprint: PhysicalBlueprintRef,
    pub(crate) tier_index: SourcedValue<u32>,
    pub(crate) source: SourceValueRef,
    pub(crate) craft_time_seconds: SourcedValue<ResourceInt64>,
    pub(crate) mandatory: Option<SourceValueRef>,
    pub(crate) optional: Vec<SourceValueRef>,
    pub(crate) costs: Vec<BlueprintCost>,
    pub(crate) research: BlueprintResearch,
}

/// One interpreted cost whose authored source and context remain in the Blueprint arena.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct BlueprintCost {
    pub(crate) source: SourceValueRef,
    pub(crate) position: SourcedValue<BlueprintCostPosition>,
    pub(crate) context: crate::blueprint_context_contract::BlueprintCostContext,
    pub(crate) meaning: BlueprintCostMeaning,
}

/// The actual cost edge in the physical recipe tier.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum BlueprintCostPosition {
    /// The single mandatory cost edge.
    Mandatory,
    /// An original optional cost array entry.
    Optional {
        /// The original zero-based array position.
        #[schemars(range(max = u32::MAX))]
        index: u32,
    },
    /// An original alternative within a parent selection.
    Option {
        /// The parent selection occurrence in the same Blueprint arena.
        parent: SourceValueRef,
        /// The original zero-based array position.
        #[schemars(range(max = u32::MAX))]
        index: u32,
    },
}

/// The selection or ingredient interpretation of a recognized authored cost.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum BlueprintCostMeaning {
    /// An authored selection with its available alternatives.
    Select {
        /// The interpreted number of alternatives to select.
        count: SourcedValue<ResourceInt64>,
        /// The original debug name occurrence when available.
        debug_name: Option<SourceValueRef>,
        #[schemars(extend("x-gameText" = true))]
        /// The protected localized selection name identity.
        display_name: SourcedValue<GameTextRef>,
        /// Original option occurrences in authored order.
        options: Vec<SourceValueRef>,
    },
    /// An authored resource ingredient.
    Resource {
        /// The interpreted original resource GUID.
        resource_guid: SourcedValue<CatalogGuid>,
        /// The interpreted resource quantity in SCU.
        quantity_scu: SourcedValue<f32>,
        /// The interpreted minimum ingredient quality.
        min_quality: SourcedValue<ResourceInt64>,
    },
    /// An authored item ingredient.
    Item {
        /// The interpreted original item GUID.
        item_guid: SourcedValue<CatalogGuid>,
        /// The interpreted whole item quantity.
        quantity_units: SourcedValue<ResourceInt64>,
        /// The interpreted minimum ingredient quality.
        min_quality: SourcedValue<ResourceInt64>,
    },
}

impl BlueprintTier {
    /// Borrows the physical owner of this tier.
    #[must_use]
    pub const fn blueprint(&self) -> &PhysicalBlueprintRef {
        &self.blueprint
    }

    /// Borrows the original zero-based tier array position.
    #[must_use]
    pub const fn tier_index(&self) -> &SourcedValue<u32> {
        &self.tier_index
    }

    /// Returns this tier occurrence within its Blueprint arena.
    #[must_use]
    pub const fn source(&self) -> SourceValueRef {
        self.source
    }

    /// Borrows the interpreted crafting duration in whole seconds.
    #[must_use]
    pub const fn craft_time_seconds(&self) -> &SourcedValue<ResourceInt64> {
        &self.craft_time_seconds
    }

    /// Returns the declared mandatory cost occurrence when available.
    #[must_use]
    pub const fn mandatory(&self) -> Option<SourceValueRef> {
        self.mandatory
    }

    /// Borrows optional cost occurrences in their original order.
    #[must_use]
    pub fn optional(&self) -> &[SourceValueRef] {
        &self.optional
    }

    /// Borrows interpreted costs in their authored depth-first order.
    #[must_use]
    pub fn costs(&self) -> &[BlueprintCost] {
        &self.costs
    }

    /// Borrows the original research envelope interpretation.
    #[must_use]
    pub const fn research(&self) -> &BlueprintResearch {
        &self.research
    }
}

impl BlueprintCost {
    /// Returns this cost occurrence within its Blueprint arena.
    #[must_use]
    pub const fn source(&self) -> SourceValueRef {
        self.source
    }

    /// Borrows the exact authored cost edge and array position.
    #[must_use]
    pub const fn position(&self) -> &SourcedValue<BlueprintCostPosition> {
        &self.position
    }

    /// Borrows the original context and effective cost rules.
    #[must_use]
    pub const fn context(&self) -> &crate::blueprint_context_contract::BlueprintCostContext {
        &self.context
    }

    /// Borrows the recognized selection or ingredient interpretation.
    #[must_use]
    pub const fn meaning(&self) -> &BlueprintCostMeaning {
        &self.meaning
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::{BlueprintCostPosition, BlueprintTier};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    #[test]
    fn recipe_quality_position_schema_requires_exact_physical_steps() -> TestResult {
        let schema = schemars::schema_for!(BlueprintCostPosition).to_value();
        let validator = jsonschema::validator_for(&schema)?;
        for valid in [
            json!({"type":"mandatory"}),
            json!({"type":"optional","index":u32::MAX}),
            json!({"type":"option","parent":{"valueIndex":"44"},"index":0}),
        ] {
            assert!(
                validator.is_valid(&valid),
                "valid exact physical cost step {valid}"
            );
        }
        for invalid in [
            json!({"type":"optional"}),
            json!({"type":"optional","index":"4"}),
            json!({"type":"optional","index":4_294_967_296_u64}),
            json!({"type":"optional","index":-1}),
            json!({"type":"option","index":0}),
            json!({"type":"option","parent":"44","index":0}),
            json!({"type":"other","index":0}),
        ] {
            assert!(
                !validator.is_valid(&invalid),
                "invalid physical cost step {invalid}"
            );
        }
        let tier = schemars::schema_for!(BlueprintTier).to_value();
        assert!(
            tier.get("required")
                .and_then(Value::as_array)
                .is_some_and(|keys| keys.contains(&json!("tierIndex"))),
            "every interpreted tier reports its physical array position"
        );
        Ok(())
    }
}

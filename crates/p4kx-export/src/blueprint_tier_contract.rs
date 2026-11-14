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
pub(crate) struct BlueprintTier {
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
pub(crate) struct BlueprintCost {
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
pub(crate) enum BlueprintCostPosition {
    Mandatory,
    Optional {
        #[schemars(range(max = u32::MAX))]
        index: u32,
    },
    Option {
        parent: SourceValueRef,
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
pub(crate) enum BlueprintCostMeaning {
    Select {
        count: SourcedValue<ResourceInt64>,
        debug_name: Option<SourceValueRef>,
        #[schemars(extend("x-gameText" = true))]
        display_name: SourcedValue<GameTextRef>,
        options: Vec<SourceValueRef>,
    },
    Resource {
        resource_guid: SourcedValue<CatalogGuid>,
        quantity_scu: SourcedValue<f32>,
        min_quality: SourcedValue<ResourceInt64>,
    },
    Item {
        item_guid: SourcedValue<CatalogGuid>,
        quantity_units: SourcedValue<ResourceInt64>,
        min_quality: SourcedValue<ResourceInt64>,
    },
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

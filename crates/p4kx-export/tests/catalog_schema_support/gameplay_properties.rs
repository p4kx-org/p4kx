// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceValue;
use serde_json::{Value, json};

use super::source_values as values;

const NAME: &str = "name:19:@StatName_GPP_Armor";
const UNIT: &str = "name:15:@Unit_GPP_Armor";

/// Constructs schema data without claiming an original Record or lookup was observed.
pub(super) fn definitions() -> Value {
    json!({"availability":"available", "rows":[{
        "guid":values::present(json!("e1000000-0000-4000-8000-000000000001")),
        "recordName":values::present(json!("CraftingGameplayPropertyDef.GPP_Armor")),
        "className":values::present(json!("GPP_Armor")),
        "recordPath":values::present(json!("libs/foundry/records/crafting/craftedproperties/gpp_armor")),
        "recordIndex":values::present(json!(0)),
        "sourceType":values::present(json!("CraftingGameplayPropertyDef")),
        "fileName":values::present(json!("libs/foundry/records/crafting/craftedproperties/gpp_armor.xml")),
        "metadataTag":values::source(SourceValue::Absent),
        "propertyName":values::present(json!(NAME)),
        "displayTransformation":values::present(transformation()),
        "unitFormat":values::present(json!(UNIT)),
        "nameOverrides":values::present(json!([values::present(name_override())]))
    }]})
}

fn transformation() -> Value {
    json!({"root":{"nodeIndex":0}, "nodes":[
        {"sourceType":values::present(json!("CraftingDisplayTransformation_Sequence")),
         "payload":values::present(json!({"type":"sequence", "transformations":values::present(json!([
             values::present(json!({"nodeIndex":1})), values::source(SourceValue::Null), values::present(json!({"nodeIndex":2}))
         ]))}))},
        {"sourceType":values::present(json!("CraftingDisplayTransformation_Scale")),
         "payload":values::present(json!({"type":"scale", "scale":values::present(json!({"type":"f32", "rawBits":"3f800000", "finite":1.0}))}))},
        {"sourceType":values::present(json!("CraftingDisplayTransformation_Base_NonRef")),
         "payload":values::present(json!({"type":"emptyBase"}))}
    ]})
}

fn name_override() -> Value {
    json!({"sourceType":values::present(json!("CraftingPropertyNameOverride")), "payload":values::present(json!({
        "propertyName":values::present(json!(NAME)),
        "condition":values::present(json!({
            "sourceType":values::present(json!("CraftingPropertyNameOverrideCondition_ItemType")),
            "payload":values::present(json!({"type":"itemType",
                "matchItemTypes":values::present(json!([values::present(json!("SyntheticItem")), values::present(json!(""))])),
                "matchSubTypes":values::present(json!([]))
            }))
        }))
    }))})
}

pub(super) fn public_names() -> Vec<Value> {
    [(NAME, "@StatName_GPP_Armor", "Armour"), (UNIT, "@Unit_GPP_Armor", "%+.2f %%")]
        .into_iter().map(|(id, key, text)| json!({
            "id":values::present(json!(id)), "key":values::present(json!(key)),
            "translations":values::present(json!({
                "en":{"sourceText":values::present(json!(text)), "displayText":values::present(json!(text))}
            }))
        })).collect()
}

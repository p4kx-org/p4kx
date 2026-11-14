// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestStruct, TestType};

pub(crate) const PROPERTY: &str = "CraftingGameplayPropertyDef";
pub(crate) const OVERRIDE: &str = "CraftingPropertyNameOverride";
pub(crate) const BASE: &str = "CraftingDisplayTransformation_Base";
pub(crate) const NON_REF: &str = "CraftingDisplayTransformation_Base_NonRef";
pub(crate) const NEGATED: &str =
    "CraftingDisplayTransformation_ConvertFactorToNegatedPercentChange";
pub(crate) const PERCENT: &str = "CraftingDisplayTransformation_ConvertFactorToPercentChange";
pub(crate) const FACTOR: &str = "CraftingDisplayTransformation_ConvertValueToFactorOfBaseValue";
pub(crate) const SCALE: &str = "CraftingDisplayTransformation_Scale";
pub(crate) const SEQUENCE: &str = "CraftingDisplayTransformation_Sequence";
pub(crate) const CONDITION: &str = "CraftingPropertyNameOverrideCondition_Base";
pub(crate) const CONDITION_NON_REF: &str = "CraftingPropertyNameOverrideCondition_Base_NonRef";
pub(crate) const ITEM_TYPE: &str = "CraftingPropertyNameOverrideCondition_ItemType";

pub(crate) const TRANSFORMS: [(&str, &str); 7] = [
    (BASE, "emptyBase"),
    (NON_REF, "emptyBase"),
    (NEGATED, "convertFactorToNegatedPercentChange"),
    (PERCENT, "convertFactorToPercentChange"),
    (FACTOR, "convertValueToFactorOfBaseValue"),
    (SCALE, "scale"),
    (SEQUENCE, "sequence"),
];

pub(crate) fn property() -> TestStruct {
    TestStruct::new(PROPERTY)
        .field("propertyName", TestType::Locale)
        .field(
            "displayTransformation",
            TestType::StrongPointer(BASE.to_owned()),
        )
        .field("unitFormat", TestType::Locale)
        .array("nameOverrides", TestType::Class(OVERRIDE.to_owned()))
}

pub(crate) fn schema() -> DataForgeBuilder {
    custom(property(), TestType::Single)
}

pub(crate) fn custom(property: TestStruct, scale: TestType) -> DataForgeBuilder {
    let mut builder = DataForgeBuilder::new()
        .version(8)
        .enumeration("EItemType", ["SyntheticA", "SyntheticB", ""])
        .enumeration("EItemSubType", ["SyntheticSub", ""])
        .structure(property)
        .structure(
            TestStruct::new(OVERRIDE)
                .field("propertyName", TestType::Locale)
                .field("condition", TestType::StrongPointer(CONDITION.to_owned())),
        );
    builder = display_types(builder, scale);
    condition_types(builder)
}

fn display_types(mut builder: DataForgeBuilder, scale: TestType) -> DataForgeBuilder {
    builder = builder
        .structure(TestStruct::new(BASE))
        .structure(TestStruct::new(NON_REF).parent(BASE));
    for name in [NEGATED, PERCENT, FACTOR] {
        builder = builder.structure(TestStruct::new(name).parent(NON_REF));
    }
    builder
        .structure(TestStruct::new(SCALE).parent(NON_REF).field("scale", scale))
        .structure(
            TestStruct::new(SEQUENCE)
                .parent(NON_REF)
                .array("transformations", TestType::StrongPointer(BASE.to_owned())),
        )
}

fn condition_types(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(TestStruct::new(CONDITION))
        .structure(TestStruct::new(CONDITION_NON_REF).parent(CONDITION))
        .structure(
            TestStruct::new(ITEM_TYPE)
                .parent(CONDITION_NON_REF)
                .array("matchItemTypes", TestType::Enum("EItemType".to_owned()))
                .array("matchSubTypes", TestType::Enum("EItemSubType".to_owned())),
        )
}

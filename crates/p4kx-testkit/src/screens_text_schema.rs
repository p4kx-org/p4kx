// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{TestStruct, TestType};

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

pub(crate) fn text_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("BuildingBlocks_ComponentLabelProperties")
            .field("show", TestType::Bool)
            .field("label", TestType::String)
            .field("style", TestType::String)
            .field("caseModifier", TestType::String)
            .field("anchorToParentX", TestType::Single)
            .field("anchorToParentY", TestType::Single)
            .field("scale", TestType::Single),
        TestStruct::new("BuildingBlocks_FontStyle")
            .field("font", TestType::String)
            .field("paintFile", TestType::String)
            .field("isBold", TestType::Bool)
            .field("scaleModifier", TestType::Single),
        TestStruct::new("BuildingBlocks_FieldModifierLocalization")
            .parent("BuildingBlocks_FieldModifier")
            .field("field", TestType::String)
            .field("value", TestType::Locale),
        TestStruct::new("BuildingBlocks_FieldModifierRecordRefType"),
        TestStruct::new("BuildingBlocks_FieldModifierRecordRefTypeFontStyleRecord")
            .parent("BuildingBlocks_FieldModifierRecordRefType")
            .field(
                "value",
                TestType::ReferenceTo("BuildingBlocks_FontStyle".to_owned()),
            ),
        TestStruct::new("BuildingBlocks_FieldModifierRecordRefTypeCanvasReferenceRecord")
            .parent("BuildingBlocks_FieldModifierRecordRefType")
            .field(
                "value",
                TestType::ReferenceTo("BuildingBlocks_Canvas".to_owned()),
            ),
        TestStruct::new("BuildingBlocks_FieldModifierRecordRef")
            .parent("BuildingBlocks_FieldModifier")
            .field(
                "field",
                pointer("BuildingBlocks_FieldModifierRecordRefType"),
            ),
        TestStruct::new("BuildingBlocks_FieldModifierEnumeratedType"),
        TestStruct::new("BuildingBlocks_FieldModifierEnumeratedTypeHorizontalAlignment")
            .parent("BuildingBlocks_FieldModifierEnumeratedType")
            .field("value", TestType::String),
        TestStruct::new("BuildingBlocks_FieldModifierEnumeratedTypeVerticalAlignment")
            .parent("BuildingBlocks_FieldModifierEnumeratedType")
            .field("value", TestType::String),
        TestStruct::new("BuildingBlocks_FieldModifierEnumeratedTypeCaseModifier")
            .parent("BuildingBlocks_FieldModifierEnumeratedType")
            .field("value", TestType::String),
        TestStruct::new("BuildingBlocks_FieldModifierEnumerated")
            .parent("BuildingBlocks_FieldModifier")
            .field(
                "field",
                pointer("BuildingBlocks_FieldModifierEnumeratedType"),
            ),
    ]
}

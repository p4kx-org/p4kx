// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{TestStruct, TestType};

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn vector_fields(structure: TestStruct) -> TestStruct {
    structure
        .field("renderShape", TestType::Bool)
        .field("flipHorizontal", TestType::Bool)
        .field("flipVertical", TestType::Bool)
        .field("enableColorOverlay", TestType::Bool)
        .field("scalingBehavior", TestType::String)
        .field("containPositionX", TestType::Single)
        .field("containPositionY", TestType::Single)
        .field("enableNineSliceRect", TestType::Bool)
        .field("nineSliceScale", TestType::Single)
        .field("nineSliceRect", class("BuildingBlocks_TRBL"))
        .field("color", pointer("BuildingBlocks_Color"))
}

pub(crate) fn image_structs() -> Vec<TestStruct> {
    let display = "BuildingBlocks_DisplayWidget";
    vec![
        vector_fields(TestStruct::new("BuildingBlocks_SvgFill").field("svgPath", TestType::String)),
        TestStruct::new("BuildingBlocks_ComponentIconProperties")
            .field("show", TestType::Bool)
            .field("iconPreset", TestType::String)
            .field("customIcon", TestType::String)
            .field("anchorToParentX", TestType::Single)
            .field("anchorToParentY", TestType::Single)
            .field("scale", TestType::Single),
        TestStruct::new("BuildingBlocks_FieldModifierString")
            .parent("BuildingBlocks_FieldModifier")
            .field("field", TestType::String)
            .field("value", TestType::String),
        TestStruct::new("BuildingBlocks_WidgetImage")
            .parent(display)
            .field("imagePath", TestType::String)
            .field("imageScalingMethod", TestType::String)
            .field("flipH", TestType::Bool)
            .field("flipV", TestType::Bool)
            .field("containPositionX", TestType::Single)
            .field("containPositionY", TestType::Single)
            .field("color", pointer("BuildingBlocks_Color")),
        vector_fields(
            TestStruct::new("BuildingBlocks_WidgetCustomShape")
                .parent(display)
                .field("svgPath", TestType::String),
        ),
        TestStruct::new("BuildingBlocks_WidgetIcon")
            .parent(display)
            .field(
                "iconProperties",
                class("BuildingBlocks_ComponentIconProperties"),
            ),
        TestStruct::new("BuildingBlocks_WidgetForegroundOverlay")
            .parent(display)
            .field("urlOptional", TestType::String),
        TestStruct::new("BuildingBlocks_WidgetBodyBackground")
            .parent(display)
            .field("urlOptional", TestType::String),
    ]
}

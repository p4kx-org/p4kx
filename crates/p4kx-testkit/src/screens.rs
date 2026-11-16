// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::screens_animation::animation_structs;
use crate::screens_geometry::{box_structs, instance_structs};
use crate::screens_image_schema::image_structs;
use crate::screens_radar_schema::radar_structs;
use crate::screens_rest::rest_structs;
use crate::screens_text_schema::text_structs;
use crate::{DataForgeBuilder, TestStruct, TestType};

/// The colour slots of the synthetic `BuildingBlocks` styles, in the order their palettes list
/// colours.
pub const SCREEN_COLOUR_SLOTS: [&str; 3] = ["Base", "Accent1", "Bright"];

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn reference(name: &str) -> TestType {
    TestType::ReferenceTo(name.to_owned())
}

/// Adds the structs of `BuildingBlocks` canvases, widgets, styles and the entity component that
/// names a canvas to `builder`, next to [`entity_schema`](crate::entity_schema).
///
/// They are named and shaped as in the game data for the fields the screens reader uses; enums
/// the data stores as enum options are written as strings, which the reader accepts alike.
#[must_use]
pub fn screens_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    let builder = builder.enumeration("BuildingBlocks_ColorStyleSlot", SCREEN_COLOUR_SLOTS);
    layout_structs()
        .into_iter()
        .chain(widget_structs())
        .chain(widget_kinds())
        .chain(style_structs())
        .chain(condition_structs())
        .chain(component_structs())
        .chain(instance_structs())
        .chain(box_structs())
        .chain(text_structs())
        .chain(image_structs())
        .chain(rest_structs())
        .chain(animation_structs())
        .chain(radar_structs())
        .fold(builder, DataForgeBuilder::structure)
}

fn layout_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("Vec3")
            .field("x", TestType::Single)
            .field("y", TestType::Single)
            .field("z", TestType::Single),
        TestStruct::new("BuildingBlocks_FixedOrRelativeValue")
            .field("value", TestType::Single)
            .field("behavior", TestType::String),
        TestStruct::new("BuildingBlocks_Size")
            .field("enableMinWidth", TestType::Bool)
            .field("enableMinHeight", TestType::Bool)
            .field("enableMaxWidth", TestType::Bool)
            .field("enableMaxHeight", TestType::Bool)
            .field("width", class("BuildingBlocks_FixedOrRelativeValue"))
            .field("height", class("BuildingBlocks_FixedOrRelativeValue"))
            .field("minWidth", class("BuildingBlocks_FixedOrRelativeValue"))
            .field("minHeight", class("BuildingBlocks_FixedOrRelativeValue"))
            .field("maxWidth", class("BuildingBlocks_FixedOrRelativeValue"))
            .field("maxHeight", class("BuildingBlocks_FixedOrRelativeValue")),
        TestStruct::new("BuildingBlocks_Overflow").field("overflow", TestType::String),
        TestStruct::new("BuildingBlocks_TRBL")
            .field("top", TestType::Single)
            .field("right", TestType::Single)
            .field("bottom", TestType::Single)
            .field("left", TestType::Single),
        TestStruct::new("BuildingBlocks_LayoutPolicy"),
        TestStruct::new("BuildingBlocks_FlexContainer")
            .parent("BuildingBlocks_LayoutPolicy")
            .field("direction", TestType::String)
            .field("wrap", TestType::String)
            .field("axisJustification", TestType::String)
            .field("crossAxisJustification", TestType::String)
            .field("itemAlignment", TestType::String)
            .field("columnSpacing", TestType::Single)
            .field("rowSpacing", TestType::Single),
        TestStruct::new("BuildingBlocks_StackContainer")
            .parent("BuildingBlocks_LayoutPolicy")
            .field("direction", TestType::String)
            .field("spacing", TestType::Single),
        TestStruct::new("BuildingBlocks_GridPackContainer")
            .parent("BuildingBlocks_LayoutPolicy")
            .field("direction", TestType::String)
            .field("gutter", TestType::Single),
        TestStruct::new("BuildingBlocks_SpacerContainer")
            .parent("BuildingBlocks_LayoutPolicy")
            .field("spacing", TestType::Single),
        TestStruct::new("BuildingBlocks_FlexItem")
            .field("growProportion", TestType::Single)
            .field("shrinkProportion", TestType::Single),
        TestStruct::new("BuildingBlocks_LayoutItemCommon").field("order", TestType::Int32),
    ]
}

fn widget_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("BuildingBlocks_Canvas")
            .field("coordinateMethod", TestType::String)
            .field("size", class("Vec3"))
            .array("embeddedStyles", pointer("BuildingBlocks_StyleEntry"))
            .field("defaultStyles", class("BuildingBlocks_DefaultStyles"))
            .field("style", reference("BuildingBlocks_Style"))
            .array("brandStyles", class("BuildingBlocks_BrandStyles"))
            .array("staticVariables", pointer("BuildingBlocks_StaticVariable"))
            .array("scene", pointer("BuildingBlocks_DisplayWidget"))
            .array("library", pointer("BuildingBlocks_DisplayWidget"))
            .field(
                "proxyRootNode",
                TestType::WeakPointer("BuildingBlocks_DisplayWidget".to_owned()),
            )
            .array("operations", pointer("BuildingBlocks_BindingsOperation")),
        TestStruct::new("BuildingBlocks_DefaultStyles")
            .field("sharedStyles", reference("BuildingBlocks_Style"))
            .array("entries", pointer("BuildingBlocks_StyleEntry")),
        TestStruct::new("BuildingBlocks_BrandStyles")
            .field("brandIdentifier", reference("BuildingBlocks_Style"))
            .array("entries", pointer("BuildingBlocks_StyleEntry")),
        TestStruct::new("BuildingBlocks_DisplayWidget")
            .field("name", TestType::String)
            .field("isActive", TestType::Bool)
            .field("affectsLayout", TestType::Bool)
            .field("layer", TestType::Int32)
            .field("inheritsAlpha", TestType::Bool)
            .field("alpha", TestType::Single)
            .array("styleTags", reference("Tag"))
            .field(
                "parent",
                TestType::WeakPointer("BuildingBlocks_DisplayWidget".to_owned()),
            )
            .field("position", class("Vec3"))
            .field("positionOffset", class("Vec3"))
            .field("orientation", class("Vec3"))
            .field("orientationOffset", class("Vec3"))
            .field("scale", class("Vec3"))
            .field("pivot", class("Vec3"))
            .field("anchor", class("Vec3"))
            .field("overflow", class("BuildingBlocks_Overflow"))
            .field("sizing", class("BuildingBlocks_Size"))
            .field("padding", class("BuildingBlocks_TRBL"))
            .field("margin", class("BuildingBlocks_TRBL"))
            .field("background", class("BuildingBlocks_Background"))
            .field("border", class("BuildingBlocks_Border"))
            .field("layoutPolicy", pointer("BuildingBlocks_LayoutPolicy"))
            .field("layoutPolicyItem", pointer("BuildingBlocks_FlexItem"))
            .field(
                "layoutItemCommon",
                pointer("BuildingBlocks_LayoutItemCommon"),
            )
            .array("inlineStyles", pointer("BuildingBlocks_StyleEntry")),
    ]
}

fn widget_kinds() -> Vec<TestStruct> {
    vec![
        button_structure(),
        TestStruct::new("BuildingBlocks_WidgetCard").parent("BuildingBlocks_DisplayWidget"),
        TestStruct::new("BuildingBlocks_WidgetCanvas")
            .parent("BuildingBlocks_DisplayWidget")
            .field("sizingMethod", TestType::String)
            .field("canvas", reference("BuildingBlocks_Canvas"))
            .field("stylesheetOverride", reference("BuildingBlocks_Style"))
            .array(
                "paramInputValues",
                pointer("BuildingBlocks_ComponentParameterInput"),
            ),
        TestStruct::new("BuildingBlocks_WidgetText")
            .parent("BuildingBlocks_DisplayWidget")
            .field("locString", TestType::Locale)
            .field("fontStyle", reference("BuildingBlocks_FontStyle"))
            .field("fontSize", TestType::Single)
            .field("autoFontSize", TestType::Bool)
            .field("color", pointer("BuildingBlocks_Color"))
            .field("letterSpacing", TestType::Single)
            .field("lineSpacing", TestType::Single)
            .field("textAlignment", TestType::String)
            .field("verticalAlignment", TestType::String)
            .field("wordWrap", TestType::Bool)
            .field("caseModifier", TestType::String)
            .field("kerning", TestType::Bool),
        TestStruct::new("BuildingBlocks_WidgetTextField")
            .parent("BuildingBlocks_DisplayWidget")
            .field(
                "labelProperties",
                class("BuildingBlocks_ComponentLabelProperties"),
            )
            .field("textAlignment", TestType::String)
            .field("verticalTextAlignment", TestType::String),
        TestStruct::new("BuildingBlocks_WidgetCircle")
            .parent("BuildingBlocks_DisplayWidget")
            .field("doFill", TestType::Bool)
            .field("doStroke", TestType::Bool)
            .field("strokeWidth", TestType::Single)
            .field("fillColor", pointer("BuildingBlocks_Color"))
            .field("strokeColor", pointer("BuildingBlocks_Color")),
        TestStruct::new("BuildingBlocks_WidgetPolygon")
            .parent("BuildingBlocks_DisplayWidget")
            .field("doFill", TestType::Bool)
            .field("doStroke", TestType::Bool)
            .field("strokeWidth", TestType::Single)
            .field("sides", TestType::Int32)
            .field("startAngle", TestType::Single)
            .field("fillColor", pointer("BuildingBlocks_Color"))
            .field("strokeColor", pointer("BuildingBlocks_Color")),
    ]
}

fn button_structure() -> TestStruct {
    TestStruct::new("BuildingBlocks_ComponentGeneralButtonSecondary")
        .parent("BuildingBlocks_DisplayWidget")
        .field("sizingMethod", TestType::String)
        .field("fillStyle", TestType::String)
        .field("iconPosition", TestType::String)
        .field(
            "labelProperties",
            class("BuildingBlocks_ComponentLabelProperties"),
        )
        .field(
            "iconProperties",
            class("BuildingBlocks_ComponentIconProperties"),
        )
}

fn style_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("Tag").field("tagName", TestType::String),
        TestStruct::new("SRGBA8")
            .field("r", TestType::UInt8)
            .field("g", TestType::UInt8)
            .field("b", TestType::UInt8)
            .field("a", TestType::UInt8),
        TestStruct::new("BuildingBlocks_Color"),
        TestStruct::new("BuildingBlocks_ColorSolid")
            .parent("BuildingBlocks_Color")
            .field("color", class("SRGBA8")),
        TestStruct::new("BuildingBlocks_ColorStyle")
            .parent("BuildingBlocks_Color")
            .field(
                "color",
                TestType::Enum("BuildingBlocks_ColorStyleSlot".to_owned()),
            )
            .field("alpha", TestType::Single),
        TestStruct::new("BuildingBlocks_Style")
            .array("entries", pointer("BuildingBlocks_StyleEntry"))
            .array("colorStyles", pointer("BuildingBlocks_Color")),
        TestStruct::new("BuildingBlocks_StyleEntry")
            .field("name", TestType::String)
            .array("conditionsList", class("BuildingBlocks_StyleConditionList"))
            .array("modifiers", pointer("BuildingBlocks_FieldModifier")),
        TestStruct::new("BuildingBlocks_StyleConditionList")
            .field("name", TestType::String)
            .array(
                "conditions",
                pointer("BuildingBlocks_StyleSelectorCondition"),
            ),
    ]
}

fn condition_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("BuildingBlocks_StyleSelectorCondition"),
        TestStruct::new("BuildingBlocks_StyleSelectorConditionType")
            .parent("BuildingBlocks_StyleSelectorCondition")
            .field("type", TestType::String),
        TestStruct::new("BuildingBlocks_StyleSelectorConditionTag")
            .parent("BuildingBlocks_StyleSelectorCondition")
            .field("tag", reference("Tag")),
        TestStruct::new("BuildingBlocks_StyleSelectorConditionParent")
            .parent("BuildingBlocks_StyleSelectorCondition")
            .array(
                "conditions",
                pointer("BuildingBlocks_StyleSelectorCondition"),
            ),
        TestStruct::new("BuildingBlocks_StyleSelectorConditionAncestor")
            .parent("BuildingBlocks_StyleSelectorCondition")
            .array(
                "conditions",
                pointer("BuildingBlocks_StyleSelectorCondition"),
            )
            .array(
                "breakConditions",
                pointer("BuildingBlocks_StyleSelectorCondition"),
            ),
        TestStruct::new("BuildingBlocks_StyleSelectorConditionChild")
            .parent("BuildingBlocks_StyleSelectorCondition")
            .array(
                "conditions",
                pointer("BuildingBlocks_StyleSelectorCondition"),
            ),
        TestStruct::new("BuildingBlocks_StyleSelectorConditionCount")
            .parent("BuildingBlocks_StyleSelectorCondition")
            .field("type", TestType::String)
            .field("count", TestType::Int32)
            .field("offset", TestType::Int32),
        TestStruct::new("BuildingBlocks_StyleSelectorConditionAllOfCondition")
            .parent("BuildingBlocks_StyleSelectorCondition")
            .array(
                "conditions",
                pointer("BuildingBlocks_StyleSelectorCondition"),
            ),
        TestStruct::new("BuildingBlocks_StyleSelectorConditionUnknown")
            .parent("BuildingBlocks_StyleSelectorCondition"),
        TestStruct::new("BuildingBlocks_FieldModifier"),
        TestStruct::new("BuildingBlocks_FieldModifierColor")
            .parent("BuildingBlocks_FieldModifier")
            .field("field", TestType::String)
            .field("color", pointer("BuildingBlocks_Color")),
        TestStruct::new("BuildingBlocks_FieldModifierBoolean")
            .parent("BuildingBlocks_FieldModifier")
            .field("field", TestType::String)
            .field("value", TestType::Bool),
        TestStruct::new("BuildingBlocks_FieldModifierNumber")
            .parent("BuildingBlocks_FieldModifier")
            .field("field", TestType::String)
            .field("value", TestType::Single),
    ]
}

fn component_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("UIBuildingBlocksEntityComponentParams")
            .parent("DataForgeComponentParams")
            .array("layers", pointer("BuildingBlocksLayer")),
        TestStruct::new("BuildingBlocksLayer")
            .field("layerName", TestType::String)
            .array("views", pointer("BuildingBlocksView")),
        TestStruct::new("BuildingBlocksView")
            .field("name", TestType::String)
            .field(
                "component",
                pointer("UIGraph_BuildingBlocksContextComponent"),
            ),
        TestStruct::new("UIGraph_BuildingBlocksContextComponent")
            .field("canvas", reference("BuildingBlocks_Canvas")),
        TestStruct::new("UIOwnerEntityComponentParams")
            .parent("DataForgeComponentParams")
            .field("moviePath", TestType::String),
    ]
}

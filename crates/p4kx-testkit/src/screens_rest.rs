// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{TestInstance, TestStruct, TestType, TestValue, TestWidget};

const OPERATION: &str = "BuildingBlocks_BindingsOperation";

fn operation_input() -> TestType {
    TestType::WeakPointer(OPERATION.to_owned())
}

fn widget_target() -> TestType {
    TestType::WeakPointer("BuildingBlocks_DisplayWidget".to_owned())
}

fn operation(name: &str) -> TestStruct {
    TestStruct::new(format!("BuildingBlocks_Bindings{name}")).parent(OPERATION)
}

fn field_operation(kind: &str) -> TestStruct {
    operation(kind)
        .field("field", TestType::String)
        .field("widget", widget_target())
        .field("input", operation_input())
}

/// Adds the structs of the `operations` of a canvas and of the parameters a canvas widget hands
/// to its canvas.
pub(crate) fn rest_structs() -> Vec<TestStruct> {
    let parameter = |kind: &str, value: TestType| {
        TestStruct::new(format!("BuildingBlocks_ComponentParameterInput{kind}"))
            .parent("BuildingBlocks_ComponentParameterInput")
            .field("parameter", TestType::String)
            .field("value", value)
    };
    let structs = vec![
        TestStruct::new(OPERATION),
        operation("BooleanComponentParameter")
            .field("parameter", TestType::String)
            .field("defaultValue", TestType::Bool),
        operation("NumberComponentParameter")
            .field("parameter", TestType::String)
            .field("defaultValue", TestType::Single),
        operation("LocalizedComponentParameter")
            .field("parameter", TestType::String)
            .field("defaultValue", TestType::Locale),
        field_operation("LocalizedField"),
        operation("BooleanInvert").field("input", operation_input()),
        operation("BooleanEvaluateAnd").array("inputs", operation_input()),
        operation("BooleanEvaluateOr").array("inputs", operation_input()),
        operation("NumberFromBoolean")
            .field("isTrue", TestType::Single)
            .field("isFalse", TestType::Single)
            .field("input", operation_input()),
        operation("StringFromBooleanCanvas")
            .field(
                "isTrue",
                TestType::ReferenceTo("BuildingBlocks_Canvas".to_owned()),
            )
            .field(
                "isFalse",
                TestType::ReferenceTo("BuildingBlocks_Canvas".to_owned()),
            )
            .field("input", operation_input()),
        field_operation("BooleanField"),
        field_operation("NumberField"),
        field_operation("StringField"),
        TestStruct::new("BindingsOperations_AudioEvent").parent(OPERATION),
        TestStruct::new("BuildingBlocks_ComponentParameterInput"),
        parameter("Boolean", TestType::Bool),
        parameter("Number", TestType::Single),
        parameter("Localization", TestType::Locale),
    ];
    structs
        .into_iter()
        .chain(variable_structs())
        .chain(comparison_structs())
        .chain(later_structs())
        .collect()
}

fn variable_structs() -> impl Iterator<Item = TestStruct> {
    ["Boolean", "Number", "Integer", "String", "Localized"]
        .into_iter()
        .map(|kind| operation(&format!("{kind}Variable")).field("binding", TestType::String))
}

fn comparison_structs() -> impl Iterator<Item = TestStruct> {
    [
        operation("BooleanFromInteger")
            .field("value", TestType::Int64)
            .field("type", TestType::String)
            .field("inputL", operation_input())
            .field("inputR", operation_input()),
        operation("BooleanFromIntegerSwitch")
            .field("defaultValue", TestType::Bool)
            .array("exceptions", TestType::Int64)
            .field("input", operation_input()),
        operation("BooleanFromNumber")
            .field("number", TestType::Single)
            .field("type", TestType::String)
            .field("input", operation_input())
            .field("inputB", operation_input()),
    ]
    .into_iter()
}

fn later_structs() -> impl Iterator<Item = TestStruct> {
    tag_structs().chain(enumerated_structs())
}

fn tag_structs() -> impl Iterator<Item = TestStruct> {
    let tag = || TestType::ReferenceTo("Tag".to_owned());
    [operation("TagFromBoolean")
        .field("isTrue", tag())
        .field("isFalse", tag())
        .field("inputTrue", operation_input())
        .field("inputFalse", operation_input())
        .field("input", operation_input())]
    .into_iter()
}

fn enumerated_structs() -> impl Iterator<Item = TestStruct> {
    [
        "WidthBehavior",
        "HeightBehavior",
        "FlexDirection",
        "FlexAxisJustification",
        "FlexCrossAxisJustification",
        "FlexItemAlignment",
        "FlexWrap",
    ]
    .into_iter()
    .map(|kind| {
        TestStruct::new(format!("BuildingBlocks_FieldModifierEnumeratedType{kind}"))
            .parent("BuildingBlocks_FieldModifierEnumeratedType")
            .field("value", TestType::String)
    })
}

/// One entry of the `operations` of a synthetic canvas; inputs and targets are named by earlier
/// operations and widgets.
#[derive(Debug, Clone, PartialEq)]
pub enum TestOp {
    /// A boolean game variable bound to `variable`, `false` at rest.
    BoolVar,
    /// A boolean game variable bound to the name, `false` at rest.
    NamedBoolVar(&'static str),
    /// A number game variable bound to the name, 0 at rest.
    NamedNumVar(&'static str),
    /// A number game variable, 0 at rest.
    NumVar,
    /// An integer game variable, 0 at rest.
    IntVar,
    /// A string game variable, empty at rest.
    TextVar,
    /// A localised game variable bound to the name, empty at rest.
    NamedLocalVar(&'static str),
    /// A boolean component parameter with its default.
    BoolParam(&'static str, bool),
    /// A number component parameter with its default.
    NumParam(&'static str, f32),
    /// A text component parameter with its default.
    TextParam(&'static str, &'static str),
    /// The negation of the operation.
    Invert(&'static str),
    /// All of the operations.
    And(Vec<&'static str>),
    /// Any of the operations.
    Or(Vec<&'static str>),
    /// The integer of the operation compared to a constant: `Equal`, `Greater`, ...
    IntIs(&'static str, &'static str, i64),
    /// True for the integer of the operation unless it is one of the exceptions, else false.
    IntSwitch(&'static str, Vec<i64>),
    /// The number of the operation compared to a constant.
    NumIs(&'static str, &'static str, f32),
    /// The number `on` when the boolean operation holds, else `off`.
    NumFromBool(&'static str, f32, f32),
    /// A kind of operation that is not evaluated.
    Other,
    /// A boolean field of the widget that the boolean operation sets.
    BoolField(&'static str, &'static str, &'static str),
    /// A number field of the widget that the number operation sets.
    NumField(&'static str, &'static str, &'static str),
    /// The text of the widget, which the text operation sets.
    TextOf(&'static str, &'static str),
    /// The localised field (`ParamInput0`, ...) of the widget, which the text operation sets.
    LocalField(&'static str, &'static str, &'static str),
    /// The canvas Record `guid` when the boolean operation holds, else the canvas Record `other`.
    CanvasFromBool(&'static str, u128, u128),
    /// The `CanvasReferenceRecord` of the canvas widget, which the canvas operation sets.
    CanvasField(&'static str, &'static str),
    /// The `Tag` Record `on` when the boolean operation holds, else the `Tag` Record `off`.
    TagFromBool(&'static str, u128, u128),
    /// The state tag field (`PrimaryStateTag`, ...) of the widget, which the tag operation sets.
    TagField(&'static str, &'static str, &'static str),
}

fn input(name: &str) -> TestValue {
    TestValue::pointer(format!("op:{name}"))
}

fn names(inputs: &[&str]) -> TestValue {
    TestValue::Array(inputs.iter().map(|name| input(name)).collect())
}

fn variable(kind: &str, binding: &str) -> TestInstance {
    TestInstance::new(format!("BuildingBlocks_Bindings{kind}Variable"))
        .value("binding", TestValue::text(binding))
}

fn param(kind: &str, parameter: &str, default: TestValue) -> TestInstance {
    TestInstance::new(format!("BuildingBlocks_Bindings{kind}ComponentParameter"))
        .value("parameter", TestValue::text(parameter))
        .value("defaultValue", default)
}

fn field(kind: &str, widget: &str, name: &str, source: &str) -> TestInstance {
    TestInstance::new(format!("BuildingBlocks_Bindings{kind}Field"))
        .value("field", TestValue::text(name))
        .value("widget", TestValue::pointer(widget))
        .value("input", input(source))
}

fn int_is(source: &str, how: &str, value: i64) -> TestInstance {
    TestInstance::new("BuildingBlocks_BindingsBooleanFromInteger")
        .value("value", TestValue::Int64(value))
        .value("type", TestValue::text(how))
        .value("inputL", input(source))
}

fn int_switch(source: &str, exceptions: &[i64]) -> TestInstance {
    TestInstance::new("BuildingBlocks_BindingsBooleanFromIntegerSwitch")
        .value("defaultValue", TestValue::Bool(true))
        .value(
            "exceptions",
            TestValue::Array(exceptions.iter().copied().map(TestValue::Int64).collect()),
        )
        .value("input", input(source))
}

impl TestOp {
    fn instance(&self) -> TestInstance {
        match self {
            Self::BoolVar => variable("Boolean", "variable"),
            Self::NamedBoolVar(name) => variable("Boolean", name),
            Self::NamedNumVar(name) => variable("Number", name),
            Self::NumVar => variable("Number", "variable"),
            Self::IntVar => variable("Integer", "variable"),
            Self::TextVar => variable("String", "variable"),
            Self::NamedLocalVar(name) => variable("Localized", name),
            Self::BoolParam(name, default) => param("Boolean", name, TestValue::Bool(*default)),
            Self::NumParam(name, default) => param("Number", name, TestValue::Single(*default)),
            Self::TextParam(name, default) => param("Localized", name, TestValue::text(*default)),
            Self::Invert(source) => TestInstance::new("BuildingBlocks_BindingsBooleanInvert")
                .value("input", input(source)),
            Self::And(sources) => TestInstance::new("BuildingBlocks_BindingsBooleanEvaluateAnd")
                .value("inputs", names(sources)),
            Self::Or(sources) => TestInstance::new("BuildingBlocks_BindingsBooleanEvaluateOr")
                .value("inputs", names(sources)),
            Self::IntIs(source, how, value) => int_is(source, how, *value),
            Self::IntSwitch(source, exceptions) => int_switch(source, exceptions),
            Self::NumIs(source, how, number) => {
                TestInstance::new("BuildingBlocks_BindingsBooleanFromNumber")
                    .value("number", TestValue::Single(*number))
                    .value("type", TestValue::text(*how))
                    .value("input", input(source))
            }
            Self::NumFromBool(source, on, off) => {
                TestInstance::new("BuildingBlocks_BindingsNumberFromBoolean")
                    .value("isTrue", TestValue::Single(*on))
                    .value("isFalse", TestValue::Single(*off))
                    .value("input", input(source))
            }
            Self::Other => TestInstance::new("BindingsOperations_AudioEvent"),
            Self::BoolField(widget, name, source) => field("Boolean", widget, name, source),
            Self::NumField(widget, name, source) => field("Number", widget, name, source),
            Self::TextOf(widget, source) => field("Localized", widget, "Text", source),
            Self::LocalField(widget, name, source) => field("Localized", widget, name, source),
            Self::CanvasFromBool(source, on, off) => {
                TestInstance::new("BuildingBlocks_BindingsStringFromBooleanCanvas")
                    .value("isTrue", TestValue::Reference(*on))
                    .value("isFalse", TestValue::Reference(*off))
                    .value("input", input(source))
            }
            Self::CanvasField(widget, source) => {
                field("String", widget, "CanvasReferenceRecord", source)
            }
            Self::TagFromBool(source, on, off) => {
                TestInstance::new("BuildingBlocks_BindingsTagFromBoolean")
                    .value("isTrue", TestValue::Reference(*on))
                    .value("isFalse", TestValue::Reference(*off))
                    .value("input", input(source))
            }
            Self::TagField(widget, name, source) => field("String", widget, name, source),
        }
    }
}

pub(crate) fn operations(items: &[(String, TestOp)]) -> TestValue {
    TestValue::Array(
        items
            .iter()
            .map(|(name, op)| TestValue::Struct(op.instance().named(format!("op:{name}"))))
            .collect(),
    )
}

/// A parameter a canvas widget hands to the canvas it holds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TestParam {
    /// A boolean value for the parameter.
    Bool(&'static str, bool),
    /// A number value for the parameter.
    Number(&'static str, f32),
    /// A text value for the parameter.
    Text(&'static str, &'static str),
}

impl TestWidget {
    /// Sets the parameter values the widget hands to its canvas.
    #[must_use]
    pub fn params(self, params: &[TestParam]) -> Self {
        let values = params.iter().map(|param| {
            TestValue::Struct(match *param {
                TestParam::Bool(name, value) => {
                    TestInstance::new("BuildingBlocks_ComponentParameterInputBoolean")
                        .value("parameter", TestValue::text(name))
                        .value("value", TestValue::Bool(value))
                }
                TestParam::Text(name, value) => {
                    TestInstance::new("BuildingBlocks_ComponentParameterInputLocalization")
                        .value("parameter", TestValue::text(name))
                        .value("value", TestValue::text(value))
                }
                TestParam::Number(name, value) => {
                    TestInstance::new("BuildingBlocks_ComponentParameterInputNumber")
                        .value("parameter", TestValue::text(name))
                        .value("value", TestValue::Single(value))
                }
            })
        });
        self.with("paramInputValues", TestValue::Array(values.collect()))
    }
}

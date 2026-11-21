// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::screens_animation::TestAlphaAnimation;
use crate::screens_canvas::record_in;
use crate::screens_widget::TestColour;
use crate::{TestInstance, TestRecord, TestValue};

const STYLE_FOLDER: &str = "libs/foundry/records/ui/buildingblocks/styles/";

/// A selector of a synthetic style entry.
#[derive(Debug, Clone, PartialEq)]
pub enum TestCondition {
    /// The widget type, as the style data names it: `Card`, `Base` for a display widget, ...
    Type(&'static str),
    /// The widget has the `Tag` Record `guid` or one below it.
    Tag(u128),
    /// The widget has one of these tags or one below it.
    AnyOfTag(Vec<u128>),
    /// The parent widget meets every condition.
    Parent(Vec<Self>),
    /// Some ancestor widget meets every condition.
    Ancestor(Vec<Self>),
    /// Some ancestor widget meets every condition of the first list, looking up from the widget
    /// and stopping at an ancestor that meets every condition of the second.
    AncestorUntil(Vec<Self>, Vec<Self>),
    /// Some child widget meets every condition.
    Child(Vec<Self>),
    /// The widget is among the siblings the count selects: `First` or `Every`, the count and the
    /// offset.
    Count(&'static str, i32, i32),
    /// Every condition holds.
    AllOf(Vec<Self>),
    /// A selector the screens reader does not evaluate.
    Unknown,
}

impl TestCondition {
    fn value(&self) -> TestValue {
        TestValue::Struct(match self {
            Self::Type(kind) => TestInstance::new("BuildingBlocks_StyleSelectorConditionType")
                .value("type", TestValue::text(*kind)),
            Self::Tag(guid) => TestInstance::new("BuildingBlocks_StyleSelectorConditionTag")
                .value("tag", TestValue::Reference(*guid)),
            Self::AnyOfTag(tags) => {
                TestInstance::new("BuildingBlocks_StyleSelectorConditionAnyOfTag").value(
                    "tags",
                    TestValue::Array(tags.iter().copied().map(TestValue::Reference).collect()),
                )
            }
            Self::Parent(inner) => TestInstance::new("BuildingBlocks_StyleSelectorConditionParent")
                .value(
                    "conditions",
                    TestValue::Array(inner.iter().map(Self::value).collect()),
                ),
            Self::Ancestor(inner) => {
                TestInstance::new("BuildingBlocks_StyleSelectorConditionAncestor").value(
                    "conditions",
                    TestValue::Array(inner.iter().map(Self::value).collect()),
                )
            }
            Self::AncestorUntil(inner, breaks) => {
                TestInstance::new("BuildingBlocks_StyleSelectorConditionAncestor")
                    .value(
                        "conditions",
                        TestValue::Array(inner.iter().map(Self::value).collect()),
                    )
                    .value(
                        "breakConditions",
                        TestValue::Array(breaks.iter().map(Self::value).collect()),
                    )
            }
            Self::Child(inner) => TestInstance::new("BuildingBlocks_StyleSelectorConditionChild")
                .value(
                    "conditions",
                    TestValue::Array(inner.iter().map(Self::value).collect()),
                ),
            Self::Count(kind, count, offset) => {
                TestInstance::new("BuildingBlocks_StyleSelectorConditionCount")
                    .value("type", TestValue::text(*kind))
                    .value("count", TestValue::Int32(*count))
                    .value("offset", TestValue::Int32(*offset))
            }
            Self::AllOf(inner) => {
                TestInstance::new("BuildingBlocks_StyleSelectorConditionAllOfCondition").value(
                    "conditions",
                    TestValue::Array(inner.iter().map(Self::value).collect()),
                )
            }
            Self::Unknown => TestInstance::new("BuildingBlocks_StyleSelectorConditionUnknown"),
        })
    }
}

/// A modifier of a synthetic style entry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TestModifier {
    /// `BackgroundColor`.
    Background(TestColour),
    /// `FillColor`.
    Fill(TestColour),
    /// `StrokeColor`.
    Stroke(TestColour),
    /// `EnableBackground`.
    EnableBackground(bool),
    /// `IsActive`.
    Active(bool),
    /// `Alpha`.
    Alpha(f32),
    /// `FontSize`.
    FontSize(f32),
    /// `LetterSpacing`.
    LetterSpacing(f32),
    /// `LineSpacing`.
    LineSpacing(f32),
    /// `WordWrap`.
    WordWrap(bool),
    /// The font style Record `guid`.
    FontStyle(u128),
    /// The canvas Record `guid` a canvas widget holds (`CanvasReferenceRecord`).
    Canvas(u128),
    /// The horizontal alignment, `Left`, `Center` or `Right`.
    Horizontal(&'static str),
    /// The vertical alignment, `Top`, `Center` or `Bottom`.
    Vertical(&'static str),
    /// The case modifier, `None`, `Upper` or `Lower`.
    Case(&'static str),
    /// The localised `Text` field, e.g. a key as `@key`.
    Text(&'static str),
    /// `ImagePath`.
    ImagePath(&'static str),
    /// `SvgPath`.
    SvgPath(&'static str),
    /// `EnableColorOverlay`.
    ColourOverlay(bool),
    /// Any numeric field by its name, e.g. `AnchorX`, `SizeX` or `OrientationZ`.
    Number(&'static str, f32),
    /// An enumerated field: the kind, e.g. `WidthBehavior` or `FlexDirection`, and the option.
    Enumerated(&'static str, &'static str),
    /// An animation of `Alpha`.
    Animation(TestAlphaAnimation),
}

/// How a modifier is written: a named field of a kind, or a typed struct inside `field`.
enum Shape {
    Field(&'static str, &'static str, TestValue),
    Typed(&'static str, String, TestValue),
    Whole(TestValue),
}

impl TestModifier {
    fn shape(self) -> Shape {
        let enumerated =
            |inner: &str, name| Shape::Typed("Enumerated", inner.to_owned(), TestValue::text(name));
        let record = |inner: &str, guid| {
            Shape::Typed("RecordRef", inner.to_owned(), TestValue::Reference(guid))
        };
        match self {
            Self::Number(field, value) => Shape::Field("Number", field, TestValue::Single(value)),
            Self::Background(colour) => Shape::Field("Color", "BackgroundColor", colour.value()),
            Self::Fill(colour) => Shape::Field("Color", "FillColor", colour.value()),
            Self::Stroke(colour) => Shape::Field("Color", "StrokeColor", colour.value()),
            Self::EnableBackground(on) => {
                Shape::Field("Boolean", "EnableBackground", TestValue::Bool(on))
            }
            Self::Active(on) => Shape::Field("Boolean", "IsActive", TestValue::Bool(on)),
            Self::WordWrap(on) => Shape::Field("Boolean", "WordWrap", TestValue::Bool(on)),
            Self::Alpha(alpha) => Shape::Field("Number", "Alpha", TestValue::Single(alpha)),
            Self::FontSize(size) => Shape::Field("Number", "FontSize", TestValue::Single(size)),
            Self::LetterSpacing(space) => {
                Shape::Field("Number", "LetterSpacing", TestValue::Single(space))
            }
            Self::LineSpacing(space) => {
                Shape::Field("Number", "LineSpacing", TestValue::Single(space))
            }
            Self::Text(text) => Shape::Field("Localization", "Text", TestValue::text(text)),
            Self::ImagePath(path) => Shape::Field("String", "ImagePath", TestValue::text(path)),
            Self::SvgPath(path) => Shape::Field("String", "SvgPath", TestValue::text(path)),
            Self::ColourOverlay(on) => {
                Shape::Field("Boolean", "EnableColorOverlay", TestValue::Bool(on))
            }
            Self::FontStyle(guid) => record("RecordRefTypeFontStyleRecord", guid),
            Self::Canvas(guid) => record("RecordRefTypeCanvasReferenceRecord", guid),
            Self::Horizontal(name) => enumerated("EnumeratedTypeHorizontalAlignment", name),
            Self::Vertical(name) => enumerated("EnumeratedTypeVerticalAlignment", name),
            Self::Case(name) => enumerated("EnumeratedTypeCaseModifier", name),
            Self::Enumerated(kind, name) => enumerated(&format!("EnumeratedType{kind}"), name),
            Self::Animation(animation) => Shape::Whole(animation.value()),
        }
    }

    pub(crate) fn value(self) -> TestValue {
        match self.shape() {
            Shape::Whole(value) => value,
            Shape::Typed(kind, inner, value) => typed(kind, &inner, value),
            Shape::Field(kind, field, value) => {
                let key = if kind == "Color" { "color" } else { "value" };
                TestValue::Struct(
                    TestInstance::new(format!("BuildingBlocks_FieldModifier{kind}"))
                        .value("field", TestValue::text(field))
                        .value(key, value),
                )
            }
        }
    }
}

fn typed(kind: &str, inner: &str, value: TestValue) -> TestValue {
    TestValue::Struct(
        TestInstance::new(format!("BuildingBlocks_FieldModifier{kind}")).value(
            "field",
            TestValue::Struct(
                TestInstance::new(format!("BuildingBlocks_FieldModifier{inner}"))
                    .value("value", value),
            ),
        ),
    )
}

/// One style entry: when its conditions hold, its modifiers apply.
#[derive(Debug, Clone, PartialEq)]
pub struct TestStyleEntry {
    name: String,
    conditions: Vec<TestCondition>,
    other_lists: Vec<Vec<TestCondition>>,
    modifiers: Vec<TestModifier>,
}

impl TestStyleEntry {
    /// Creates an entry named `name` without conditions or modifiers.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            conditions: Vec::new(),
            other_lists: Vec::new(),
            modifiers: Vec::new(),
        }
    }

    /// Adds a second list of conditions: the entry applies when either list holds.
    #[must_use]
    pub fn or_list(mut self, conditions: Vec<TestCondition>) -> Self {
        self.other_lists.push(conditions);
        self
    }

    /// Adds a condition that must hold together with the others.
    #[must_use]
    pub fn when(mut self, condition: TestCondition) -> Self {
        self.conditions.push(condition);
        self
    }

    /// Adds a modifier.
    #[must_use]
    pub fn set(mut self, modifier: TestModifier) -> Self {
        self.modifiers.push(modifier);
        self
    }

    pub(crate) fn value(&self) -> TestValue {
        let lists = std::iter::once(&self.conditions).chain(&self.other_lists);
        TestValue::Struct(
            TestInstance::new("BuildingBlocks_StyleEntry")
                .value("name", TestValue::text(self.name.clone()))
                .value(
                    "conditionsList",
                    TestValue::Array(
                        lists
                            .map(|list| {
                                TestValue::Struct(
                                    TestInstance::new("BuildingBlocks_StyleConditionList")
                                        .value("name", TestValue::text("list"))
                                        .value(
                                            "conditions",
                                            TestValue::Array(
                                                list.iter().map(TestCondition::value).collect(),
                                            ),
                                        ),
                                )
                            })
                            .collect(),
                    ),
                )
                .value(
                    "modifiers",
                    TestValue::Array(self.modifiers.iter().map(|m| m.value()).collect()),
                ),
        )
    }
}

pub(crate) fn entries(entries: &[TestStyleEntry]) -> TestValue {
    TestValue::Array(entries.iter().map(TestStyleEntry::value).collect())
}

/// A synthetic `BuildingBlocks_Style` Record: a palette and style entries.
#[derive(Debug, Clone, PartialEq)]
pub struct TestStyle {
    guid: u128,
    name: String,
    palette: Vec<TestColour>,
    entries: Vec<TestStyleEntry>,
}

impl TestStyle {
    /// Creates a style whose palette lists `palette` for the slots of
    /// [`SCREEN_COLOUR_SLOTS`](crate::SCREEN_COLOUR_SLOTS) in order.
    #[must_use]
    pub fn new(guid: u128, name: impl Into<String>, palette: Vec<TestColour>) -> Self {
        Self {
            guid,
            name: name.into(),
            palette,
            entries: Vec::new(),
        }
    }

    /// Adds a style entry.
    #[must_use]
    pub fn entry(mut self, entry: TestStyleEntry) -> Self {
        self.entries.push(entry);
        self
    }

    /// Returns the Record, at the record path `.../styles/<name>` where the HUD styles of the
    /// manufacturers are.
    #[must_use]
    pub fn record(&self) -> TestRecord {
        record_in(STYLE_FOLDER, self.guid, "BuildingBlocks_Style", &self.name)
            .value("entries", entries(&self.entries))
            .value(
                "colorStyles",
                TestValue::Array(self.palette.iter().map(|c| c.value()).collect()),
            )
    }
}

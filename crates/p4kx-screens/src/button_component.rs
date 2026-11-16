// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::StructValue;

use crate::canvas::Canvas;
use crate::reader::{child, flag, float, text};
use crate::report::Report;
use crate::rest::{Rest, parameter_for_tag, same_parameter};
use crate::style::TagPaths;
use crate::walk_rest::Handover;

pub(crate) const SECONDARY: &str = "BuildingBlocks_ComponentGeneralButtonSecondary";

#[derive(Debug, Clone)]
pub(crate) struct ButtonComponent {
    label: Part,
    icon: Part,
    text: Option<String>,
    has_icon_override: bool,
    fill: Choice,
    position: Choice,
}

#[derive(Debug, Clone, Copy)]
struct Part {
    shown: Option<bool>,
    anchor: [Option<f32>; 2],
    scale: Option<f32>,
}

impl Part {
    fn read(part: Option<&StructValue<'_>>) -> Self {
        let number = |name| {
            part.and_then(|part| float(part, name))
                .filter(|v| v.is_finite())
        };
        Self {
            shown: part.and_then(|part| flag(part, "show")),
            anchor: [number("anchorToParentX"), number("anchorToParentY")],
            scale: number("scale"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Choice {
    Absent,
    Known(&'static str),
    Unrecognised,
}

impl Choice {
    fn read(source: &StructValue<'_>, field: &str, leaves: &[(&str, &'static str)]) -> Self {
        if source.field(field).is_none() {
            return Self::Absent;
        }
        let Some(value) = text(source, field) else {
            return Self::Unrecognised;
        };
        leaves
            .iter()
            .find(|(name, _)| *name == value)
            .map_or(Self::Unrecognised, |(_, leaf)| Self::Known(leaf))
    }
}

impl ButtonComponent {
    pub(crate) fn read(source: &StructValue<'_>) -> Option<Self> {
        if source.type_name() != SECONDARY {
            return None;
        }
        let label = child(source, "labelProperties");
        let icon = child(source, "iconProperties");
        let has_icon_override = icon.is_some_and(|icon| {
            text(icon, "customIcon").is_some_and(|path| !path.is_empty())
                || text(icon, "iconPreset").is_some_and(|name| !matches!(name, "" | "_None"))
        });
        Some(Self {
            label: Part::read(label),
            icon: Part::read(icon),
            text: label
                .and_then(|label| text(label, "label"))
                .map(str::to_owned),
            has_icon_override,
            fill: Choice::read(
                source,
                "fillStyle",
                &[("Filled", "fillstylefilled"), ("Ghost", "fillstyleghost")],
            ),
            position: Choice::read(
                source,
                "iconPosition",
                &[
                    ("Top", "iconpostop"),
                    ("Right", "iconposright"),
                    ("Bottom", "iconposbottom"),
                    ("Left", "iconposleft"),
                ],
            ),
        })
    }

    pub(crate) fn transfer(
        &self,
        (canvas, proxy): (&Canvas, usize),
        tags: &TagPaths,
        values: &mut Handover<'_>,
        report: &mut Report,
    ) {
        const LABEL: &str = "ParamInput0";
        if let Some(label) = &self.text
            && !values.text.iter().any(|(name, _)| name == LABEL)
        {
            values.text = values
                .text
                .iter()
                .cloned()
                .chain([(LABEL.to_owned(), label.clone())])
                .collect();
        }
        let mut typed: Vec<_> = values.typed.iter().cloned().collect();
        for (name, value) in self.part_values() {
            if let Some(value) = value {
                add_parameter(&mut typed, name, value);
            }
        }
        for (field, setting, description) in [
            ("PrimaryStateTag", self.fill, "fill style"),
            ("SecondaryStateTag", self.position, "icon position"),
        ] {
            let selected = match setting {
                Choice::Absent => continue,
                Choice::Unrecognised => None,
                Choice::Known(leaf) => {
                    parameter_for_tag(&canvas.operations, (proxy, field), |tag| {
                        tags.name_of(tag)
                            .is_some_and(|name| leaf_matches(name, leaf))
                    })
                }
            };
            if let Some((parameter, key)) = selected {
                add_parameter(&mut typed, parameter, Rest::Integer(key));
            } else {
                report.note(
                    SECONDARY,
                    &format!("button {description} does not select an authored template case"),
                    &values.widget.name,
                );
            }
        }
        values.typed = typed.into();
        if self.icon.shown == Some(true) && self.has_icon_override {
            report.note(
                SECONDARY,
                "the component's icon asset override is not evaluated",
                &values.widget.name,
            );
        }
    }

    fn part_values(&self) -> [(&'static str, Option<Rest>); 8] {
        let number = |value: Option<f32>| value.map(|value| Rest::Num(f64::from(value)));
        [
            ("ParamInput0", self.label.shown.map(Rest::Bool)),
            ("ParamInput1", self.icon.shown.map(Rest::Bool)),
            ("ParamInput0", number(self.icon.anchor[0])),
            ("ParamInput1", number(self.icon.anchor[1])),
            ("ParamInput2", number(self.label.anchor[0])),
            ("ParamInput3", number(self.label.anchor[1])),
            ("ParamInput4", number(self.icon.scale)),
            ("ParamInput5", number(self.label.scale)),
        ]
    }
}

fn add_parameter(typed: &mut Vec<(String, Rest)>, name: &str, value: Rest) {
    if !typed
        .iter()
        .any(|(given, existing)| same_parameter(name, &value, given, existing))
    {
        typed.push((name.to_owned(), value));
    }
}

fn leaf_matches(name: &str, leaf: &str) -> bool {
    name.bytes()
        .filter(u8::is_ascii_alphanumeric)
        .map(|byte| byte.to_ascii_lowercase())
        .eq(leaf.bytes())
}

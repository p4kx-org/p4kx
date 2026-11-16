// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::StructValue;

use crate::reader::{float, text};

#[path = "flex_arrangement.rs"]
mod arrangement;

pub(crate) use arrangement::{Item, LayoutApproximation, Slot, arrange, limit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Justify {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Align {
    Start,
    Center,
    End,
    Stretch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Wrap {
    No,
    Yes,
    Reverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContainerKind {
    Flex,
    Stack,
    Grid,
}

/// A container that lays its children out in lines along one axis: a flex container, a stack or
/// a grid pack.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Flex {
    pub(crate) kind: ContainerKind,
    pub(crate) column: bool,
    pub(crate) reverse: bool,
    pub(crate) wrap: Wrap,
    pub(crate) infinite: bool,
    pub(crate) justify: Justify,
    pub(crate) lines: Justify,
    pub(crate) align: Align,
    pub(crate) column_gap: f32,
    pub(crate) row_gap: f32,
}

impl Flex {
    const fn main_gap(&self) -> f32 {
        if self.column {
            self.row_gap
        } else {
            self.column_gap
        }
    }

    const fn cross_gap(&self) -> f32 {
        if self.column {
            self.column_gap
        } else {
            self.row_gap
        }
    }

    /// Returns how the lines are placed across: the start of a container that wraps in reverse
    /// is the end of the cross axis.
    const fn line_justification(&self) -> Justify {
        match (self.wrap, self.lines) {
            (Wrap::Reverse, Justify::Start) => Justify::End,
            (Wrap::Reverse, Justify::End) => Justify::Start,
            (_, how) => how,
        }
    }

    /// Replaces the column and row spacing a style sets.
    pub(crate) const fn apply_gaps(&mut self, [column, row]: [Option<f32>; 2]) {
        if let Some(gap) = column {
            self.column_gap = gap;
        }
        if let Some(gap) = row {
            self.row_gap = gap;
        }
    }
}

pub(crate) fn justify(name: Option<&str>) -> Justify {
    match name.unwrap_or("Start").to_ascii_lowercase().as_str() {
        "center" => Justify::Center,
        "end" | "flexend" => Justify::End,
        "spacebetween" => Justify::SpaceBetween,
        "spacearound" => Justify::SpaceAround,
        "spaceevenly" => Justify::SpaceEvenly,
        "stretch" => Justify::Stretch,
        _ => Justify::Start,
    }
}

pub(crate) fn align(name: Option<&str>) -> Align {
    match name.unwrap_or("Start").to_ascii_lowercase().as_str() {
        "center" => Align::Center,
        "end" | "flexend" => Align::End,
        "stretch" => Align::Stretch,
        _ => Align::Start,
    }
}

const FLEX: &str = "BuildingBlocks_FlexContainer";
const STACK: &str = "BuildingBlocks_StackContainer";
const GRID: &str = "BuildingBlocks_GridPackContainer";

/// Reads a layout policy: the container it makes, or the name of a policy that has no
/// counterpart here.
pub(crate) fn parse_flex(layout: &StructValue<'_>) -> (Option<Flex>, Option<String>) {
    let spacing = |name| float(layout, name).unwrap_or(0.0);
    let vertical = text(layout, "direction") == Some("Vertical");
    let flex = match layout.type_name() {
        FLEX => {
            let direction = text(layout, "direction").unwrap_or("Row");
            let wrap = text(layout, "wrap").unwrap_or("NoWrap");
            Flex {
                kind: ContainerKind::Flex,
                column: direction.contains("Column"),
                reverse: direction.contains("Reverse"),
                wrap: match wrap {
                    "Wrap" => Wrap::Yes,
                    "WrapReverse" => Wrap::Reverse,
                    _ => Wrap::No,
                },
                infinite: wrap == "NoWrapInfinite",
                justify: justify(text(layout, "axisJustification")),
                lines: justify(text(layout, "crossAxisJustification")),
                align: align(text(layout, "itemAlignment")),
                column_gap: spacing("columnSpacing"),
                row_gap: spacing("rowSpacing"),
            }
        }
        STACK => Flex {
            kind: ContainerKind::Stack,
            column: vertical,
            reverse: false,
            wrap: Wrap::No,
            infinite: true,
            justify: Justify::Start,
            lines: Justify::Start,
            align: Align::Start,
            column_gap: spacing("spacing"),
            row_gap: spacing("spacing"),
        },
        GRID => Flex {
            kind: ContainerKind::Grid,
            column: vertical,
            reverse: false,
            wrap: Wrap::Yes,
            infinite: false,
            justify: Justify::Start,
            lines: Justify::Start,
            align: Align::Start,
            column_gap: spacing("gutter"),
            row_gap: spacing("gutter"),
        },
        other => return (None, Some(other.to_owned())),
    };
    (Some(flex), None)
}

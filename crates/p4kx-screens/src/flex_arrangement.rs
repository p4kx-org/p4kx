// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use super::{Align, ContainerKind, Flex, Justify, Wrap};

/// A child of a container with its outer size, margins included, and how it may grow, shrink
/// and be bounded along the axes.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Item {
    pub(crate) size: [f32; 2],
    pub(crate) fixed_main_size: bool,
    pub(crate) measured_auto_cross_size: bool,
    pub(crate) grow: f32,
    pub(crate) shrink: f32,
    pub(crate) min: [f32; 2],
    pub(crate) max: [f32; 2],
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Slot {
    pub(crate) position: [f32; 2],
    pub(crate) size: [f32; 2],
}

/// The final slots and the layout assumptions that affected their positions or sizes.
#[derive(Default)]
pub(crate) struct Arrangement {
    pub(crate) slots: Vec<Slot>,
    approximations: [Option<LayoutApproximation>; 4],
}

impl Arrangement {
    /// Returns the limitations of the effective policy that affected this placement.
    pub(crate) fn approximations(&self) -> impl Iterator<Item = LayoutApproximation> {
        self.approximations.into_iter().flatten()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum LayoutApproximation {
    FixedStretch,
    SeparateCrossAlignment,
    ReversedCrossStart,
    AutomaticStackCross,
}

impl LayoutApproximation {
    pub(crate) const fn reason(self) -> &'static str {
        match self {
            Self::FixedStretch => {
                "Effective layoutPolicy.axisJustification=Stretch shares spare space with fixed sizing.width or sizing.height. The game rule is unverified."
            }
            Self::SeparateCrossAlignment => {
                "Effective layoutPolicy.crossAxisJustification positions the single line and itemAlignment positions its children separately. The game rule is unverified."
            }
            Self::ReversedCrossStart => {
                "Effective layoutPolicy.wrap=WrapReverse flips Start and End on crossAxisJustification. The cross start in the game is unverified."
            }
            Self::AutomaticStackCross => {
                "Stack layoutPolicy.direction keeps a child's automatic cross-axis sizing.width or sizing.height based on its content. The game rule is unverified."
            }
        }
    }
}

fn at(pair: [f32; 2], axis: usize) -> f32 {
    pair.get(axis).copied().unwrap_or(0.0)
}

fn set(pair: &mut [f32; 2], axis: usize, value: f32) {
    if let Some(slot) = pair.get_mut(axis) {
        *slot = value;
    }
}

fn number(count: usize) -> f32 {
    f32::from(u16::try_from(count).unwrap_or(u16::MAX))
}

/// Holds `value` between `min` and `max`, the minimum winning, and above zero.
pub(crate) const fn limit(value: f32, min: f32, max: f32) -> f32 {
    value.min(max).max(min).max(0.0)
}

fn break_lines(flex: &Flex, items: &[Item], main: usize, extent: f32) -> Vec<Range<usize>> {
    if flex.wrap == Wrap::No || items.is_empty() {
        return std::iter::once(0..items.len()).collect();
    }
    let mut lines = Vec::new();
    let mut start = 0;
    let mut filled = 0.0;
    for (index, item) in items.iter().enumerate() {
        let size = at(item.size, main);
        let gap = if index > start { flex.main_gap() } else { 0.0 };
        if index > start && filled + gap + size > extent {
            lines.push(start..index);
            start = index;
            filled = size;
        } else {
            filled += gap + size;
        }
    }
    lines.push(start..items.len());
    lines
}

fn distribute(
    flex: &Flex,
    items: &[Item],
    line: &Range<usize>,
    main: usize,
    extent: f32,
    stretched_fixed: &mut bool,
) -> Vec<f32> {
    let members = items.get(line.clone()).unwrap_or(&[]);
    let mut sizes: Vec<f32> = members.iter().map(|item| at(item.size, main)).collect();
    let gaps = flex.main_gap() * (number(members.len()) - 1.0).max(0.0);
    let free = extent - sizes.iter().sum::<f32>() - gaps;
    if free > 0.0 {
        let total: f32 = members.iter().map(|item| item.grow.max(0.0)).sum();
        let stretch = flex.justify == Justify::Stretch && total <= 0.0;
        for (size, item) in sizes.iter_mut().zip(members) {
            let share = if stretch {
                free / number(members.len())
            } else if total > 0.0 {
                free * item.grow.max(0.0) / total
            } else {
                0.0
            };
            let next = limit(*size + share, at(item.min, main), at(item.max, main));
            *stretched_fixed |= stretch && item.fixed_main_size && next > *size;
            *size = next;
        }
    } else if free < 0.0 && !flex.infinite {
        let weights: Vec<f32> = members
            .iter()
            .zip(&sizes)
            .map(|(item, size)| item.shrink.max(0.0) * size)
            .collect();
        let total: f32 = weights.iter().sum();
        if total > 0.0 {
            for ((size, item), weight) in sizes.iter_mut().zip(members).zip(weights) {
                *size = limit(
                    *size + free * weight / total,
                    at(item.min, main),
                    at(item.max, main),
                );
            }
        }
    }
    sizes
}

fn spread(how: Justify, spare: f32, count: usize) -> (f32, f32) {
    let spare = spare.max(0.0);
    let count = number(count);
    match how {
        Justify::Center => (spare / 2.0, 0.0),
        Justify::End => (spare, 0.0),
        Justify::SpaceBetween if count > 1.0 => (0.0, spare / (count - 1.0)),
        Justify::SpaceAround => (spare / count.max(1.0) / 2.0, spare / count.max(1.0)),
        Justify::SpaceEvenly => (spare / (count + 1.0), spare / (count + 1.0)),
        Justify::Start | Justify::SpaceBetween | Justify::Stretch => (0.0, 0.0),
    }
}

/// Places the items in lines using the effective container policy.
pub(crate) fn arrange(flex: &Flex, content: [f32; 2], items: &[Item]) -> Arrangement {
    let main = usize::from(flex.column);
    let cross = 1 - main;
    let main_extent = at(content, main);
    let cross_extent = at(content, cross);
    let lines = break_lines(flex, items, main, main_extent);
    let mut placed = vec![Slot::default(); items.len()];
    let mut stretched_fixed = false;
    let mains: Vec<Vec<f32>> = lines
        .iter()
        .map(|line| distribute(flex, items, line, main, main_extent, &mut stretched_fixed))
        .collect();
    let (spans, free) = cross_spans(flex, items, &lines, cross, cross_extent);
    let (lead, between) = spread(flex.line_justification(), free, lines.len());
    let order: Vec<usize> = if flex.wrap == Wrap::Reverse {
        (0..lines.len()).rev().collect()
    } else {
        (0..lines.len()).collect()
    };
    let mut cursor = lead;
    for line_at in order {
        let (Some(line), Some(sizes), Some(&span)) =
            (lines.get(line_at), mains.get(line_at), spans.get(line_at))
        else {
            continue;
        };
        place_line(
            flex,
            items,
            line,
            sizes,
            (cursor, span),
            main_extent,
            &mut placed,
        );
        cursor += span + flex.cross_gap() + between;
    }
    let separate_cross_alignment = lines.len() == 1
        && spans
            .first()
            .is_some_and(|span| (span - cross_extent).abs() > f32::EPSILON)
        && differs_from_full_line(flex, items, &placed, cross, cross_extent);
    let stack_auto_cross = flex.kind == ContainerKind::Stack
        && stack_uses_measured_cross_size(items, &placed, cross, cross_extent);
    let reverse_cross_start = flex.wrap == Wrap::Reverse
        && !items.is_empty()
        && free > 0.0
        && matches!(flex.lines, Justify::Start | Justify::End);
    Arrangement {
        slots: placed,
        approximations: [
            stretched_fixed.then_some(LayoutApproximation::FixedStretch),
            separate_cross_alignment.then_some(LayoutApproximation::SeparateCrossAlignment),
            reverse_cross_start.then_some(LayoutApproximation::ReversedCrossStart),
            stack_auto_cross.then_some(LayoutApproximation::AutomaticStackCross),
        ],
    }
}

fn stack_uses_measured_cross_size(
    items: &[Item],
    slots: &[Slot],
    axis: usize,
    extent: f32,
) -> bool {
    items.iter().zip(slots).any(|(item, slot)| {
        let filled = limit(extent, at(item.min, axis), at(item.max, axis));
        item.measured_auto_cross_size && (at(slot.size, axis) - filled).abs() > f32::EPSILON
    })
}

fn cross_spans(
    flex: &Flex,
    items: &[Item],
    lines: &[Range<usize>],
    cross: usize,
    extent: f32,
) -> (Vec<f32>, f32) {
    let mut spans: Vec<f32> = lines
        .iter()
        .map(|line| {
            items
                .get(line.clone())
                .unwrap_or_default()
                .iter()
                .map(|item| at(item.size, cross))
                .fold(0.0_f32, f32::max)
        })
        .collect();
    let gaps = flex.cross_gap() * (number(lines.len()) - 1.0).max(0.0);
    let free = extent - spans.iter().sum::<f32>() - gaps;
    if flex.lines == Justify::Stretch && free > 0.0 {
        let share = free / number(lines.len());
        for span in &mut spans {
            *span += share;
        }
    }
    let free = extent - spans.iter().sum::<f32>() - gaps;
    (spans, free)
}

fn differs_from_full_line(
    flex: &Flex,
    items: &[Item],
    slots: &[Slot],
    axis: usize,
    extent: f32,
) -> bool {
    items.iter().zip(slots).any(|(item, slot)| {
        let (offset, size) = align_cross(flex.align, item, axis, extent);
        (at(slot.position, axis) - offset).abs() > f32::EPSILON
            || (at(slot.size, axis) - size).abs() > f32::EPSILON
    })
}

fn align_cross(align: Align, item: &Item, axis: usize, span: f32) -> (f32, f32) {
    let size = at(item.size, axis);
    let room = span - size;
    match align {
        Align::Start => (0.0, size),
        Align::Center => (room / 2.0, size),
        Align::End => (room, size),
        Align::Stretch => (0.0, limit(span, at(item.min, axis), at(item.max, axis))),
    }
}

fn place_line(
    flex: &Flex,
    items: &[Item],
    line: &Range<usize>,
    sizes: &[f32],
    (line_start, span): (f32, f32),
    extent: f32,
    placed: &mut [Slot],
) {
    let main = usize::from(flex.column);
    let cross = 1 - main;
    let used = flex.main_gap().mul_add(
        (number(sizes.len()) - 1.0).max(0.0),
        sizes.iter().sum::<f32>(),
    );
    let (lead, between) = spread(flex.justify, extent - used, sizes.len());
    let indices: Vec<usize> = if flex.reverse {
        line.clone().rev().collect()
    } else {
        line.clone().collect()
    };
    let mut cursor = lead;
    for index in indices {
        let (Some(item), Some(&along)) = (items.get(index), sizes.get(index - line.start)) else {
            continue;
        };
        let mut size = item.size;
        set(&mut size, main, along);
        let (across, cross_size) = align_cross(flex.align, item, cross, span);
        set(&mut size, cross, cross_size);
        let mut position = [0.0; 2];
        set(&mut position, main, cursor);
        set(&mut position, cross, line_start + across);
        cursor += along + flex.main_gap() + between;
        if let Some(slot) = placed.get_mut(index) {
            *slot = Slot { position, size };
        }
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::super::{Align, ContainerKind, Flex, Justify, Wrap};
use super::{Item, Slot, arrange};

#[test]
fn stretches_every_item_when_the_count_exceeds_u16() {
    let policy = Flex {
        kind: ContainerKind::Flex,
        column: false,
        reverse: false,
        wrap: Wrap::No,
        infinite: false,
        justify: Justify::Stretch,
        lines: Justify::Start,
        align: Align::Start,
        column_gap: 1.0,
        row_gap: 0.0,
    };
    let items = vec![
        Item {
            size: [1.0, 1.0],
            fixed_main_size: true,
            measured_auto_cross_size: false,
            grow: 0.0,
            shrink: 0.0,
            min: [0.0; 2],
            max: [f32::INFINITY; 2],
        };
        usize::from(u16::MAX) + 1
    ];
    let count = f32::from(u16::MAX) + 1.0;
    let stretched_width = 2.0_f32;
    let extent = stretched_width.mul_add(count, policy.column_gap * (count - 1.0));
    let layout = arrange(&policy, [extent, 1.0], &items);
    assert_eq!(
        layout.slots.first(),
        Some(&Slot {
            position: [0.0, 0.0],
            size: [stretched_width, 1.0],
        }),
        "the first item gets an equal share of the space after the real gap count",
    );
    assert_eq!(
        layout.slots.last(),
        Some(&Slot {
            position: [extent - stretched_width, 0.0],
            size: [stretched_width, 1.0],
        }),
        "the last item finishes at the container edge",
    );
}

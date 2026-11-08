// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use p4kx_extraction::Guid;

use crate::contract_recipe::CostLine;

/// Appends every resource GUID in the existing depth-first cost order.
pub(crate) fn collect(line: &CostLine<'_>, out: &mut Vec<Guid>) {
    use std::convert::Infallible;
    let _: ControlFlow<Infallible> = walk(line, &mut |guid| {
        out.push(guid);
        ControlFlow::Continue(())
    });
}

/// Tests the same resource selection without allocating an intermediate list.
#[must_use]
pub(crate) fn contains(line: &CostLine<'_>, wanted: Guid) -> bool {
    walk(line, &mut |guid| {
        if guid == wanted {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    })
    .is_break()
}

fn walk<B>(line: &CostLine<'_>, emit: &mut impl FnMut(Guid) -> ControlFlow<B>) -> ControlFlow<B> {
    if let Some(guid) = line.resource_guid {
        emit(guid.0)?;
    }
    for option in &line.options {
        walk(option, emit)?;
    }
    ControlFlow::Continue(())
}

#[cfg(test)]
#[path = "cost_resources_tests.rs"]
mod tests;

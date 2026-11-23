// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::DataForge;
use crate::query::source::{QuerySource, QueryUnavailable};

/// Reads an original instance while retaining the outcome of its physical extent lookup.
pub(super) fn read<'source>(
    forge: &'source DataForge<'source>,
    structure: u32,
    instance: u32,
    observe: Option<impl FnMut(QuerySource)>,
) -> Option<&'source [u8]> {
    let bytes = forge.instance(structure, instance);
    if let Some(mut observe) = observe {
        if usize::try_from(structure).is_ok_and(|index| index < forge.struct_count()) {
            extent(structure, instance, bytes.is_some(), observe);
        } else {
            observe(QuerySource::Unavailable(QueryUnavailable::Instance {
                structure,
                instance,
            }));
        }
    }
    bytes
}

pub(super) fn extent(
    structure: u32,
    instance: u32,
    resolved: bool,
    mut observe: impl FnMut(QuerySource),
) {
    observe(QuerySource::InstanceExtent {
        structure,
        resolved,
    });
    if !resolved {
        observe(QuerySource::Unavailable(QueryUnavailable::Instance {
            structure,
            instance,
        }));
    }
}

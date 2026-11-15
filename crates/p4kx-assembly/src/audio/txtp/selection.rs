// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::ptr;

use crate::audio::hirc_path_source::{HircChildSource, HircRouteSource};
use crate::audio::{HircPathSource, SoundVariant};

use super::render::RenderedCommand;

pub(super) struct SelectionDiagnostic<'a> {
    pub(super) commands: &'a [RenderedCommand<'a>],
    pub(super) routes: Vec<&'a HircRouteSource>,
    pub(super) children: Vec<&'a HircChildSource>,
}

pub(super) fn resolve<'a>(
    variants: &'a [SoundVariant<'_>],
    commands: &'a [RenderedCommand<'a>],
) -> SelectionDiagnostic<'a> {
    let paths = commands
        .iter()
        .filter_map(|command| variants.get(command.variant))
        .flat_map(SoundVariant::path_sources);
    let mut recorded_routes = HashSet::new();
    let routes = paths
        .clone()
        .filter_map(HircPathSource::inbound_route)
        .filter(|route| recorded_routes.insert(ptr::from_ref(*route)))
        .collect();
    let mut recorded_children = HashSet::new();
    let children = paths
        .filter_map(HircPathSource::inbound_child)
        .filter(|child| recorded_children.insert((ptr::from_ref(child.owner()), child.ordinal())))
        .collect();
    SelectionDiagnostic {
        commands,
        routes,
        children,
    }
}

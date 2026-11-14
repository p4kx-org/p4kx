// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::CreatureAnimationSourceReport;
use p4kx_extraction::{SourceInventory, SourcedValue};
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct AnimationSourcesJson<'a> {
    owner: OwnerJson<'a>,
    clips: Vec<ClipJson<'a>>,
    selection_complete: bool,
    source_inventory: &'a SourceInventory,
}

#[derive(Serialize)]
struct OwnerJson<'a> {
    database: &'a SourcedValue<String>,
    geometry: &'a SourcedValue<String>,
    skeleton: &'a SourcedValue<String>,
    parameters: &'a SourcedValue<String>,
}

#[derive(Serialize)]
struct ClipJson<'a> {
    entry: &'a SourcedValue<String>,
    ordinal: &'a SourcedValue<usize>,
    graph_selectors: &'a [SourcedValue<String>],
}

impl<'a> From<&'a CreatureAnimationSourceReport> for AnimationSourcesJson<'a> {
    fn from(report: &'a CreatureAnimationSourceReport) -> Self {
        Self {
            owner: OwnerJson {
                database: &report.owner.database,
                geometry: &report.owner.geometry,
                skeleton: &report.owner.skeleton,
                parameters: &report.owner.parameters,
            },
            clips: report
                .clips
                .iter()
                .map(|clip| ClipJson {
                    entry: &clip.entry,
                    ordinal: &clip.ordinal,
                    graph_selectors: &clip.graph_selectors,
                })
                .collect(),
            selection_complete: report.selection_complete,
            source_inventory: &report.source_inventory,
        }
    }
}

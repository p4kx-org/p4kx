// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use anyhow::{Result, ensure};
use p4kx_extraction::{SourceLocation, SourceUse, SourceUseRole};
use p4kx_screens::{CanvasInput, CanvasRouteInput};

use super::Projection;

pub(super) struct CanvasRoutes {
    root_record: Option<u32>,
    direct: HashMap<u32, DirectRoute>,
}

struct DirectRoute {
    parent_record: u32,
    selector: SourceUse,
}

impl CanvasRoutes {
    pub(super) fn append(&self, mut record: u32, sources: &mut Vec<SourceUse>) -> bool {
        while self.root_record != Some(record) {
            let Some(edge) = self.direct.get(&record) else {
                return false;
            };
            sources.push(edge.selector.clone());
            record = edge.parent_record;
        }
        true
    }
}

impl Projection {
    pub(super) fn canvas_routes(
        &self,
        canvas: CanvasInput,
        reads: &[CanvasRouteInput],
    ) -> Result<CanvasRoutes> {
        let root_record = match canvas {
            CanvasInput::Read { record_index, .. } => Some(record_index),
            _ => None,
        };
        let mut direct = HashMap::new();
        let mut seen = HashSet::new();
        for read in reads {
            if let Some(widget) = read.through_widget {
                ensure!(
                    seen.contains(&widget.record_index),
                    "canvas parent was not read"
                );
                let selector = self.scope.at(
                    SourceLocation::DataForge {
                        record_index: widget.record_index,
                        field: format!("{}[{}].canvas", widget.section.field_name(), widget.slot),
                    },
                    SourceUseRole::Selector,
                )?;
                direct.insert(
                    read.record_index,
                    DirectRoute {
                        parent_record: widget.record_index,
                        selector,
                    },
                );
            }
            ensure!(seen.insert(read.record_index), "canvas was read twice");
        }
        Ok(CanvasRoutes {
            root_record,
            direct,
        })
    }
}

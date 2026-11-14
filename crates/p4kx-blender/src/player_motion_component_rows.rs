// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::player_bundle_wire::Row;
use crate::soc_scene_class_resources::Resource;

use super::MotionComponent;

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MotionComponentFields<'a> {
    ordinal: usize,
    controller_path: &'a SourcedValue<String>,
    third_person_database_path: &'a SourcedValue<String>,
    controller: &'a SourcedValue<Resource<usize>>,
    third_person_database: &'a SourcedValue<Resource<usize>>,
}

impl MotionComponent {
    pub(crate) fn bundle_rows(
        &self,
        index: usize,
        push: &mut impl FnMut(&Row<'_>) -> io::Result<()>,
    ) -> io::Result<()> {
        let fields = MotionComponentFields {
            ordinal: self.ordinal,
            controller_path: &self.controller_path,
            third_person_database_path: &self.third_person_database_path,
            controller: &self.controller,
            third_person_database: &self.third_person_database,
        };
        push(&Row::MotionComponent {
            index,
            value: fields,
        })?;
        if let Some(value) = &self.owner {
            push(&Row::MotionControllerTopology {
                component: index,
                value,
            })?;
        }
        if let Some(options) = &self.authored_options {
            options.bundle_rows(index, push)?;
        }
        if let Some(graph) = &self.database_graph {
            graph.bundle_rows(index, push)?;
        }
        Ok(())
    }
}

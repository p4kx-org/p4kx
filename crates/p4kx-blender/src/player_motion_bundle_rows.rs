// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::player_bundle_wire::Row;
use crate::soc_scene_class_resources::Resource;

use super::MotionOptions;

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MotionOptionFields<'a> {
    #[serde(rename = "controllerFragmentFile")]
    controller: &'a SourcedValue<String>,
    #[serde(rename = "databaseFragmentFile")]
    database: &'a SourcedValue<String>,
    #[serde(rename = "sharedFragmentFile")]
    shared: &'a SourcedValue<Resource<usize>>,
}

impl MotionOptions {
    pub(in crate::player_source_motion) fn bundle_rows(
        &self,
        component: usize,
        push: &mut impl FnMut(&Row<'_>) -> io::Result<()>,
    ) -> io::Result<()> {
        push(&Row::MotionOptions {
            component,
            value: MotionOptionFields {
                controller: &self.controller_fragment_file,
                database: &self.database_fragment_file,
                shared: &self.shared_fragment_file,
            },
        })?;
        for (index, value) in self.options.iter().enumerate() {
            push(&Row::MotionOption {
                component,
                index,
                value,
            })?;
        }
        Ok(())
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::GeometrySource;
use p4kx_extraction::{
    PhysicalSourceRef, SourceScope, SourceUse, SourceUseRole, SourceUses, SourceValue,
    SourcedValue, StructValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_classes::{at, string};

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ControllerCandidate {
    record_index: SourcedValue<u32>,
    record_guid: SourcedValue<String>,
    geometry_component: SourcedValue<usize>,
    sub_geometry: SourcedValue<Box<[usize]>>,
    geometry_path: SourcedValue<String>,
    geometry_source: SourcedValue<PhysicalSourceRef>,
    controller_component: SourcedValue<usize>,
    controller_path: SourcedValue<String>,
    database_path: SourcedValue<String>,
}

pub(super) struct CandidateInput<'a, 'tree, 'source> {
    pub(super) scope: &'a SourceScope,
    pub(super) sources: &'a SourceUses,
    pub(super) record: u32,
    pub(super) guid: String,
    pub(super) geometry: &'a GeometrySource<'tree, 'source>,
    pub(super) geometry_path: &'a str,
    pub(super) directory: SourcedValue<PhysicalSourceRef>,
    pub(super) ordinal: usize,
    pub(super) controller: &'a StructValue<'source>,
}

impl ControllerCandidate {
    pub(super) fn read(
        input: CandidateInput<'_, '_, '_>,
    ) -> Result<(Self, Vec<SourceUse>), SocSceneError> {
        let CandidateInput {
            scope,
            sources,
            record,
            guid,
            geometry,
            geometry_path,
            directory,
            ordinal,
            controller,
        } = input;
        let geometry_use = scope.at(geometry.model_location(), SourceUseRole::Payload)?;
        let geometry_uses = std::slice::from_ref(&geometry_use);
        let record_use = |field: &str, role| at(scope, record, field.to_owned(), role);
        let component_use = |field: &str, role| {
            at(
                scope,
                record,
                format!("Components[{ordinal}].{field}"),
                role,
            )
        };
        let controller_use = component_use("$type", SourceUseRole::Selector)?;
        let record_use = record_use("$guid", SourceUseRole::Payload)?;
        let result = Self {
            record_index: sources.value(
                SourceValue::Present(record),
                std::slice::from_ref(&record_use),
                true,
            ),
            record_guid: sources.value(
                SourceValue::Present(guid),
                std::slice::from_ref(&record_use),
                true,
            ),
            geometry_component: sources.value(
                SourceValue::Present(geometry.component()),
                geometry_uses,
                true,
            ),
            sub_geometry: sources.value(
                SourceValue::Present(geometry.sub_geometry().into()),
                geometry_uses,
                true,
            ),
            geometry_path: sources.value(
                SourceValue::Present(geometry_path.to_owned()),
                geometry_uses,
                true,
            ),
            geometry_source: directory,
            controller_component: sources.value(
                SourceValue::Present(ordinal),
                std::slice::from_ref(&controller_use),
                true,
            ),
            controller_path: member(sources, controller, "AnimationController", component_use)?,
            database_path: member(sources, controller, "AnimationDatabase", component_use)?,
        };
        Ok((result, vec![record_use, geometry_use, controller_use]))
    }
}

fn member(
    sources: &SourceUses,
    controller: &StructValue<'_>,
    field: &str,
    source_use: impl FnOnce(&str, SourceUseRole) -> Result<SourceUse, SocSceneError>,
) -> Result<SourcedValue<String>, SocSceneError> {
    let value = string(controller.field(field));
    let complete = !matches!(value, SourceValue::Unreadable);
    let used = source_use(field, SourceUseRole::Payload)?;
    Ok(sources.value(value, &[used], complete))
}

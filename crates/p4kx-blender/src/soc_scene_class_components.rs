// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    SourceScope, SourceUse, SourceUseRole, SourceValue, SourcedValue, StructValue, Value,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_class_loadout::DefaultLoadout;
use crate::soc_scene_class_mannequin::{self, Mannequin};
use crate::soc_scene_class_resources::{Resource, ResourceKind, Resources};
use crate::soc_scene_class_sensor::Sensor;
use crate::soc_scene_classes::{at, string};

pub(crate) struct ComponentInput<'a, 'source, 'archive> {
    pub(crate) record: u32,
    pub(crate) ordinal: usize,
    pub(crate) component: &'a StructValue<'source>,
    pub(crate) scope: &'a SourceScope,
    pub(crate) resources: &'a Resources<'archive>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(crate) enum ClassComponent {
    Door {
        ordinal: usize,
        fragment: SourcedValue<String>,
        animation_parameters_type: SourcedValue<String>,
    },
    Sensor {
        ordinal: usize,
        sensor: Box<Sensor>,
    },
    AnimationController {
        ordinal: usize,
        controller_path: SourcedValue<String>,
        database_path: SourcedValue<String>,
        default_animation_type: SourcedValue<String>,
        controller: Box<SourcedValue<Resource<Mannequin>>>,
        database: Box<SourcedValue<Resource<Mannequin>>>,
    },
    DefaultLoadout {
        ordinal: usize,
        loadout: Box<DefaultLoadout>,
    },
}

impl ClassComponent {
    pub(crate) fn read(
        input: &ComponentInput<'_, '_, '_>,
        all: &[Value<'_>],
    ) -> Result<Option<Self>, SocSceneError> {
        let ordinal = input.ordinal;
        let component = input.component;
        let prefix = format!("Components[{ordinal}]");
        let result = match component.type_name() {
            "SCItemDoorParams" => Self::Door {
                ordinal,
                fragment: input.cell(&prefix, "FragName", string(component.field("FragName")))?,
                animation_parameters_type: input.cell(
                    &prefix,
                    "AnimationParams",
                    struct_type(component.field("AnimationParams")),
                )?,
            },
            "SCItemProximitySensorParams" => Self::Sensor {
                ordinal,
                sensor: Box::new(Sensor::read(input, &prefix)?),
            },
            "SAnimationControllerParams" => Self::animation(input, &prefix, all)?,
            "SEntityComponentDefaultLoadoutParams" => Self::DefaultLoadout {
                ordinal,
                loadout: Box::new(DefaultLoadout::read(input, &prefix)?),
            },
            _ => return Ok(None),
        };
        Ok(Some(result))
    }

    fn animation(
        input: &ComponentInput<'_, '_, '_>,
        prefix: &str,
        all: &[Value<'_>],
    ) -> Result<Self, SocSceneError> {
        let controller = string(input.component.field("AnimationController"));
        let database = string(input.component.field("AnimationDatabase"));
        let selectors = door_fragments(input, all)?;
        let read = |field: &str| input.use_at(prefix, field, SourceUseRole::Selector);
        Ok(Self::AnimationController {
            ordinal: input.ordinal,
            controller_path: input.cell(prefix, "AnimationController", controller.clone())?,
            database_path: input.cell(prefix, "AnimationDatabase", database.clone())?,
            default_animation_type: input.cell(
                prefix,
                "defaultAnimation",
                struct_type(input.component.field("defaultAnimation")),
            )?,
            controller: Box::new(input.resources.read(
                controller,
                &read("AnimationController")?,
                ResourceKind::Mannequin,
                |xml, origin| soc_scene_class_mannequin::controller(xml, origin, &selectors),
            )?),
            database: Box::new(input.resources.read(
                database,
                &read("AnimationDatabase")?,
                ResourceKind::Mannequin,
                |xml, origin| soc_scene_class_mannequin::database(xml, origin, &selectors),
            )?),
        })
    }
}

fn door_fragments(
    input: &ComponentInput<'_, '_, '_>,
    values: &[Value<'_>],
) -> Result<Vec<(String, SourceUse)>, SocSceneError> {
    values
        .iter()
        .enumerate()
        .filter_map(|(index, value)| {
            let Value::Struct(component) = value else {
                return None;
            };
            if component.type_name() != "SCItemDoorParams" {
                return None;
            }
            let Some(Value::String(name)) = component.field("FragName") else {
                return None;
            };
            if name.is_empty() {
                return None;
            }
            Some(
                input
                    .use_at(
                        &format!("Components[{index}]"),
                        "FragName",
                        SourceUseRole::Selector,
                    )
                    .map(|used| ((*name).to_owned(), used)),
            )
        })
        .collect()
}

impl ComponentInput<'_, '_, '_> {
    pub(crate) fn use_at(
        &self,
        prefix: &str,
        name: &str,
        role: SourceUseRole,
    ) -> Result<SourceUse, SocSceneError> {
        at(self.scope, self.record, format!("{prefix}.{name}"), role)
    }

    pub(crate) fn cell<T>(
        &self,
        prefix: &str,
        name: &str,
        value: SourceValue<T>,
    ) -> Result<SourcedValue<T>, SocSceneError> {
        let complete = !matches!(value, SourceValue::Unreadable);
        let used = self.use_at(prefix, name, SourceUseRole::Payload)?;
        Ok(self.resources.sources.value(value, &[used], complete))
    }
}

pub(crate) fn struct_type(value: Option<&Value<'_>>) -> SourceValue<String> {
    match value {
        Some(Value::Struct(value)) => SourceValue::Present(value.type_name().to_owned()),
        None => SourceValue::Absent,
        Some(Value::Null) => SourceValue::Null,
        Some(Value::Error(_)) => SourceValue::Unreadable,
        _ => SourceValue::Invalid,
    }
}

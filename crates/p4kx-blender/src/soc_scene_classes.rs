// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_assembly::{SocContainerCapture, SocSourceGuid};
use p4kx_extraction::{
    Archive, CancelToken, DataForge, DataForgeEntry, DataForgeOrigin, EntryOrigin, Guid,
    InputScope, Record, RecordTree, SourceInput, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses, SourceValue, SourcedValue, TreeBuilder, Value,
};
use p4kx_model::SocEntity;
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_class_candidates::{candidates, inline_candidate};
use crate::soc_scene_class_components::{ClassComponent, ComponentInput};
use crate::soc_scene_class_resources::Resources;
use crate::soc_scene_light_classes::{DefaultClass, LightClasses};
use crate::soc_scene_origin::XmlOrigin;

const ENTITY_CLASS: &str = "EntityClassDefinition";

#[derive(Default)]
pub(crate) struct Classes {
    by_guid: BTreeMap<Guid, Vec<ClassSite>>,
    rows: Vec<SocClass>,
    available: bool,
    guid_index: Option<SourceUse>,
    pub(crate) lights: LightClasses,
}

struct ClassSite {
    index: usize,
    original: SourceUse,
    sensor_type: Option<SourceUse>,
}

impl Classes {
    pub(crate) fn read(
        capture: &SocContainerCapture,
        archive: &Archive,
        sources: &SourceUses,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        let wanted = candidates(capture, cancel)?;
        if wanted.entities.is_empty() && wanted.lights.is_empty() {
            return Ok(Self {
                available: true,
                ..Self::default()
            });
        }
        let Ok(entry) = DataForgeEntry::read_until(archive, &|| cancel.is_cancelled()) else {
            check(cancel)?;
            return Ok(Self::default());
        };
        let (DataForgeOrigin::Archive(origin), Ok(forge)) = (entry.origin(), entry.open()) else {
            return Ok(Self::default());
        };
        let mut result = Self {
            available: true,
            ..Self::default()
        };
        let scope = forge_scope(sources, origin)?;
        result.guid_index = Some(scope.at(
            SourceLocation::Metadata {
                field: "recordGuidIndex".into(),
            },
            SourceUseRole::Payload,
        )?);
        let resources = Resources::new(archive, sources, cancel);
        result.add_wanted(&forge, wanted.entities, &scope, &resources)?;
        result.lights = LightClasses::read(wanted.lights, &forge, &scope, sources, cancel)?;
        Ok(result)
    }

    fn add_wanted(
        &mut self,
        forge: &DataForge<'_>,
        wanted: BTreeMap<Guid, bool>,
        scope: &SourceScope,
        resources: &Resources<'_>,
    ) -> Result<(), SocSceneError> {
        let mut builder = TreeBuilder::new();
        for (guid, inline) in wanted {
            check(resources.cancel)?;
            let records: Vec<_> = forge
                .records_by_guid(guid)
                .map(|record| ClassRead::read(record, &mut builder))
                .collect();
            if inline || records.iter().any(|record| record.sensor_ordinal.is_some()) {
                for record in &records {
                    self.add(record, scope, resources)?;
                }
            }
        }
        Ok(())
    }

    fn add(
        &mut self,
        selected: &ClassRead<'_>,
        scope: &SourceScope,
        resources: &Resources<'_>,
    ) -> Result<(), SocSceneError> {
        let record = selected.record;
        let used = at(
            scope,
            record.index(),
            "$guid".to_owned(),
            SourceUseRole::Payload,
        )?;
        let sensor_type = selected
            .sensor_ordinal
            .map(|ordinal| {
                at(
                    scope,
                    record.index(),
                    format!("Components[{ordinal}].$type"),
                    SourceUseRole::Payload,
                )
            })
            .transpose()?;
        self.by_guid
            .entry(record.guid())
            .or_default()
            .push(ClassSite {
                index: self.rows.len(),
                original: used.clone(),
                sensor_type,
            });
        let class = SocClass::read(selected, scope, resources, used)?;
        self.rows.push(class);
        Ok(())
    }

    pub(crate) fn target(
        &self,
        entity: SocEntity<'_>,
        origin: &XmlOrigin<'_>,
    ) -> Result<Option<SourcedValue<ClassTarget>>, SocSceneError> {
        let guid = SocSourceGuid::read(entity.class_guid_attributes());
        let selected =
            matches!(guid, SocSourceGuid::Present(value) if self.by_guid.contains_key(&value));
        if !selected && !inline_candidate(entity) {
            return Ok(None);
        }
        let mut uses = selector_uses(entity, origin)?;
        let value = match guid {
            SocSourceGuid::Absent => SourceValue::Absent,
            SocSourceGuid::Present(guid) => self.select(guid, &mut uses),
            _ => SourceValue::Invalid,
        };
        let complete = !matches!(value, SourceValue::Unreadable);
        Ok(Some(origin.sources.value(value, &uses, complete)))
    }

    pub(crate) fn sensor(
        &self,
        entity: SocEntity<'_>,
        origin: &XmlOrigin<'_>,
    ) -> Result<Option<SourcedValue<bool>>, SocSceneError> {
        let SocSourceGuid::Present(guid) = SocSourceGuid::read(entity.class_guid_attributes())
        else {
            return Ok(None);
        };
        let Some([site]) = self.by_guid.get(&guid).map(Vec::as_slice) else {
            return Ok(None);
        };
        let Some(sensor_type) = &site.sensor_type else {
            return Ok(None);
        };
        let mut uses = selector_uses(entity, origin)?;
        uses.extend([site.original.clone(), sensor_type.clone()]);
        Ok(Some(origin.sources.value(
            SourceValue::Present(true),
            &uses,
            true,
        )))
    }

    fn select(&self, guid: Guid, uses: &mut Vec<SourceUse>) -> SourceValue<ClassTarget> {
        if !self.available {
            return SourceValue::Unreadable;
        }
        match self.by_guid.get(&guid).map(Vec::as_slice) {
            None => {
                uses.extend(self.guid_index.iter().cloned());
                SourceValue::Present(ClassTarget::Unmatched)
            }
            Some([site]) => {
                uses.push(site.original.clone());
                SourceValue::Present(ClassTarget::Matched { index: site.index })
            }
            Some(sites) => {
                uses.extend(sites.iter().map(|site| site.original.clone()));
                SourceValue::Present(ClassTarget::Ambiguous {
                    indices: sites.iter().map(|site| site.index).collect(),
                })
            }
        }
    }

    pub(crate) fn finish(self) -> (Vec<SocClass>, Vec<DefaultClass>) {
        (self.rows, self.lights.finish())
    }
}

fn selector_uses(
    entity: SocEntity<'_>,
    origin: &XmlOrigin<'_>,
) -> Result<Vec<SourceUse>, SocSceneError> {
    let mut uses = vec![origin.node(entity.element(), SourceUseRole::Selector)?];
    for attribute in entity.class_guid_attributes() {
        uses.push(origin.attribute(&attribute, SourceUseRole::Selector)?.1);
    }
    Ok(uses)
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum ClassTarget {
    Unmatched,
    Matched { index: usize },
    Ambiguous { indices: Vec<usize> },
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SocClass {
    record_index: u32,
    guid: SourcedValue<String>,
    record_type: SourcedValue<String>,
    components: SourcedValue<Vec<ClassComponent>>,
}

struct ClassRead<'source> {
    record: Record<'source>,
    tree: Option<RecordTree<'source>>,
    sensor_ordinal: Option<usize>,
}

impl<'source> ClassRead<'source> {
    fn read(record: Record<'source>, builder: &mut TreeBuilder<'source>) -> Self {
        let tree = if matches!(record.type_name(), Ok(ENTITY_CLASS)) {
            builder.build(&record).ok()
        } else {
            None
        };
        let sensor_ordinal = match tree.as_ref().and_then(|tree| tree.field("Components")) {
            Some(Value::Array(components)) => components.iter().position(|value| {
                matches!(value, Value::Struct(value) if value.type_name() == "SCItemProximitySensorParams")
            }),
            _ => None,
        };
        Self {
            record,
            tree,
            sensor_ordinal,
        }
    }
}

impl SocClass {
    fn read(
        selected: &ClassRead<'_>,
        scope: &SourceScope,
        resources: &Resources<'_>,
        guid: SourceUse,
    ) -> Result<Self, SocSceneError> {
        let record = selected.record;
        let sources = resources.sources;
        let record_index = record.index();
        let used = at(
            scope,
            record_index,
            "Components".to_owned(),
            SourceUseRole::Payload,
        )?;
        let record_type = record.type_name();
        let type_use = at(
            scope,
            record_index,
            "$type".to_owned(),
            SourceUseRole::Payload,
        )?;
        let typed = record_type
            .as_ref()
            .map_or(SourceValue::Unreadable, |name| {
                SourceValue::Present((*name).to_owned())
            });
        let type_complete = record_type.is_ok();
        let value = match record_type {
            Ok(ENTITY_CLASS) => match &selected.tree {
                Some(tree) => match tree.field("Components") {
                    Some(Value::Array(components)) => SourceValue::Present(class_components(
                        components,
                        record_index,
                        scope,
                        resources,
                    )?),
                    None => SourceValue::Absent,
                    Some(Value::Null) => SourceValue::Null,
                    Some(Value::Error(_)) => SourceValue::Unreadable,
                    _ => SourceValue::Invalid,
                },
                None => SourceValue::Unreadable,
            },
            Ok(_) => SourceValue::Unsupported,
            Err(_) => SourceValue::Unreadable,
        };
        let complete = !matches!(value, SourceValue::Unreadable);
        Ok(Self {
            record_index,
            guid: sources.value(
                SourceValue::Present(record.guid().to_string()),
                &[guid],
                true,
            ),
            record_type: sources.value(typed, &[type_use], type_complete),
            components: sources.value(value, &[used], complete),
        })
    }
}

fn class_components(
    components: &[Value<'_>],
    record: u32,
    scope: &SourceScope,
    resources: &Resources<'_>,
) -> Result<Vec<ClassComponent>, SocSceneError> {
    let mut rows = Vec::new();
    for (ordinal, component) in components.iter().enumerate() {
        check(resources.cancel)?;
        let Value::Struct(component) = component else {
            continue;
        };
        let input = ComponentInput {
            record,
            ordinal,
            component,
            scope,
            resources,
        };
        if let Some(value) = ClassComponent::read(&input, components)? {
            rows.push(value);
        }
    }
    Ok(rows)
}

pub(crate) fn string(value: Option<&Value<'_>>) -> SourceValue<String> {
    match value {
        Some(Value::String(text)) => SourceValue::Present((*text).to_owned()),
        None => SourceValue::Absent,
        Some(Value::Null) => SourceValue::Null,
        Some(Value::Error(_)) => SourceValue::Unreadable,
        _ => SourceValue::Invalid,
    }
}

pub(crate) fn at(
    scope: &SourceScope,
    record_index: u32,
    field: String,
    role: SourceUseRole,
) -> Result<SourceUse, SocSceneError> {
    Ok(scope.at(
        SourceLocation::DataForge {
            record_index,
            field,
        },
        role,
    )?)
}

fn forge_scope(sources: &SourceUses, origin: &EntryOrigin) -> Result<SourceScope, SocSceneError> {
    Ok(sources.observe(SourceInput::Archive(origin), InputScope::Payload)?)
}

pub(crate) fn check(cancel: &CancelToken) -> Result<(), SocSceneError> {
    if cancel.is_cancelled() {
        Err(SocSceneError::Cancelled)
    } else {
        Ok(())
    }
}

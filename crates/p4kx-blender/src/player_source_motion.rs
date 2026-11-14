// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::SourceDocument;
use p4kx_extraction::{
    CancelToken, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceValue, SourcedValue,
    StructValue, Value,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::player_source::{PlayerSourceError, check};
use crate::player_source_classes::{self, ClassTarget, Classes};
use crate::soc_scene::SocSceneError;
use crate::soc_scene_class_resources::{Resource, ResourceCapture, ResourceKind, Resources};
use crate::soc_scene_origin::XmlOrigin;
use crate::soc_scene_xml::XmlDocuments;

#[path = "player_motion_options.rs"]
mod motion_options;

#[path = "player_subadb_graph.rs"]
mod subadb_graph;

use subadb_graph::{DatabaseGraph, DatabaseRoot};

use motion_options::controller::Controller;
pub(crate) use motion_options::{BranchOption, MotionOptionFields, OptionRow};
use motion_options::{Database, MotionOptions};
pub(crate) use subadb_graph::{BranchFields, DatabaseLink};

use crate::player_bundle_wire::Row;
use std::io;

const PLAYER_CLASS: &str = "Player";
const CHARACTER_COMPONENT: &str = "SAnimatedCharacterComponentParams";

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Motion {
    pub(crate) actor: SourcedValue<ClassTarget>,
    pub(crate) components: SourcedValue<Vec<MotionComponent>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MotionComponent {
    ordinal: usize,
    controller_path: SourcedValue<String>,
    third_person_database_path: SourcedValue<String>,
    controller: SourcedValue<Resource<usize>>,
    third_person_database: SourcedValue<Resource<usize>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    authored_options: Option<MotionOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    database_graph: Option<DatabaseGraph>,
}

impl Motion {
    pub(crate) fn read(
        resources: &Resources<'_>,
        directory: &SourceUse,
        classes: Classes<'_, '_>,
        documents: &mut XmlDocuments<'_>,
    ) -> Result<Self, PlayerSourceError> {
        let actor =
            player_source_classes::by_name(resources.sources, classes, directory, PLAYER_CLASS)?;
        let Some((entities, scope)) = classes else {
            let components = resources.sources.value(
                SourceValue::Unreadable,
                std::slice::from_ref(directory),
                false,
            );
            return Ok(Self { actor, components });
        };
        let records = entities.named_class_candidates(PLAYER_CLASS);
        let [record] = records else {
            let candidates = scope.at(
                SourceLocation::Metadata {
                    field: "entityClassIndex".into(),
                },
                SourceUseRole::Payload,
            )?;
            let state = if records.is_empty() {
                SourceValue::Absent
            } else {
                SourceValue::Unsupported
            };
            let components = resources.sources.value(
                state,
                &[directory.clone(), candidates],
                records.is_empty(),
            );
            return Ok(Self { actor, components });
        };
        let location = |field: &str| SourceLocation::DataForge {
            record_index: record.index(),
            field: field.to_owned(),
        };
        let uses = [
            directory.clone(),
            scope.at(location("$name"), SourceUseRole::Selector)?,
            scope.at(location("Components"), SourceUseRole::Payload)?,
        ];
        let value = match record.tree() {
            Ok(tree) => match tree.field("Components") {
                Some(Value::Array(values)) => SourceValue::Present(components(
                    values,
                    record.index(),
                    scope,
                    resources,
                    documents,
                )?),
                Some(Value::Null) => SourceValue::Null,
                Some(Value::Error(_)) => SourceValue::Unreadable,
                None => SourceValue::Absent,
                _ => SourceValue::Invalid,
            },
            Err(_) => SourceValue::Unreadable,
        };
        let complete = !matches!(value, SourceValue::Unreadable);
        let components = resources.sources.value(value, &uses, complete);
        Ok(Self { actor, components })
    }
}

fn components(
    values: &[Value<'_>],
    record: u32,
    scope: &SourceScope,
    resources: &Resources<'_>,
    documents: &mut XmlDocuments<'_>,
) -> Result<Vec<MotionComponent>, PlayerSourceError> {
    let mut rows = Vec::new();
    for (ordinal, value) in values.iter().enumerate() {
        check(resources.cancel)?;
        let Value::Struct(component) = value else {
            continue;
        };
        if component.type_name() != CHARACTER_COMPONENT {
            continue;
        }
        let input = MotionInput {
            record,
            ordinal,
            component,
            scope,
            resources,
        };
        rows.push(input.project(documents)?);
    }
    Ok(rows)
}

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
        if let Some(options) = &self.authored_options {
            options.bundle_rows(index, push)?;
        }
        if let Some(graph) = &self.database_graph {
            graph.bundle_rows(index, push)?;
        }
        Ok(())
    }
}

struct MotionInput<'value, 'source, 'archive> {
    record: u32,
    ordinal: usize,
    component: &'value StructValue<'source>,
    scope: &'value SourceScope,
    resources: &'value Resources<'archive>,
}

struct Observed<T> {
    selector: SourceUse,
    data: T,
}

struct SelectedResource<T> {
    path: SourcedValue<String>,
    value: SourcedValue<Resource<usize>>,
    capture: ResourceCapture<usize>,
    observed: Option<Observed<T>>,
}

impl MotionInput<'_, '_, '_> {
    fn project(
        &self,
        documents: &mut XmlDocuments<'_>,
    ) -> Result<MotionComponent, PlayerSourceError> {
        let controller = self.resource(
            "actionController",
            "ControllerDef",
            documents,
            |xml, origin, cancel, _, _| Controller::read(xml, origin, cancel),
        )?;
        let mut database = self.resource(
            "animDatabase3P",
            "AnimDB",
            documents,
            |xml, origin, cancel, documents, index| {
                let draft = documents.adb_declaration(origin);
                let mut database = Database::read(xml, origin, cancel, draft)?;
                database.retain_declarations(documents, index);
                Ok(database)
            },
        )?;
        let database_graph = match (
            &mut database.observed,
            database.value.value(),
            database.path.value(),
        ) {
            (Some(observed), Some(Resource::Present { data, .. }), Some(path)) => {
                Some(DatabaseGraph::read(
                    self.resources,
                    DatabaseRoot {
                        database: &mut observed.data,
                        document: *data,
                        authored_path: path,
                        selector: &observed.selector,
                        capture: database.capture,
                    },
                    documents,
                )?)
            }
            _ => None,
        };
        let authored_options = controller
            .observed
            .zip(database.observed)
            .map(|(controller, database)| {
                MotionOptions::read(
                    self.resources,
                    controller.data,
                    database.data,
                    [&controller.selector, &database.selector],
                    documents,
                )
            })
            .transpose()?;
        Ok(MotionComponent {
            ordinal: self.ordinal,
            controller_path: controller.path,
            third_person_database_path: database.path,
            controller: controller.value,
            third_person_database: database.value,
            authored_options,
            database_graph,
        })
    }

    fn resource<T>(
        &self,
        name: &str,
        expected_root: &'static str,
        documents: &mut XmlDocuments<'_>,
        read: impl FnOnce(
            &SourceDocument<'_>,
            &XmlOrigin<'_>,
            &CancelToken,
            &mut XmlDocuments<'_>,
            usize,
        ) -> Result<T, SocSceneError>,
    ) -> Result<SelectedResource<T>, PlayerSourceError> {
        let value = crate::soc_scene_classes::string(self.component.field(name));
        let source = |role| {
            self.scope.at(
                SourceLocation::DataForge {
                    record_index: self.record,
                    field: format!("Components[{}].{name}", self.ordinal),
                },
                role,
            )
        };
        let selector = source(SourceUseRole::Selector)?;
        let payload = source(SourceUseRole::Payload)?;
        let complete = !matches!(value, SourceValue::Unreadable);
        let path = self
            .resources
            .sources
            .value(value.clone(), &[payload], complete);
        let mut observed = None;
        let capture = self.resources.capture_with_documents(
            value,
            ResourceKind::DataRelative,
            documents,
            |xml, origin, documents| {
                let root = xml.root();
                if root.namespace().is_some() || root.name() != expected_root {
                    return Ok(None);
                }
                let index = documents.bind(xml, origin)?;
                if let Some(seeds) = documents.tag_definitions() {
                    seeds.owner(index, root, origin)?;
                }
                observed = Some(Observed {
                    selector: selector.clone(),
                    data: read(xml, origin, self.resources.cancel, documents, index)?,
                });
                Ok(Some(index))
            },
        )?;
        let value = capture.bind(self.resources.sources, std::slice::from_ref(&selector));
        Ok(SelectedResource {
            path,
            value,
            capture,
            observed,
        })
    }
}

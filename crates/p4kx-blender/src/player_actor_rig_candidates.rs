// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{BodyVariant, GeometrySource, visit_geometry_sources};
use p4kx_extraction::{
    CancelToken, SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses, SourceValue,
    SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::player_source::{PlayerSourceError, check};
use crate::player_source_classes::Classes;
use crate::soc_scene_class_resources::{ResourceKind, resource_path};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RigCandidate {
    record_index: u32,
    component: usize,
    sub_geometry: Vec<usize>,
    model_path: SourcedValue<String>,
    tags: SourcedValue<String>,
    body_evidence: BodyEvidence,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct BodyEvidence {
    authored_tags: SourcedValue<Vec<&'static str>>,
    standard_skeleton_path: SourcedValue<Vec<&'static str>>,
}

pub(crate) fn read(
    classes: Classes<'_, '_>,
    directory: &SourceUse,
    sources: &SourceUses,
    cancel: &CancelToken,
) -> Result<SourcedValue<Vec<RigCandidate>>, PlayerSourceError> {
    let Some((entities, scope)) = classes else {
        return Ok(sources.value(
            SourceValue::Unreadable,
            std::slice::from_ref(directory),
            false,
        ));
    };
    let records = entities.named_class_candidates(super::PLAYER_CLASS);
    let [actor] = records else {
        let index = scope.at(
            SourceLocation::Metadata {
                field: "entityClassIndex".into(),
            },
            SourceUseRole::Payload,
        )?;
        let (state, complete) = if records.is_empty() {
            (SourceValue::Absent, true)
        } else {
            (SourceValue::Unsupported, false)
        };
        return Ok(sources.value(state, &[directory.clone(), index], complete));
    };
    let location = |field: &str| SourceLocation::DataForge {
        record_index: actor.index(),
        field: field.into(),
    };
    let uses = [
        directory.clone(),
        scope.at(location("$name"), SourceUseRole::Selector)?,
        scope.at(location("Components"), SourceUseRole::Payload)?,
    ];
    let Ok(tree) = actor.tree() else {
        return Ok(sources.value(SourceValue::Unreadable, &uses, false));
    };
    let mut candidates = Vec::new();
    visit_geometry_sources(&tree, |node| {
        check(cancel)?;
        candidates.push(candidate(actor.index(), &node, sources, scope)?);
        Ok::<(), PlayerSourceError>(())
    })?;
    Ok(sources.value(SourceValue::Present(candidates), &uses, true))
}

fn candidate(
    record: u32,
    node: &GeometrySource<'_, '_>,
    sources: &SourceUses,
    scope: &SourceScope,
) -> Result<RigCandidate, PlayerSourceError> {
    let path = field(sources, scope, node.model_location(), node.model())?;
    let tags = field(sources, scope, node.tag_location(), node.tags())?;
    let body_evidence = BodyEvidence {
        authored_tags: tags.clone().map(|tags| {
            BodyVariant::ALL
                .into_iter()
                .filter(|body| {
                    tags.split_whitespace()
                        .any(|tag| tag.eq_ignore_ascii_case(body.tag()))
                })
                .map(BodyVariant::code)
                .collect()
        }),
        standard_skeleton_path: path.clone().map(|path| {
            let normalized = resource_path(&path, ResourceKind::DataRelative);
            BodyVariant::ALL
                .into_iter()
                .filter(|body| {
                    let standard = resource_path(body.skeleton(), ResourceKind::DataRelative);
                    normalized.as_deref().zip(standard.as_deref()).is_some_and(
                        |(candidate, standard)| candidate.eq_ignore_ascii_case(standard),
                    )
                })
                .map(BodyVariant::code)
                .collect()
        }),
    };
    Ok(RigCandidate {
        record_index: record,
        component: node.component(),
        sub_geometry: node.sub_geometry().to_vec(),
        model_path: path,
        tags,
        body_evidence,
    })
}

fn field(
    sources: &SourceUses,
    scope: &SourceScope,
    position: SourceLocation,
    value: Option<&p4kx_extraction::Value<'_>>,
) -> Result<SourcedValue<String>, PlayerSourceError> {
    let field = crate::soc_scene_classes::string(value);
    let complete = !matches!(field, SourceValue::Unreadable);
    let used = scope.at(position, SourceUseRole::Payload)?;
    Ok(sources.value(field, &[used], complete))
}

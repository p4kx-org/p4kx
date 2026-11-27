// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{SourceDocument, SourceElement, SourceReadOptions};
use p4kx_extraction::{Entry, SourceUseRole, SourceValue, SourcedValue, archive_animation_path};
use schemars::JsonSchema;
use serde::Serialize;

use crate::player_source::{PlayerSourceError, check};
use crate::soc_scene_origin::XmlOrigin;
use crate::soc_scene_xml::XmlDocuments;

use super::input::Inputs;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Rule {
    pub(super) list: usize,
    pub(super) child: usize,
    name: Vec<SourcedValue<String>>,
    path: Vec<SourcedValue<String>>,
    pub(super) kind: SourcedValue<Kind>,
}

#[derive(Clone, Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(super) enum Kind {
    Folder,
    TracksDatabase {
        canonical: String,
    },
    RecursiveCaf {
        folder_rule: usize,
        canonical: String,
    },
    Unsupported,
    Invalid,
}

pub(super) struct ParsedRules {
    pub(super) state: SourcedValue<usize>,
    pub(super) rules: Vec<Rule>,
}

pub(super) fn read(
    inputs: &Inputs<'_, '_>,
    entry: &Entry,
    documents: &mut XmlDocuments<'_>,
) -> Result<ParsedRules, PlayerSourceError> {
    let member = inputs.member(entry)?;
    let origin = XmlOrigin::with_archive_member(inputs.sources, &member)?;
    let Ok(bytes) = member.payload() else {
        return Ok(ParsedRules {
            state: inputs.unreadable(&origin.scope, "chrParamsPayload")?,
            rules: Vec::new(),
        });
    };
    let Ok(document) = SourceDocument::parse(bytes, SourceReadOptions::default()) else {
        let payload = origin.scope.at(
            p4kx_extraction::SourceLocation::Metadata {
                field: "chrParamsPayload".into(),
            },
            SourceUseRole::Payload,
        )?;
        documents.reject_malformed(&origin, &payload)?;
        return Ok(ParsedRules {
            state: inputs.bytes(
                &origin.scope,
                0..bytes.len() as u64,
                "chrParamsPayload",
                SourceValue::Invalid,
                false,
            )?,
            rules: Vec::new(),
        });
    };
    let root = document.root();
    let valid = root.namespace().is_none() && root.name() == "Params";
    let root_use = origin.node(root, SourceUseRole::Payload)?;
    let state = if valid {
        SourceValue::Present(documents.bind(&document, &origin)?)
    } else {
        documents.reject_root(&document, &origin, &root_use)?;
        SourceValue::Invalid
    };
    let state = inputs
        .sources
        .value(state, &[root_use], valid && !document.is_lossy());
    let mut rules = Vec::new();
    if valid {
        for (ordinal, list) in root.elements().enumerate() {
            check(inputs.cancel)?;
            if list.namespace().is_none() && list.name() == "AnimationList" {
                read_list(inputs, &origin, ordinal, list, &mut rules)?;
            }
        }
    }
    Ok(ParsedRules { state, rules })
}

fn read_list(
    inputs: &Inputs<'_, '_>,
    origin: &XmlOrigin<'_>,
    list: usize,
    node: SourceElement<'_>,
    rules: &mut Vec<Rule>,
) -> Result<(), PlayerSourceError> {
    let mut folder = None;
    for (child, animation) in node.elements().enumerate() {
        check(inputs.cancel)?;
        if animation.namespace().is_some() || animation.name() != "Animation" {
            continue;
        }
        let name = attributes(animation, "name", origin)?;
        let path = attributes(animation, "path", origin)?;
        let used = origin.node(animation, SourceUseRole::Selector)?;
        let kind = classify(single(&name), single(&path), folder.as_ref());
        let mut origins = name
            .iter()
            .chain(&path)
            .map(SourcedValue::origin)
            .collect::<Vec<_>>();
        if matches!(kind, Kind::RecursiveCaf { .. })
            && let Some((index, _)) = &folder
            && let Some(previous) = rules.get(*index)
        {
            origins.push(previous.kind.origin());
        }
        let element = inputs
            .sources
            .value(SourceValue::Present(()), &[used], true);
        origins.push(element.origin());
        let kind = inputs
            .sources
            .value_with_origins(SourceValue::Present(kind), &origins)?;
        if name.iter().any(|name| {
            name.value()
                .is_some_and(|name| name.eq_ignore_ascii_case("#filepath"))
        }) {
            folder = single(&path)
                .zip(single(&name))
                .filter(|(_, name)| name.eq_ignore_ascii_case("#filepath"))
                .and_then(|(path, _)| archive_animation_path(path.trim_end_matches(['/', '\\'])))
                .map(|path| (rules.len(), path.to_ascii_lowercase()));
        }
        rules.push(Rule {
            list,
            child,
            name,
            path,
            kind,
        });
    }
    Ok(())
}

fn attributes(
    node: SourceElement<'_>,
    name: &str,
    origin: &XmlOrigin<'_>,
) -> Result<Vec<SourcedValue<String>>, PlayerSourceError> {
    node.attributes()
        .filter(|attribute| attribute.namespace().is_none() && attribute.name() == name)
        .map(|attribute| {
            let (_, used) = origin.attribute(&attribute, SourceUseRole::Selector)?;
            Ok(origin.sources.value(
                SourceValue::Present(attribute.value().to_owned()),
                &[used],
                true,
            ))
        })
        .collect()
}

fn single(values: &[SourcedValue<String>]) -> Option<&str> {
    let [value] = values else { return None };
    value.value().map(String::as_str)
}

fn classify(name: Option<&str>, path: Option<&str>, folder: Option<&(usize, String)>) -> Kind {
    let (Some(name), Some(path)) = (name, path) else {
        return Kind::Invalid;
    };
    if name.eq_ignore_ascii_case("#filepath") {
        return archive_animation_path(path.trim_end_matches(['/', '\\']))
            .map_or(Kind::Invalid, |_| Kind::Folder);
    }
    if name.eq_ignore_ascii_case("$TracksDatabase") {
        return archive_animation_path(path)
            .filter(|path| {
                path.rsplit_once('.')
                    .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("dba"))
            })
            .map_or(Kind::Invalid, |canonical| Kind::TracksDatabase {
                canonical: canonical.to_ascii_lowercase(),
            });
    }
    if name == "*"
        && (path.eq_ignore_ascii_case("*/*.caf") || path.eq_ignore_ascii_case("*\\*.caf"))
    {
        return folder.map_or(Kind::Invalid, |(index, canonical)| Kind::RecursiveCaf {
            folder_rule: *index,
            canonical: canonical.clone(),
        });
    }
    Kind::Unsupported
}

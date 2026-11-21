// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_extraction::{
    CancelToken, SourceUse, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};

use crate::soc_scene_classes::check;

#[path = "player_controller_topology_draft.rs"]
mod draft;
pub(crate) use draft::Draft;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ControllerTopology {
    document: usize,
    contexts: Vec<Node>,
    scopes: Vec<Scoped>,
    fragments: Vec<Fragment>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Node {
    position: SourcedValue<SocXmlPosition>,
    name: SourcedValue<String>,
    attributes: Vec<Attribute>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Attribute {
    position: SocXmlPosition,
    name: String,
    namespace: Option<String>,
    value: SourcedValue<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Scoped {
    declaration: Node,
    context: SourcedValue<Match>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Fragment {
    declaration: Node,
    scopes: Vec<SourcedValue<Match>>,
    overrides: Vec<ScopeOverride>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct ScopeOverride {
    declaration: Node,
    scopes: Vec<SourcedValue<Match>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Match {
    name: String,
    candidates: Vec<usize>,
    status: Status,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
enum Status {
    EmptySelector,
    Missing,
    Resolved,
    Ambiguous,
    Incomplete,
}

struct Lookup {
    nodes: Vec<SourceUse>,
    by_name: HashMap<String, Vec<usize>>,
    evidence: Vec<SourceUse>,
    complete: bool,
}

impl ControllerTopology {
    pub(crate) fn bind(
        document: usize,
        draft: Draft,
        sources: &SourceUses,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        let origin = XmlOrigin {
            scope: draft.scope,
            sources,
            chunk: None,
            archive_member: None,
        };
        let evidence = origin.at(&draft.root, "$element", SourceUseRole::Selector)?;
        let contexts = Lookup::new(&draft.contexts, &origin, &evidence, !draft.lossy, cancel)?;
        let scopes = Lookup::new(&draft.scopes, &origin, &evidence, !draft.lossy, cancel)?;
        let bound = draft
            .scopes
            .members
            .into_iter()
            .zip(&scopes.nodes)
            .map(|(node, observed)| {
                check(cancel)?;
                let context = single_selector(&node, &origin, &contexts, observed)?;
                Ok(Scoped {
                    declaration: Node::bind(node, observed, &origin, cancel)?,
                    context,
                })
            })
            .collect::<Result<_, SocSceneError>>()?;
        let fragments = fragments(draft.fragments, &origin, &scopes, cancel)?;
        let contexts = draft
            .contexts
            .members
            .into_iter()
            .zip(&contexts.nodes)
            .map(|(node, observed)| Node::bind(node, observed, &origin, cancel))
            .collect::<Result<_, _>>()?;
        Ok(Self {
            document,
            contexts,
            scopes: bound,
            fragments,
        })
    }
}

impl Node {
    fn bind(
        draft: draft::Node,
        used: &SourceUse,
        origin: &XmlOrigin<'_>,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        check(cancel)?;
        let source = std::slice::from_ref(used);
        let name = origin
            .sources
            .value(SourceValue::Present(draft.name), source, true);
        let position = origin
            .sources
            .value(SourceValue::Present(draft.position), source, true);
        let attributes = draft
            .attributes
            .into_iter()
            .map(|attr| {
                check(cancel)?;
                let source = origin.at(&attr.position, &attr.name, SourceUseRole::Payload)?;
                Ok(Attribute {
                    position: attr.position,
                    name: attr.name,
                    namespace: attr.namespace,
                    value: origin
                        .sources
                        .value(SourceValue::Present(attr.value), &[source], true),
                })
            })
            .collect::<Result<_, SocSceneError>>()?;
        Ok(Self {
            position,
            name,
            attributes,
        })
    }
}

impl Lookup {
    fn new(
        section: &draft::Section<draft::Node>,
        origin: &XmlOrigin<'_>,
        root: &SourceUse,
        complete: bool,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        let mut evidence = vec![root.clone()];
        for position in &section.positions {
            check(cancel)?;
            evidence.push(origin.at(position, "$element", SourceUseRole::Selector)?);
        }
        let mut by_name = HashMap::<_, Vec<usize>>::new();
        let mut nodes = Vec::new();
        for (index, node) in section.members.iter().enumerate() {
            check(cancel)?;
            by_name.entry(node.name.clone()).or_default().push(index);
            nodes.push(origin.at(&node.position, "$element", SourceUseRole::Selector)?);
        }
        Ok(Self {
            nodes,
            by_name,
            evidence,
            complete,
        })
    }

    fn select(
        &self,
        name: &str,
        origin: &XmlOrigin<'_>,
        selectors: &[SourceUse],
    ) -> SourcedValue<Match> {
        let candidates = self.by_name.get(name).map_or(&[][..], Vec::as_slice);
        let status = match candidates {
            _ if name.is_empty() => Status::EmptySelector,
            [_, _, ..] => Status::Ambiguous,
            _ if !self.complete => Status::Incomplete,
            [_] => Status::Resolved,
            [] => Status::Missing,
        };
        let mut evidence = selectors.to_vec();
        if !name.is_empty() {
            evidence.extend_from_slice(&self.evidence);
            evidence.extend(
                candidates
                    .iter()
                    .filter_map(|&index| self.nodes.get(index))
                    .cloned(),
            );
        }
        origin.sources.value(
            SourceValue::Present(Match {
                name: name.into(),
                candidates: candidates.to_vec(),
                status,
            }),
            &evidence,
            name.is_empty() || self.complete,
        )
    }
}

fn single_selector(
    node: &draft::Node,
    origin: &XmlOrigin<'_>,
    lookup: &Lookup,
    owner: &SourceUse,
) -> Result<SourcedValue<Match>, SocSceneError> {
    let mut matches = node.attributes("context");
    let mut uses = vec![owner.clone()];
    let first = matches.next();
    for attribute in first.into_iter().chain(matches) {
        uses.push(origin.at(
            &attribute.position,
            &attribute.name,
            SourceUseRole::Selector,
        )?);
    }
    Ok(match (first, uses.len()) {
        (None, _) => origin.sources.value(SourceValue::Absent, &uses, true),
        (Some(attribute), 2) => lookup.select(&attribute.value, origin, &uses),
        _ => origin.sources.value(SourceValue::Invalid, &uses, true),
    })
}

fn fragments(
    fragments: Vec<draft::Fragment>,
    origin: &XmlOrigin<'_>,
    lookup: &Lookup,
    cancel: &CancelToken,
) -> Result<Vec<Fragment>, SocSceneError> {
    fragments
        .into_iter()
        .map(|fragment| {
            let (declaration, scopes) =
                bind_with_scopes(fragment.declaration, origin, lookup, cancel)?;
            let overrides = fragment
                .overrides
                .into_iter()
                .map(|node| {
                    let (declaration, scopes) = bind_with_scopes(node, origin, lookup, cancel)?;
                    Ok(ScopeOverride {
                        declaration,
                        scopes,
                    })
                })
                .collect::<Result<_, SocSceneError>>()?;
            Ok(Fragment {
                declaration,
                scopes,
                overrides,
            })
        })
        .collect()
}

fn bind_with_scopes(
    node: draft::Node,
    origin: &XmlOrigin<'_>,
    lookup: &Lookup,
    cancel: &CancelToken,
) -> Result<(Node, Vec<SourcedValue<Match>>), SocSceneError> {
    check(cancel)?;
    let scopes = scope_selectors(&node, origin, lookup, cancel)?;
    let used = origin.at(&node.position, "$element", SourceUseRole::Selector)?;
    Ok((Node::bind(node, &used, origin, cancel)?, scopes))
}

fn scope_selectors(
    node: &draft::Node,
    origin: &XmlOrigin<'_>,
    lookup: &Lookup,
    cancel: &CancelToken,
) -> Result<Vec<SourcedValue<Match>>, SocSceneError> {
    let mut attributes = node.attributes("scopes");
    let Some(attribute) = attributes.next() else {
        return Ok(Vec::new());
    };
    if attributes.next().is_some() || attribute.value.is_empty() {
        return Ok(Vec::new());
    }
    let evidence = origin.at(
        &attribute.position,
        &attribute.name,
        SourceUseRole::Selector,
    )?;
    attribute
        .value
        .split('+')
        .map(|name| {
            check(cancel)?;
            Ok(lookup.select(name, origin, std::slice::from_ref(&evidence)))
        })
        .collect()
}

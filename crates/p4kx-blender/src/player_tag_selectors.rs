// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{CancelToken, FieldOrigin, SourceUses};
use schemars::JsonSchema;
use serde::Serialize;

use crate::player_adb_declarations::{AdbDeclarations, AttributeSelection, Fragment};
use crate::soc_scene::SocSceneError;

use super::scope::{Candidate, Link, Survey, TagScope};
use super::source::{self, Kind, Role};
use super::{Document, Edge, TagDefinitions};

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AdbTagUses {
    pub(crate) scopes: Vec<TagScope>,
    pub(crate) selectors: Vec<AdbTagSelectors>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AdbTagSelectors {
    document: usize,
    index: usize,
    global: Selector,
    fragment: Selector,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Selector {
    attribute: AttributeSelection,
    root_scope: Option<usize>,
    fragment_owner: Option<FragmentOwner>,
    tokens: Vec<Token>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct FragmentOwner {
    lookup: Lookup,
    tag_scope: Option<usize>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Token {
    spelling: String,
    #[serde(flatten)]
    lookup: Lookup,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Lookup {
    status: Status,
    contains_encrypted_source: Option<bool>,
    source_set_complete: bool,
    matches: Vec<Candidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
enum Status {
    Resolved,
    Missing,
    Ambiguous,
    Incomplete,
    EmptyToken,
    UnsupportedSpelling,
    NoFragmentDefinition,
    NoFragmentTagDefinition,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ScopeKey {
    Root(usize),
    Fragment(usize, usize),
}

struct Resolver<'a> {
    documents: BTreeMap<usize, &'a Document>,
    roots: BTreeMap<(usize, Kind), (usize, &'a Edge)>,
    scope_indexes: BTreeMap<ScopeKey, usize>,
    surveys: Vec<Survey>,
    sources: &'a SourceUses,
    cancel: &'a CancelToken,
}

impl AdbTagUses {
    pub(crate) fn read(
        declarations: &[AdbDeclarations],
        definitions: &TagDefinitions,
        sources: &SourceUses,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        let mut resolver = Resolver::new(definitions, sources, cancel);
        let mut selectors = Vec::new();
        for document in declarations {
            for (index, option) in document.fragments.iter().enumerate() {
                source::check(cancel)?;
                if option.tags.occurrences.is_empty() && option.local_tags.occurrences.is_empty() {
                    continue;
                }
                selectors.push(resolver.option(document.document, index, option)?);
            }
        }
        Ok(Self {
            scopes: resolver
                .surveys
                .into_iter()
                .map(|survey| survey.output)
                .collect(),
            selectors,
        })
    }
}

impl<'a> Resolver<'a> {
    fn new(
        definitions: &'a TagDefinitions,
        sources: &'a SourceUses,
        cancel: &'a CancelToken,
    ) -> Self {
        let documents = definitions
            .documents
            .iter()
            .map(|document| (document.index, document))
            .collect();
        let roots = definitions
            .roots
            .iter()
            .enumerate()
            .filter(|(_, edge)| matches!(edge.role, Role::DatabaseTags | Role::DatabaseFragments))
            .map(|(index, edge)| ((edge.owner, edge.target_kind), (index, edge)))
            .collect();
        Self {
            documents,
            roots,
            scope_indexes: BTreeMap::new(),
            surveys: Vec::new(),
            sources,
            cancel,
        }
    }

    fn option(
        &mut self,
        document: usize,
        index: usize,
        option: &Fragment,
    ) -> Result<AdbTagSelectors, SocSceneError> {
        let mut global = Selector::new(&option.tags);
        if let Some(value) = text(&option.tags) {
            let root = self.root(document, Kind::Tags)?;
            global.root_scope = Some(root);
            global.tokens = self.tokens(value, &option.tags.origin, Some(root), None)?;
        }
        let fragment = self.fragment(document, option)?;
        Ok(AdbTagSelectors {
            document,
            index,
            global,
            fragment,
        })
    }

    fn fragment(&mut self, document: usize, option: &Fragment) -> Result<Selector, SocSceneError> {
        let attribute = &option.local_tags;
        let mut selector = Selector::new(attribute);
        let Some(value) = text(attribute) else {
            return Ok(selector);
        };
        let root = self.root(document, Kind::Fragments)?;
        selector.root_scope = Some(root);
        let name = option.name.value().ok_or(SocSceneError::Position)?;
        let survey = self.surveys.get(root).ok_or(SocSceneError::Position)?;
        let lookup = lookup(name, option.name.origin(), Some(survey), None);
        let [candidate] = lookup.matches.as_slice() else {
            return self.fragment_unresolved(selector, lookup, value, &attribute.origin, root);
        };
        if lookup.status != Status::Resolved {
            return self.fragment_unresolved(selector, lookup, value, &attribute.origin, root);
        }
        let scope = self.sub_scope(candidate)?;
        selector.tokens = self.tokens(
            value,
            &attribute.origin,
            scope,
            Some((
                root,
                if scope.is_some() {
                    Status::Incomplete
                } else {
                    Status::NoFragmentTagDefinition
                },
            )),
        )?;
        selector.fragment_owner = Some(FragmentOwner {
            lookup,
            tag_scope: scope,
        });
        Ok(selector)
    }

    fn fragment_unresolved(
        &self,
        mut selector: Selector,
        lookup: Lookup,
        value: &str,
        own: &FieldOrigin,
        root: usize,
    ) -> Result<Selector, SocSceneError> {
        let status = if lookup.status == Status::Missing {
            Status::NoFragmentDefinition
        } else {
            lookup.status
        };
        selector.tokens = self.tokens(value, own, None, Some((root, status)))?;
        selector.fragment_owner = Some(FragmentOwner {
            lookup,
            tag_scope: None,
        });
        Ok(selector)
    }

    fn tokens(
        &self,
        value: &str,
        own: &FieldOrigin,
        scope: Option<usize>,
        context: Option<(usize, Status)>,
    ) -> Result<Vec<Token>, SocSceneError> {
        let survey = scope
            .map(|index| self.surveys.get(index).ok_or(SocSceneError::Position))
            .transpose()?;
        let context = context
            .map(|(index, status)| {
                self.surveys
                    .get(index)
                    .map(|survey| (&survey.output.origin, status))
                    .ok_or(SocSceneError::Position)
            })
            .transpose()?;
        value
            .split('+')
            .map(|spelling| {
                source::check(self.cancel)?;
                Ok(Token {
                    spelling: spelling.into(),
                    lookup: lookup(spelling, own, survey, context),
                })
            })
            .collect()
    }

    fn root(&mut self, document: usize, kind: Kind) -> Result<usize, SocSceneError> {
        let &(index, edge) = self
            .roots
            .get(&(document, kind))
            .ok_or(SocSceneError::Position)?;
        self.scope(ScopeKey::Root(index), kind, Link::Root { index }, edge)
    }

    fn sub_scope(&mut self, candidate: &Candidate) -> Result<Option<usize>, SocSceneError> {
        let document = self
            .documents
            .get(&candidate.document)
            .ok_or(SocSceneError::Position)?;
        let Some((index, edge)) = document
            .links
            .iter()
            .enumerate()
            .find(|(_, edge)| candidate.owns(edge))
        else {
            return Ok(None);
        };
        self.scope(
            ScopeKey::Fragment(candidate.document, index),
            Kind::Tags,
            Link::Definition {
                document: candidate.document,
                index,
            },
            edge,
        )
        .map(Some)
    }

    fn scope(
        &mut self,
        key: ScopeKey,
        kind: Kind,
        link: Link,
        edge: &Edge,
    ) -> Result<usize, SocSceneError> {
        if let Some(&index) = self.scope_indexes.get(&key) {
            return Ok(index);
        }
        let survey = Survey::read(
            kind,
            &self.documents,
            (link, edge),
            self.sources,
            self.cancel,
        )?;
        let index = self.surveys.len();
        self.scope_indexes.insert(key, index);
        self.surveys.push(survey);
        Ok(index)
    }
}

impl Selector {
    fn new(attribute: &AttributeSelection) -> Self {
        Self {
            attribute: attribute.clone(),
            root_scope: None,
            fragment_owner: None,
            tokens: Vec::new(),
        }
    }
}

fn text(attribute: &AttributeSelection) -> Option<&str> {
    let [occurrence] = attribute.occurrences.as_slice() else {
        return None;
    };
    occurrence
        .value
        .value()
        .map(String::as_str)
        .filter(|value| !value.is_empty())
}

fn lookup(
    spelling: &str,
    own: &FieldOrigin,
    survey: Option<&Survey>,
    context: Option<(&FieldOrigin, Status)>,
) -> Lookup {
    if spelling.is_empty() {
        return Lookup::new(Status::EmptyToken, [], [Some(own), None, None]);
    }
    if !spelling.is_ascii() {
        return Lookup::new(Status::UnsupportedSpelling, [], [Some(own), None, None]);
    }
    let Some(survey) = survey else {
        let (origin, status) = context.map_or((None, Status::Incomplete), |(origin, status)| {
            (Some(origin), status)
        });
        return Lookup::new(status, [], [Some(own), origin, None]);
    };
    let matches = survey.candidates(spelling);
    let status = match matches {
        [_, _, ..] => Status::Ambiguous,
        _ if !survey.output.origin.source_set_complete() => Status::Incomplete,
        [_] => Status::Resolved,
        [] => Status::Missing,
    };
    Lookup::new(
        status,
        matches.iter().cloned(),
        [Some(own), Some(&survey.output.origin), context.map(|c| c.0)],
    )
}

impl Lookup {
    fn new(
        status: Status,
        matches: impl IntoIterator<Item = Candidate>,
        origins: [Option<&FieldOrigin>; 3],
    ) -> Self {
        let mut complete = true;
        let mut encrypted = Some(false);
        for origin in origins.into_iter().flatten() {
            complete &= origin.source_set_complete();
            encrypted = match (encrypted, origin.contains_encrypted_source()) {
                (Some(true), _) | (_, Some(true)) => Some(true),
                (Some(false), Some(false)) => Some(false),
                _ => None,
            };
        }
        Self {
            status,
            contains_encrypted_source: encrypted,
            source_set_complete: complete,
            matches: matches.into_iter().collect(),
        }
    }
}

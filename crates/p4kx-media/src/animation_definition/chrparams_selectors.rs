// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::NamedCafSelector;
use p4kx_extraction::cryxml::{SourceAttribute, SourceDocument, SourceElement, SourceReadOptions};

use super::wildcard_match::{Rule, Scope};
use super::{links, payload::LinkKind};
use crate::MediaError;

pub(super) struct AnimationAttributes<'document> {
    pub(super) name: SourceAttribute<'document>,
    pub(super) path: SourceAttribute<'document>,
}

pub(super) struct Selector<'document> {
    pub(super) attributes: Option<AnimationAttributes<'document>>,
    pub(super) filepath: bool,
    pub(super) tracks_database: bool,
    pub(super) work: u64,
}

impl<'document> Selector<'document> {
    pub(super) fn read(element: SourceElement<'document>) -> Self {
        let mut name = None;
        let mut path = None;
        let mut duplicate = false;
        let mut filepath = false;
        let mut tracks_database = false;
        let mut work = 0;
        for attribute in element
            .attributes()
            .filter(|attribute| attribute.namespace().is_none())
        {
            work += 1;
            if attribute.name().eq_ignore_ascii_case("name") {
                filepath |= attribute.value().eq_ignore_ascii_case("#filepath");
                tracks_database |= attribute.value().eq_ignore_ascii_case("$TracksDatabase");
                duplicate |= name.replace(attribute).is_some();
            } else if attribute.name().eq_ignore_ascii_case("path") {
                duplicate |= path.replace(attribute).is_some();
            }
        }
        let attributes = match (name, path, duplicate) {
            (Some(name), Some(path), false) => Some(AnimationAttributes { name, path }),
            _ => None,
        };
        Self {
            attributes,
            filepath,
            tracks_database,
            work,
        }
    }
}

pub(super) fn named(element: SourceElement<'_>, name: &str) -> bool {
    element.namespace().is_none() && element.name().eq_ignore_ascii_case(name)
}

pub(super) enum Target<'source> {
    Definition(String),
    Reference(String),
    Wildcard(Rule<'source>),
}

pub(super) fn visit_targets(
    bytes: &[u8],
    charge: impl FnMut(u64) -> Result<(), MediaError>,
    visit: impl FnMut(Target<'_>) -> Result<(), MediaError>,
) -> Result<(), MediaError> {
    let document = SourceDocument::parse(bytes, SourceReadOptions::default()).map_err(|cause| {
        MediaError::Contract(format!("invalid scoped character parameters: {cause}"))
    })?;
    visit_document_targets(&document, charge, visit)
}

pub(super) fn visit_document_targets(
    document: &SourceDocument<'_>,
    mut charge: impl FnMut(u64) -> Result<(), MediaError>,
    mut visit: impl FnMut(Target<'_>) -> Result<(), MediaError>,
) -> Result<(), MediaError> {
    charge(0)?;
    if document.is_lossy() {
        return Ok(());
    }
    let root = document.root();
    if !named(root, "Params") {
        return Ok(());
    }
    for list in root.elements() {
        charge(1)?;
        if !named(list, "AnimationList") {
            continue;
        }
        let mut folder = Scope::Missing;
        for animation in list.elements() {
            charge(1)?;
            if !named(animation, "Animation") {
                continue;
            }
            let selector = Selector::read(animation);
            charge(selector.work)?;
            if selector.filepath {
                folder = preceding_folder(selector.attributes.as_ref());
            } else if let Some(attributes) = selector.attributes.as_ref()
                && let Some(target) = target(attributes, folder)
            {
                visit(target)?;
            }
        }
    }
    Ok(())
}

fn preceding_folder<'source>(attributes: Option<&AnimationAttributes<'source>>) -> Scope<'source> {
    match attributes.map(|attributes| attributes.path.value()) {
        None => Scope::Malformed,
        Some(path) if path.trim_end_matches(['/', '\\']).is_empty() => Scope::Reset,
        Some(path) => Scope::Authored(path),
    }
}

fn target<'source>(
    attributes: &AnimationAttributes<'source>,
    folder: Scope<'_>,
) -> Option<Target<'source>> {
    let name = attributes.name.value();
    let authored = attributes.path.value();
    if let Some(kind) = links::chrparams_link_kind(name) {
        let path = links::target_path(kind, authored)?;
        return Some(match kind {
            LinkKind::EventDatabase | LinkKind::CharacterParametersInclude => {
                Target::Definition(path)
            }
            _ => Target::Reference(path),
        });
    }
    if let Some(selector) = NamedCafSelector::new(name, authored) {
        let folder = match folder {
            Scope::Authored(path) => Some(path),
            _ => None,
        };
        return selector.archive_path(folder).map(Target::Reference);
    }
    Rule::parse(name, authored, folder)
        .ok()
        .map(Target::Wildcard)
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::NamedCafSelector;
use p4kx_extraction::cryxml::{SourceAttribute, SourceDocument, SourceElement, SourceReadOptions};

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

pub(super) enum Target {
    Definition(String),
    Reference(String),
}

pub(super) fn visit_targets(
    bytes: &[u8],
    mut charge: impl FnMut(u64) -> Result<(), MediaError>,
    mut visit: impl FnMut(Target) -> Result<(), MediaError>,
) -> Result<(), MediaError> {
    let document = SourceDocument::parse(bytes, SourceReadOptions::default()).map_err(|cause| {
        MediaError::Contract(format!("invalid scoped character parameters: {cause}"))
    })?;
    let root = document.root();
    if !named(root, "Params") {
        return Ok(());
    }
    for list in root.elements() {
        charge(1)?;
        if !named(list, "AnimationList") {
            continue;
        }
        let mut folder = None;
        for animation in list.elements() {
            charge(1)?;
            if !named(animation, "Animation") {
                continue;
            }
            let selector = Selector::read(animation);
            charge(selector.work)?;
            if selector.filepath {
                folder = selector.attributes.map(|attributes| attributes.path);
            } else if let Some(attributes) = selector.attributes.as_ref()
                && let Some(target) =
                    target(attributes, folder.as_ref().map(SourceAttribute::value))
            {
                visit(target)?;
            }
        }
    }
    Ok(())
}

fn target(attributes: &AnimationAttributes<'_>, folder: Option<&str>) -> Option<Target> {
    let name = attributes.name.value();
    let authored = attributes.path.value();
    if let Some(kind) = links::chrparams_database_kind(name) {
        let path = links::target_path(kind, authored)?;
        return Some(match kind {
            LinkKind::EventDatabase => Target::Definition(path),
            _ => Target::Reference(path),
        });
    }
    let archive_path = NamedCafSelector::new(name, authored)?.archive_path(folder)?;
    Some(Target::Reference(archive_path))
}

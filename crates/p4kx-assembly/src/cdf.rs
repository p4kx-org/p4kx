// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::SourceLocation;

use crate::CdfAttachmentSite;
use crate::cdf_attachment_source;
use crate::cdf_attribute_position::CdfCoordinateDomain;
use crate::cdf_bone::BoneAttachment;
use crate::cdf_model_sources::CdfModelSources;
use crate::loadout_xml::{Node, Scope};
use crate::markup::{self, Budget, Element};

pub(crate) use p4kx_extraction::model::character_definition::is_definition_path as is_character_definition;

/// The most bytes of a character definition that are read.
pub(crate) const MAX_CDF_BYTES: u64 = 1 << 20;

/// The selected main model, drawable bone attachments and omitted bindings.
#[derive(Debug, PartialEq)]
pub(crate) struct Character {
    pub(crate) model: String,
    pub(crate) material: Option<String>,
    pub(crate) bones: Vec<BoneAttachment>,
    pub(crate) left_out: Vec<String>,
    pub(crate) skeleton: Option<String>,
    pub(crate) model_sources: Option<CdfModelSources>,
    pub(crate) primary_rigid: Option<BoneAttachment>,
    pub(crate) rig_sources: Option<CdfModelSources>,
}

/// Reads the character definition in `bytes`.
///
/// # Errors
///
/// Returns why the document is no character definition or names nothing to draw.
pub(crate) fn read(bytes: &[u8]) -> Result<Character, String> {
    let mut budget = Budget::new(bytes.len());
    let root = markup::parse(bytes, &mut budget)?;
    Drawing::read(&root)?.project(Selection::Bound, |_| None, |_| None)
}

/// A failed definition parse or read of its declared body.
#[derive(Debug)]
pub(crate) enum DrawError<E> {
    Definition(String),
    Body(E),
}

pub(crate) enum DeclaredModel<T> {
    Drawable(T),
    Rig(T),
}

/// Projects the CDF and returns the declared model when it supplies the body or rigid rig.
pub(crate) fn read_drawing<T, E>(
    bytes: &[u8],
    observe_attachments: bool,
    read_body: impl FnOnce(&str) -> Result<Option<DeclaredModel<T>>, E>,
) -> Result<(Character, Option<T>), DrawError<E>> {
    let mut budget = Budget::new(bytes.len());
    let drawing = markup::parse_with_source(bytes, &mut budget, |root, original, scope| {
        let drawing = Drawing::read(&root)?;
        let body = match drawing.body_candidate().map_or(Ok(None), read_body) {
            Ok(body) => body,
            Err(error) => return Ok(Err(error)),
        };
        let (selection, body) = match body {
            Some(DeclaredModel::Drawable(body)) => (Selection::DeclaredBody, Some(body)),
            Some(DeclaredModel::Rig(rig)) => drawing.primary_rigid().map_or_else(
                || (Selection::Bound, None),
                |primary| (Selection::DeclaredRig(primary), Some(rig)),
            ),
            None => (Selection::Bound, None),
        };
        let character =
            project_observed(&drawing, selection, original, scope, observe_attachments)?;
        Ok(Ok((character, body)))
    })
    .map_err(DrawError::Definition)?;
    drawing.map_err(DrawError::Body)
}

fn project_observed(
    drawing: &Drawing<'_>,
    selection: Selection,
    original: Node<'_, '_>,
    scope: &Scope<'_, '_>,
    observe_attachments: bool,
) -> Result<Character, String> {
    let children = original.children();
    let attachments = children
        .iter()
        .copied()
        .find(|child| child.name().eq_ignore_ascii_case("AttachmentList"))
        .map(Node::children);
    let attachment = |ordinal| attachments.as_ref()?.get(ordinal).copied();
    let model = children
        .iter()
        .copied()
        .find(|child| child.name().eq_ignore_ascii_case("Model"));
    drawing.project(
        selection,
        |ordinal| {
            if !observe_attachments {
                return None;
            }
            Some(cdf_attachment_source::capture(attachment(ordinal)?, scope))
        },
        |selected| match selected {
            MainSelector::Attachment {
                ordinal,
                preferred_skin,
            } => CdfModelSources::attachment(attachment(ordinal)?, scope, preferred_skin),
            MainSelector::ModelFile => CdfModelSources::file(model?, scope),
        },
    )
}

pub(crate) fn read_skeleton_with_position(
    bytes: &[u8],
) -> Result<(Option<String>, Option<SourceLocation>), String> {
    let mut budget = Budget::new(bytes.len());
    markup::parse_with_source(bytes, &mut budget, |root, original, scope| {
        let character = Drawing::read(&root)?.project(Selection::Bound, |_| None, |_| None)?;
        let position = character.skeleton.as_ref().and_then(|_| {
            let node = original
                .children()
                .into_iter()
                .find(|child| child.name().eq_ignore_ascii_case("Model"))?;
            let position = scope.attribute_ascii_position(node, "File")?;
            CdfCoordinateDomain::Model.location(&position, "File")
        });
        Ok((character.skeleton, position))
    })
}

enum Selection {
    Bound,
    DeclaredBody,
    DeclaredRig(BoneAttachment),
}

enum MainSelector {
    Attachment {
        ordinal: usize,
        preferred_skin: bool,
    },
    ModelFile,
}

struct Candidate<'a> {
    ordinal: usize,
    binding: &'a str,
    skin: bool,
    element: &'a Element,
}

struct Drawing<'source> {
    model_file: Option<&'source str>,
    attachments: Vec<Candidate<'source>>,
    first_skin: Option<usize>,
}

impl<'source> Drawing<'source> {
    fn read(root: &'source Element) -> Result<Self, String> {
        if !root.name.eq_ignore_ascii_case("CharacterDefinition") {
            return Err(format!(
                "the root element is {}, not CharacterDefinition",
                root.name
            ));
        }
        let attachments: Vec<_> = attachment_candidates(root).collect();
        let first_skin = attachments.iter().position(|candidate| candidate.skin);
        Ok(Self {
            model_file: skeleton_file(root),
            attachments,
            first_skin,
        })
    }

    fn body_candidate(&self) -> Option<&'source str> {
        if self.first_skin.is_some() || self.attachments.is_empty() {
            return None;
        }
        self.model_file
            .filter(|path| !is_character_definition(path))
    }

    fn primary_rigid(&self) -> Option<BoneAttachment> {
        let first = self.attachments.first()?;
        BoneAttachment::read(first.element, first.binding)
            .filter(|bone| !is_character_definition(&bone.binding))
    }

    fn project(
        &self,
        selection: Selection,
        mut capture: impl FnMut(usize) -> Option<Arc<[CdfAttachmentSite]>>,
        capture_model: impl Fn(MainSelector) -> Option<CdfModelSources>,
    ) -> Result<Character, String> {
        let chosen = match &selection {
            Selection::Bound => self
                .first_skin
                .or_else(|| (!self.attachments.is_empty()).then_some(0)),
            Selection::DeclaredBody => None,
            Selection::DeclaredRig(_) => Some(0),
        };
        let (model, material, selected) = match chosen.and_then(|at| self.attachments.get(at)) {
            Some(candidate) => (
                slashes(candidate.binding),
                candidate.element.attribute("Material").map(slashes),
                MainSelector::Attachment {
                    ordinal: candidate.ordinal,
                    preferred_skin: candidate.skin,
                },
            ),
            None => (
                slashes(
                    self.model_file
                        .ok_or("it names no model and no attachment")?,
                ),
                None,
                MainSelector::ModelFile,
            ),
        };
        if is_character_definition(&model) {
            return Err("it names another character definition".to_owned());
        }
        let primary_rigid = match selection {
            Selection::DeclaredRig(mut primary) => {
                primary.sources = self
                    .attachments
                    .first()
                    .and_then(|first| capture(first.ordinal));
                Some(primary)
            }
            Selection::Bound | Selection::DeclaredBody => None,
        };
        let rig_sources = primary_rigid
            .as_ref()
            .and_then(|_| capture_model(MainSelector::ModelFile));
        let (bones, left_out) = sort_rest(
            self.attachments
                .iter()
                .enumerate()
                .filter(|(at, _)| Some(*at) != chosen)
                .map(|(_, candidate)| candidate),
            capture,
        );
        Ok(Character {
            model,
            material,
            bones,
            left_out,
            skeleton: self.model_file.map(slashes),
            model_sources: capture_model(selected),
            primary_rigid,
            rig_sources,
        })
    }
}

fn attachment_candidates(root: &Element) -> impl Iterator<Item = Candidate<'_>> {
    root.child("AttachmentList")
        .into_iter()
        .flat_map(|list| list.children.iter().enumerate())
        .filter_map(|(ordinal, attachment)| {
            if !attachment.name.eq_ignore_ascii_case("Attachment") {
                return None;
            }
            let binding = attachment.attribute("Binding").filter(|b| !b.is_empty())?;
            let skin = attachment
                .attribute("Type")
                .is_some_and(|kind| kind.eq_ignore_ascii_case("CA_SKIN"));
            Some(Candidate {
                ordinal,
                binding,
                skin,
                element: attachment,
            })
        })
}

/// Returns the file of the definition's `Model` element, its skeleton.
fn skeleton_file(root: &Element) -> Option<&str> {
    root.child("Model")
        .and_then(|model| model.attribute("File"))
        .filter(|file| !file.is_empty())
}

/// Splits the attachments that are not the drawn model into the bone attachments that are drawn
/// and the files of those that are not.
fn sort_rest<'a>(
    rest: impl Iterator<Item = &'a Candidate<'a>>,
    mut capture: impl FnMut(usize) -> Option<Arc<[CdfAttachmentSite]>>,
) -> (Vec<BoneAttachment>, Vec<String>) {
    let mut bones = Vec::new();
    let mut left_out = Vec::new();
    for candidate in rest {
        let bone = BoneAttachment::read(candidate.element, candidate.binding)
            .filter(|bone| !is_character_definition(&bone.binding));
        match bone {
            Some(mut bone) => {
                bone.sources = capture(candidate.ordinal);
                bones.push(bone);
            }
            None => left_out.push(slashes(candidate.binding)),
        }
    }
    (bones, left_out)
}

/// Returns `path` with `/` as separator, as the other paths of a report are spelled.
pub(crate) fn slashes(path: &str) -> String {
    path.replace('\\', "/")
}

#[cfg(test)]
#[path = "cdf_tests.rs"]
mod tests;

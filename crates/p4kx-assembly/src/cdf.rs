// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::SourceLocation;

use crate::CdfAttachmentSite;
use crate::cdf_attachment_source;
use crate::cdf_attribute_position::CdfCoordinateDomain;
use crate::cdf_bone::BoneAttachment;
use crate::cdf_model_sources::CdfModelSources;
use crate::loadout_xml::Node;
use crate::markup::{self, Budget, Element};

/// The most bytes of a character definition that are read.
pub(crate) const MAX_CDF_BYTES: u64 = 1 << 20;

/// What is drawn of a character definition (`.cdf`): the model of its first skin attachment, else
/// of its first attachment, its bone attachments, and the files of the attachments that are not
/// drawn.
#[derive(Debug, PartialEq)]
pub(crate) struct Character {
    pub(crate) model: String,
    pub(crate) material: Option<String>,
    pub(crate) bones: Vec<BoneAttachment>,
    pub(crate) left_out: Vec<String>,
    pub(crate) skeleton: Option<String>,
    pub(crate) model_sources: Option<CdfModelSources>,
}

/// Returns whether `path` names a character definition.
pub(crate) fn is_character_definition(path: &str) -> bool {
    path.len() > 4
        && path
            .get(path.len() - 4..)
            .is_some_and(|extension| extension.eq_ignore_ascii_case(".cdf"))
}

/// Reads the character definition in `bytes`.
///
/// # Errors
///
/// Returns why the document is no character definition or names nothing to draw.
pub(crate) fn read(bytes: &[u8]) -> Result<Character, String> {
    let mut budget = Budget::new(bytes.len());
    let root = markup::parse(bytes, &mut budget)?;
    project(&root, |_| None, |_| None)
}

pub(crate) fn read_observing_sources(bytes: &[u8]) -> Result<Character, String> {
    let mut budget = Budget::new(bytes.len());
    markup::parse_with_source(bytes, &mut budget, |root, original, scope| {
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
        project(
            &root,
            |ordinal| Some(cdf_attachment_source::capture(attachment(ordinal)?, scope)),
            |selected| match selected {
                MainSelector::Attachment {
                    ordinal,
                    preferred_skin,
                } => CdfModelSources::attachment(attachment(ordinal)?, scope, preferred_skin),
                MainSelector::ModelFile => CdfModelSources::file(model?, scope),
            },
        )
    })
}

pub(crate) fn read_skeleton_with_position(
    bytes: &[u8],
) -> Result<(Option<String>, Option<SourceLocation>), String> {
    let mut budget = Budget::new(bytes.len());
    markup::parse_with_source(bytes, &mut budget, |root, original, scope| {
        let character = project(&root, |_| None, |_| None)?;
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

fn project(
    root: &Element,
    capture: impl FnMut(usize) -> Option<Arc<[CdfAttachmentSite]>>,
    capture_model: impl FnOnce(MainSelector) -> Option<CdfModelSources>,
) -> Result<Character, String> {
    if !root.name.eq_ignore_ascii_case("CharacterDefinition") {
        return Err(format!(
            "the root element is {}, not CharacterDefinition",
            root.name
        ));
    }
    let attachments: Vec<_> = attachment_candidates(root).collect();
    let chosen = attachments
        .iter()
        .position(|candidate| candidate.skin)
        .or_else(|| (!attachments.is_empty()).then_some(0));
    let (model, material, selected) = match chosen.and_then(|at| attachments.get(at)) {
        Some(candidate) => (
            slashes(candidate.binding),
            candidate.element.attribute("Material").map(slashes),
            MainSelector::Attachment {
                ordinal: candidate.ordinal,
                preferred_skin: candidate.skin,
            },
        ),
        None => (
            slashes(skeleton_file(root).ok_or("it names no model and no attachment")?),
            None,
            MainSelector::ModelFile,
        ),
    };
    if is_character_definition(&model) {
        return Err("it names another character definition".to_owned());
    }
    let (bones, left_out) = sort_rest(
        attachments
            .iter()
            .enumerate()
            .filter(|(at, _)| Some(*at) != chosen)
            .map(|(_, candidate)| candidate),
        capture,
    );
    let skeleton = skeleton_file(root).map(slashes);
    Ok(Character {
        model,
        material,
        bones,
        left_out,
        skeleton,
        model_sources: capture_model(selected),
    })
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
mod tests {
    use super::{is_character_definition, read};

    const THRUSTER: &str = r#"<CharacterDefinition>
  <Model File="Objects\t\nozzle_CHR.chr" LODRatio="40"/>
  <AttachmentList>
    <Attachment Type="CA_BONE" AName="Housing" BoneName="housing_attach" Binding="Objects\t\housing.cga" Flags="0"/>
    <Attachment Type="CA_SKIN" AName="Nozzle" Binding="Objects\t\nozzle.skin" Material="Objects\t\paint.mtl"/>
  </AttachmentList>
</CharacterDefinition>"#;

    #[test]
    fn the_first_skin_is_drawn_and_the_other_attachments_are_listed() {
        let character = read(THRUSTER.as_bytes()).expect("a character");
        assert_eq!(character.model, "Objects/t/nozzle.skin");
        assert_eq!(character.material.as_deref(), Some("Objects/t/paint.mtl"));
        assert_eq!(character.left_out.len(), 0);
        let [housing] = character.bones.as_slice() else {
            panic!("one bone attachment: {:?}", character.bones);
        };
        assert_eq!(housing.name, "Housing");
        assert_eq!(housing.bone, "housing_attach");
        assert_eq!(housing.binding, "Objects/t/housing.cga");
        assert_eq!(housing.position, [0.0; 3]);
        assert_eq!(housing.rotation, [1.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn a_bone_attachment_reads_its_offset_rotation_and_material() {
        let text = r#"<CharacterDefinition><AttachmentList>
            <Attachment Type="CA_SKIN" Binding="s.skin"/>
            <Attachment Type="CA_BONE" BoneName="b" Binding="x\Part.cga" Material="m.mtl"
              RelPosition="1,-2.5,3" RelRotation="0,0,1,0"/>
            </AttachmentList></CharacterDefinition>"#;
        let character = read(text.as_bytes()).expect("a character");
        let [part] = character.bones.as_slice() else {
            panic!("one bone attachment: {:?}", character.bones);
        };
        assert_eq!(part.name, "Part");
        assert_eq!(part.material.as_deref(), Some("m.mtl"));
        assert_eq!(part.position, [1.0, -2.5, 3.0]);
        assert_eq!(part.rotation, [0.0, 0.0, 1.0, 0.0]);
    }

    #[test]
    fn a_bone_attachment_that_cannot_be_read_is_left_out() {
        let text = r#"<CharacterDefinition><AttachmentList>
            <Attachment Type="CA_SKIN" Binding="s.skin"/>
            <Attachment Type="CA_BONE" Binding="nobone.cga"/>
            <Attachment Type="CA_BONE" BoneName="b" Binding="badpos.cga" RelPosition="1,2"/>
            <Attachment Type="CA_BONE" BoneName="b" Binding="zero.cga" RelRotation="0,0,0,0"/>
            <Attachment Type="CA_BONE" BoneName="b" Binding="nested.cdf"/>
            <Attachment Type="CA_SKIN" Binding="second.skin"/>
            </AttachmentList></CharacterDefinition>"#;
        let character = read(text.as_bytes()).expect("a character");
        assert_eq!(character.bones.len(), 0);
        assert_eq!(
            character.left_out,
            [
                "nobone.cga",
                "badpos.cga",
                "zero.cga",
                "nested.cdf",
                "second.skin"
            ]
        );
    }

    #[test]
    fn without_a_skin_the_first_attachment_is_drawn() {
        let text = r#"<CharacterDefinition><AttachmentList>
            <Attachment Type="CA_BONE" Binding="a.cga"/><Attachment Type="CA_BONE" Binding="b.cga"/>
            </AttachmentList></CharacterDefinition>"#;
        let character = read(text.as_bytes()).expect("a character");
        assert_eq!(character.model, "a.cga");
        assert_eq!(character.left_out, ["b.cga"]);
    }

    #[test]
    fn without_attachments_the_model_file_is_drawn() {
        let text = r#"<CharacterDefinition><Model File="m.chr"/></CharacterDefinition>"#;
        let character = read(text.as_bytes()).expect("a character");
        assert_eq!(character.model, "m.chr");
        assert_eq!(character.left_out.len(), 0);
    }

    #[test]
    fn a_document_that_names_nothing_or_nests_a_definition_is_refused() {
        assert!(read(b"<CharacterDefinition/>").is_err());
        assert!(read(b"<Other/>").is_err());
        let nested = r#"<CharacterDefinition><AttachmentList>
            <Attachment Type="CA_SKIN" Binding="again.CDF"/></AttachmentList></CharacterDefinition>"#;
        assert!(read(nested.as_bytes()).is_err());
        assert!(read(b"not xml").is_err());
    }

    #[test]
    fn the_extension_is_matched_ignoring_case() {
        assert!(is_character_definition("Objects/x/y.CDF"));
        assert!(!is_character_definition("Objects/x/y.cdfm"));
        assert!(!is_character_definition(".cdf"));
        assert!(!is_character_definition("cdf"));
    }
}

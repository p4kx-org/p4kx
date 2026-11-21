// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Entry;
use p4kx_extraction::cryxml::{
    SourceAttributePosition, SourceDocument, SourceElement, SourceReadOptions,
};

use super::{
    Binding, Location, Match, Observation, Original, ProbeCounts, material_path, model_extension,
};
use crate::TestResult;

impl<'a> Original<'a> {
    pub(super) fn cdf(
        &self,
        bytes: &[u8],
        counts: &mut ProbeCounts,
    ) -> TestResult<Option<Observation<'a>>> {
        let Ok(document) = SourceDocument::parse(bytes, SourceReadOptions::default()) else {
            return Ok(None);
        };
        let root = document.root();
        if document.is_lossy() || !root.name().eq_ignore_ascii_case("CharacterDefinition") {
            return Ok(None);
        }
        counts.recognized += 1;
        for (list, attachments) in root.elements().enumerate() {
            if !attachments.name().eq_ignore_ascii_case("AttachmentList") {
                continue;
            }
            for (child, attached) in attachments.elements().enumerate() {
                if let Some(found) =
                    self.attachment(attached, [list as u64, child as u64], counts)?
                {
                    return Ok(Some(found));
                }
            }
        }
        Ok(None)
    }

    fn attachment(
        &self,
        attached: SourceElement<'_>,
        indices: [u64; 2],
        counts: &mut ProbeCounts,
    ) -> TestResult<Option<Observation<'a>>> {
        if !attached.name().eq_ignore_ascii_case("Attachment") {
            return Ok(None);
        }
        let Some(attribute) = attached.attributes().find(|attr| {
            attr.name().eq_ignore_ascii_case("Material") && attr.namespace().is_none()
        }) else {
            return Ok(None);
        };
        let authored = attribute.value();
        counts.declarations += 1;
        let Some(path) = material_path(authored, true, counts) else {
            return Ok(None);
        };
        let binding = self.find_binding(attached)?;
        if path.beside_model && binding.is_none() {
            counts.binding_unavailable += 1;
            return Ok(None);
        }
        let model = binding.as_ref().map(|binding| binding.target);
        let Some(material) = self.material(model, &path)? else {
            return Ok(None);
        };
        let (namespace, attribute_indices) = match attribute.position() {
            SourceAttributePosition::Text(range) => {
                ("cdf-xml-attribute", [range.start as u64, range.end as u64])
            }
            SourceAttributePosition::CryXml {
                node_index,
                attribute_index,
            } => (
                "cdf-cryxml-attribute",
                [u64::from(*node_index), u64::from(*attribute_index)],
            ),
            _ => return Ok(None),
        };
        Ok(Some(Observation {
            material,
            binding,
            binding_used: path.beside_model,
            authored: authored.to_owned(),
            indices: indices.to_vec(),
            range: None,
            location: Location {
                namespace,
                field: attribute.name().to_owned(),
                indices: attribute_indices,
            },
        }))
    }

    fn binding(&self, authored: &str) -> TestResult<Option<&'a Entry>> {
        let parsed = authored.trim_start_matches(['/', '\\']);
        let root = format!("Data/{parsed}");
        Ok(match self.path_match(&root)? {
            Match::Missing => self.path_match(parsed)?.unique(),
            found => found.unique(),
        })
    }

    fn find_binding(&self, attached: SourceElement<'_>) -> TestResult<Option<Binding<'a>>> {
        let Some(authored) = attached
            .attributes()
            .find(|attr| attr.name().eq_ignore_ascii_case("Binding") && attr.namespace().is_none())
            .and_then(|attr| valid_binding(attr.value()))
        else {
            return Ok(None);
        };
        let target = self.binding(authored)?;
        Ok(target.map(|target| Binding {
            target,
            authored: authored.to_owned(),
        }))
    }
}

fn valid_binding(path: &str) -> Option<&str> {
    let original = path.trim_start_matches(['/', '\\']);
    let (_, extension) = original.rsplit_once('.')?;
    (model_extension(extension)
        && !original.contains([':', '!', '\0'])
        && original
            .split(['/', '\\'])
            .all(|part| !matches!(part, "" | "." | "..")))
    .then_some(path)
}

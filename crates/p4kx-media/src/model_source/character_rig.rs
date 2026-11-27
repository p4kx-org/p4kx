// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::character_definition::CharacterDefinition;

use super::character_origin::CharacterElement;
use super::character_payload::{ATTACHMENT, AttachmentRig, AttachmentRigDisposition};
use super::character_reference::CharacterReferences;
use super::character_rig_input::{self, RigRead};
use super::character_rig_placement::AttachmentPose;
use super::origins::{self, ModelOrigins};
use super::path_reference::{self, Observation};
use super::payload::ModelSourceFamily;
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{
    EntryInput, Identity, MediaError, MediaWriter, OriginalIdentifier, Reference,
    ReferenceResolution, ReferenceUnavailable, SourceContext, Subresource,
};

const RIG: &str = "/data/payload/rig_placement/rig";

pub(super) struct CharacterRig {
    declaration: OriginContributions,
    observation: Option<Box<RigDeclaration>>,
    input: RigRead,
}

struct RigDeclaration {
    path: Observation,
    authored: OriginContributions,
}

impl CharacterRig {
    pub(super) fn read(
        cdf: &CharacterDefinition<'_>,
        origins: &ModelOrigins<'_>,
        references: &CharacterReferences<'_, '_>,
        first_bone: [usize; 2],
        writer: &mut MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let mut selector = writer.capture_origins();
        let root = CharacterElement::new(cdf.root(), origins);
        selector.include_original(&root.element_origin("Model", writer)?)?;
        let mut path = None;
        let mut multiple = false;
        for (_, model) in cdf.models() {
            writer.checkpoint()?;
            let model = CharacterElement::new(model, origins);
            let (value, evidence) = model.value("File", writer)?;
            selector.include_original(&evidence)?;
            if let Some(value) = value {
                multiple |= path.replace((value, evidence)).is_some();
            }
        }
        let declaration = selector.finish();
        let (observation, input) = if multiple {
            (
                None,
                absent(AttachmentRigDisposition::AmbiguousRigDeclaration, origins),
            )
        } else if let Some((path, authored)) = path {
            observe(path, authored, references, first_bone, origins, writer)?
        } else {
            (
                None,
                absent(AttachmentRigDisposition::NoRigDeclaration, origins),
            )
        };
        Ok(Self {
            declaration,
            observation,
            input,
        })
    }

    pub(super) fn write(
        &self,
        authored: &AttachmentPose<'_>,
        prepared: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<Box<AttachmentRig>, MediaError> {
        let path = if let Some(observed) = &self.observation {
            observed
                .path
                .write(RIG, &observed.authored, &self.declaration, prepared, writer)?
        } else {
            origins::at(prepared, RIG, self.declaration.clone(), writer)?;
            self.declaration.clone()
        };
        let rig = self
            .observation
            .as_ref()
            .map(|observed| observed.path.reference.clone());
        authored
            .project(rig, &self.input, &path, prepared, writer)
            .map(Box::new)
    }
}

fn observe(
    path: &str,
    authored: OriginContributions,
    references: &CharacterReferences<'_, '_>,
    first_bone: [usize; 2],
    origins: &ModelOrigins<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(Option<Box<RigDeclaration>>, RigRead), MediaError> {
    let directory = references.context.directory;
    let observation = if path.trim().is_empty()
        || (references.strings_lossy && path.contains(char::REPLACEMENT_CHARACTER))
    {
        Observation::unavailable(
            OriginalIdentifier::new_with_path(path),
            ReferenceUnavailable::InvalidIdentifier,
            writer,
        )?
    } else {
        path_reference::lookup(directory, path, writer)?
    };
    let input = match token(references.context, &observation.reference)? {
        Ok(input) => {
            let at = Subresource::new(ATTACHMENT, first_bone.map(|index| index as u64).to_vec());
            references.context.with_required_input(
                &input,
                &at,
                writer,
                character_rig_input::read,
            )?
        }
        Err(disposition) => absent(disposition, origins),
    };
    let declaration = RigDeclaration {
        path: observation,
        authored,
    };
    Ok((Some(Box::new(declaration)), input))
}

fn absent(disposition: AttachmentRigDisposition, origins: &ModelOrigins<'_>) -> RigRead {
    RigRead::Unavailable {
        disposition,
        skeleton_chunk: None,
        evidence: origins.independent(),
    }
}

fn token<'a>(
    context: &'a SourceContext<'_>,
    reference: &Reference,
) -> Result<Result<EntryInput<'a>, AttachmentRigDisposition>, MediaError> {
    use AttachmentRigDisposition as Disposition;

    let target = match reference.resolution() {
        ReferenceResolution::Resolved { target } => target,
        ReferenceResolution::Missing => return Ok(Err(Disposition::TargetMissing)),
        ReferenceResolution::Ambiguous { .. } => return Ok(Err(Disposition::TargetAmbiguous)),
        ReferenceResolution::NotSelected { .. } => return Ok(Err(Disposition::TargetNotSelected)),
        ReferenceResolution::Unavailable { .. } => return Ok(Err(Disposition::TargetUnavailable)),
    };
    let directory = context.directory;
    let Identity::Entry { containers, entry } = &target.source else {
        return Err(MediaError::Contract("CDF rig has no physical Entry".into()));
    };
    if &target.root != directory.root || containers != directory.chain {
        return Err(MediaError::Contract(
            "CDF rig is outside its owning Archive".into(),
        ));
    }
    if !ModelSourceFamily::of(&entry.path).is_some_and(ModelSourceFamily::is_model) {
        return Ok(Err(Disposition::UnsupportedRig));
    }
    Ok(Ok(EntryInput {
        root: directory.root,
        chain: directory.chain,
        position: entry.entry_index,
    }))
}

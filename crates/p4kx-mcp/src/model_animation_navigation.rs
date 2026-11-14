// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    AnimationFlagRun, ArchiveAnimationAdmission, FieldOrigin, InputScope, SourceCoordinateOmission,
    SourceDetails, SourceEncryption, SourceEvidenceError, SourceInput, SourceLocation, SourceScope,
    SourceUse, SourceUseRole,
};

use crate::content_origin::EntryPageSources;
use crate::model_animation_sites::ChunkSites;

#[derive(Clone, Copy)]
pub(crate) enum Navigation<'a> {
    Complete(&'a [ArchiveAnimationAdmission]),
    Witness(Option<&'a ArchiveAnimationAdmission>),
}

impl<'a> Navigation<'a> {
    pub(crate) fn witness(admissions: &'a [ArchiveAnimationAdmission]) -> Self {
        let encrypted = admissions.iter().find(|admission| {
            admission
                .origin
                .as_ref()
                .is_some_and(|origin| origin.encryption() == SourceEncryption::Encrypted)
        });
        Self::Witness(encrypted.or_else(|| admissions.first()))
    }

    pub(crate) fn origin(
        self,
        model: &EntryPageSources,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let mut uses = vec![model.leaf_path()?];
        let complete = match self {
            Self::Complete(admissions) => {
                let mut complete = true;
                for admission in admissions {
                    complete &= record_admission(model, admission, &mut uses)?;
                }
                complete
            }
            Self::Witness(admission) => {
                if let Some(admission) = admission {
                    uses.push(record_witness(model, admission)?);
                }
                false
            }
        };
        Ok(model.origin_when(&uses, complete))
    }
}

fn record_admission(
    model: &EntryPageSources,
    admission: &ArchiveAnimationAdmission,
    uses: &mut Vec<SourceUse>,
) -> Result<bool, SourceEvidenceError> {
    let Some(origin) = admission.origin.as_ref() else {
        let scope = model
            .uses
            .observe(SourceInput::Unknown, InputScope::Payload)?;
        uses.push(scope.at_opaque(
            SourceUseRole::Selector,
            SourceCoordinateOmission::Unavailable,
        )?);
        return Ok(false);
    };
    let scope = model
        .uses
        .observe(SourceInput::Archive(origin), InputScope::Payload)?;
    let start = uses.len();
    let mut complete = origin.ancestors().all(|step| step.is_some()) && admission.complete;
    for chunk in [admission.metadata.as_ref(), admission.data.as_ref()]
        .into_iter()
        .flatten()
    {
        let sites = ChunkSites::of(&scope, chunk)?;
        complete &= sites.complete;
        uses.extend(sites.selectors);
    }
    if let Some(metadata) = &admission.metadata {
        if let Some(bytes) = &admission.declared_count {
            uses.push(record_count(
                &scope,
                metadata.ordinal(),
                bytes.start,
                bytes.end,
            )?);
        }
        for flags in &admission.flags {
            uses.push(record_flags(&scope, metadata.ordinal(), flags)?);
        }
    }
    if uses.len() == start {
        uses.push(scope.at_opaque(
            SourceUseRole::Selector,
            SourceCoordinateOmission::Unavailable,
        )?);
        complete = false;
    }
    Ok(complete)
}

fn record_witness(
    model: &EntryPageSources,
    admission: &ArchiveAnimationAdmission,
) -> Result<SourceUse, SourceEvidenceError> {
    let source = admission
        .origin
        .as_ref()
        .map_or(SourceInput::Unknown, SourceInput::Archive);
    let scope = model.uses.observe_with_details(
        source,
        InputScope::Payload,
        SourceDetails::CaptureLimited,
    )?;
    if let Some(chunk) = &admission.metadata {
        if let Some(bytes) = &admission.declared_count {
            return record_count(&scope, chunk.ordinal(), bytes.start, bytes.end);
        }
        if let Some(flags) = admission.flags.first() {
            return record_flags(&scope, chunk.ordinal(), flags);
        }
    }
    scope.at_opaque(
        SourceUseRole::Selector,
        SourceCoordinateOmission::Unavailable,
    )
}

fn record_count(
    scope: &SourceScope,
    chunk: usize,
    start: u64,
    end: u64,
) -> Result<SourceUse, SourceEvidenceError> {
    selector(scope, "declaredClipCount", vec![chunk as u64, start, end])
}

fn record_flags(
    scope: &SourceScope,
    chunk: usize,
    flags: &AnimationFlagRun,
) -> Result<SourceUse, SourceEvidenceError> {
    selector(
        scope,
        "clipFlags",
        vec![
            chunk as u64,
            flags.first_clip,
            flags.first_word.start,
            flags.first_word.end,
            flags.stride,
            flags.count,
        ],
    )
}

fn selector(
    scope: &SourceScope,
    field: &str,
    indices: Vec<u64>,
) -> Result<SourceUse, SourceEvidenceError> {
    scope.at(
        SourceLocation::Subresource {
            namespace: "cryEngineAnimationAdmission".into(),
            field: field.into(),
            indices,
        },
        SourceUseRole::Selector,
    )
}

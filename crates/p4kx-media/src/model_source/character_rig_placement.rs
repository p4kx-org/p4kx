// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::{
    AttachmentBoneBinding, Transform, attachment_offset, resolve_attachment_bone,
};
use p4kx_extraction::{InputScope, SourceUseRole};

use super::character_origin::CharacterElement;
use super::character_payload::{AttachmentRig, AttachmentRigDisposition};
use super::character_rig_input::{RigRead, VerifiedRig};
use super::origins;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter, Reference};

type Disposition = AttachmentRigDisposition;

const PLACEMENT: &str = "/data/payload/rig_placement";

pub(super) struct AttachmentPose<'a> {
    pub(super) element: &'a CharacterElement<'a, 'a>,
    pub(super) kind: &'a OriginContributions,
    pub(super) bone: Option<&'a str>,
    pub(super) position: Option<[f32; 3]>,
    pub(super) rotation: Option<[f32; 4]>,
    pub(super) strings_lossy: bool,
}

struct Placement {
    disposition: Disposition,
    ordinals: Vec<usize>,
    lineage: Vec<usize>,
    transform: Option<Transform>,
    uses_offset: bool,
}

impl Placement {
    const fn pending(disposition: Disposition) -> Self {
        Self {
            disposition,
            ordinals: Vec::new(),
            lineage: Vec::new(),
            transform: None,
            uses_offset: false,
        }
    }
}

impl AttachmentPose<'_> {
    pub(super) fn project(
        &self,
        reference: Option<Reference>,
        input: &RigRead,
        path: &OriginContributions,
        prepared: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<AttachmentRig, MediaError> {
        let mut skeleton = writer.capture_origins();
        skeleton.include_original(self.kind)?;
        skeleton.include_original(path)?;
        let (skeleton_chunk, placement, rig) = match input {
            RigRead::Unavailable {
                disposition,
                skeleton_chunk,
                evidence,
            } => {
                skeleton.include_original(evidence)?;
                (*skeleton_chunk, Placement::pending(*disposition), None)
            }
            RigRead::Ready(rig) => {
                skeleton.include_original(&rig.evidence)?;
                (Some(rig.ordinal), self.bind(rig), Some(rig.as_ref()))
            }
        };
        let selection = skeleton.finish();
        let names = self.name_evidence(rig, &selection, writer)?;
        let pose = self.pose_evidence(rig, &placement, &names, writer)?;
        Self::write_origins(selection, names, pose, prepared, writer)?;
        Ok(AttachmentRig {
            rig: reference,
            disposition: placement.disposition,
            skeleton_chunk,
            bone_ordinals: placement.ordinals,
            rig_from_attachment: placement.transform.map(|frame| frame.rows),
        })
    }

    fn bind(&self, rig: &VerifiedRig) -> Placement {
        let Some(name) = self.bone.filter(|name| !name.is_empty()) else {
            return Placement::pending(Disposition::BoneNameMissing);
        };
        let bones = rig.skeleton.bones();
        if name.contains(char::REPLACEMENT_CHARACTER)
            && (self.strings_lossy
                || rig.repaired_names.iter().any(|&ordinal| {
                    bones
                        .get(ordinal)
                        .is_some_and(|bone| bone.name.eq_ignore_ascii_case(name))
                }))
        {
            return Placement::pending(Disposition::BoneNameInvalid);
        }
        match resolve_attachment_bone(bones, name) {
            AttachmentBoneBinding::Missing => Placement::pending(Disposition::MissingBone),
            AttachmentBoneBinding::Ambiguous { ordinals } => Placement {
                ordinals,
                ..Placement::pending(Disposition::AmbiguousBone)
            },
            AttachmentBoneBinding::UnavailableFrame { ordinal, lineage } => Placement {
                ordinals: vec![ordinal],
                lineage,
                ..Placement::pending(Disposition::UnavailableFrame)
            },
            AttachmentBoneBinding::Bound {
                ordinal,
                lineage,
                transform,
            } => self.apply_offset(ordinal, lineage, transform),
            _ => Placement::pending(Disposition::UnavailableFrame),
        }
    }

    fn apply_offset(
        &self,
        ordinal: usize,
        lineage: Vec<usize>,
        rig_from_bone: Transform,
    ) -> Placement {
        let offset = self
            .position
            .zip(self.rotation)
            .and_then(|(position, rotation)| attachment_offset(position, rotation));
        let transform = offset
            .as_ref()
            .and_then(|offset| rig_from_bone.then(offset).finite());
        let disposition = if transform.is_some() {
            Disposition::Bound
        } else if offset.is_some() {
            Disposition::UnavailableFrame
        } else {
            Disposition::InvalidOffset
        };
        Placement {
            disposition,
            ordinals: vec![ordinal],
            lineage,
            transform,
            uses_offset: true,
        }
    }

    fn name_evidence(
        &self,
        rig: Option<&VerifiedRig>,
        selection: &OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut binding = writer.capture_origins();
        binding.include_original(selection)?;
        if let Some(rig) = rig {
            binding.include_original(&self.element.value("BoneName", writer)?.1)?;
            if self.bone.is_some_and(|name| {
                !(name.is_empty()
                    || (self.strings_lossy && name.contains(char::REPLACEMENT_CHARACTER)))
            }) {
                binding.include_original(&rig.names)?;
            }
        }
        Ok(binding.finish())
    }

    fn pose_evidence(
        &self,
        rig: Option<&VerifiedRig>,
        placement: &Placement,
        names: &OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let Some(rig) = rig.filter(|_| !placement.lineage.is_empty()) else {
            return Ok(names.clone());
        };
        let mut frame = writer.capture_origins();
        frame.include_original(names)?;
        for &ordinal in &placement.lineage {
            writer.checkpoint()?;
            let bone = rig.skeleton.sources().bone(ordinal).ok_or_else(|| {
                MediaError::Contract("CDF attachment lost its physical bone provenance".into())
            })?;
            for (range, field, role) in [
                (bone.parent(), "parent", SourceUseRole::Selector),
                (
                    bone.relative_pose(),
                    "relativePose",
                    SourceUseRole::Conversion,
                ),
            ] {
                let indices = [rig.ordinal as u64, ordinal as u64, range.start, range.end];
                let site = OriginSite::new("model-skeleton-bone", &indices, field);
                frame.add(&rig.input, InputScope::Payload, site, role, writer)?;
            }
        }
        if placement.uses_offset {
            for field in ["RelPosition", "RelRotation"] {
                frame.include_original(&self.element.value(field, writer)?.1)?;
            }
        }
        Ok(frame.finish())
    }

    fn write_origins(
        selection: OriginContributions,
        names: OriginContributions,
        pose: OriginContributions,
        prepared: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        for (suffix, evidence) in [
            ("", pose.clone()),
            ("/disposition", pose.clone()),
            ("/skeleton_chunk", selection),
            ("/bone_ordinals", names),
            ("/rig_from_attachment", pose),
        ] {
            origins::at(prepared, &format!("{PLACEMENT}{suffix}"), evidence, writer)?;
        }
        Ok(())
    }
}

pub(super) const fn is_bone(kind: &str) -> bool {
    kind.eq_ignore_ascii_case("CA_BONE")
}

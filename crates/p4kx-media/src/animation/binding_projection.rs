// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::model::{AnimationController, ControllerBinder, ControllerEligibility};
use p4kx_extraction::{InputScope, SourceUseRole};

use super::binding_payload::{Binding, BindingDisposition};
use super::origin::{self as proof, AnimationOrigins};
use super::payload::{self as wire, AnimationPayload};
use super::skeleton_selection::{self, SelectedSkeleton};
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::{MediaError, MediaWriter, Status, Subresource};

pub(super) struct ClipBinding<'skeleton> {
    skeleton: &'skeleton SelectedSkeleton,
    binder: ControllerBinder<'skeleton>,
    ids: Range<u64>,
}

impl<'skeleton> ClipBinding<'skeleton> {
    pub(super) fn new(skeleton: &'skeleton SelectedSkeleton, ids: Range<u64>) -> Self {
        Self {
            skeleton,
            binder: skeleton.source.binder(),
            ids,
        }
    }

    pub(super) fn write(
        &mut self,
        clip: usize,
        controller: &AnimationController<'_>,
        origins: &AnimationOrigins<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let row = self.observe(controller);
        let mut prepared = origins.prepared()?;
        let authored = origins.payload(
            self.ids.start..controller.id_range().end,
            "controllerIdPrefix",
            SourceUseRole::Selector,
            writer,
        )?;
        proof::at(
            &mut prepared,
            "/data/payload/first_controller_ordinal",
            authored.clone(),
            writer,
        )?;
        proof::control(&mut prepared, authored.clone(), writer)?;
        let track = self.track_evidence(controller, origins, &authored, writer)?;
        proof::at(
            &mut prepared,
            "/data/payload/first_track_ordinal",
            track.clone(),
            writer,
        )?;
        let bone = self.bone_evidence(controller, origins, row.first_bone_ordinal, writer)?;
        for field in [
            "/data/payload/first_bone_ordinal",
            "/data/payload/matching_bones",
        ] {
            proof::at(&mut prepared, field, bone.clone(), writer)?;
        }
        let decision = if matches!(row.disposition, BindingDisposition::EmptyController) {
            track.clone()
        } else {
            combine(&track, &bone, writer)?
        };
        let bound = if row.first_track_ordinal == Some(controller.ordinal()) {
            decision.clone()
        } else {
            track
        };
        proof::at(&mut prepared, "/data/payload/disposition", decision, writer)?;
        proof::at(
            &mut prepared,
            "/data/payload/bound_bone_ordinal",
            bound,
            writer,
        )?;
        prepared.write(
            writer,
            &Subresource::new(
                wire::BINDING,
                vec![clip as u64, controller.ordinal() as u64],
            ),
            Status::Complete,
            &AnimationPayload::ControllerBinding(row),
            &[],
        )
    }

    fn observe(&mut self, controller: &AnimationController<'_>) -> Binding {
        let eligible =
            controller.rotation().key_count > 0 || controller.translation().key_count > 0;
        let eligibility = if eligible {
            ControllerEligibility::Track
        } else {
            ControllerEligibility::Empty
        };
        let matched = self.binder.observe(
            controller.ordinal(),
            controller.controller_id(),
            eligibility,
        );
        let disposition = if !eligible {
            BindingDisposition::EmptyController
        } else if matched.first_bone().is_none() {
            BindingDisposition::MissingBone
        } else if matched.bound_bone().is_some() {
            BindingDisposition::Bound
        } else {
            BindingDisposition::DuplicateController
        };
        Binding {
            disposition,
            first_controller_ordinal: matched.first_controller(),
            first_track_ordinal: matched.first_eligible(),
            first_bone_ordinal: matched.first_bone(),
            matching_bones: matched.matching_bones(),
            bound_bone_ordinal: matched.bound_bone(),
        }
    }

    fn track_evidence(
        &self,
        controller: &AnimationController<'_>,
        origins: &AnimationOrigins<'_>,
        authored: &OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let descriptors = origins.payload(
            self.ids.end..controller.table_range().end,
            "controllerKeyCountSurvey",
            SourceUseRole::Selector,
            writer,
        )?;
        combine(authored, &descriptors, writer)
    }

    fn bone_evidence(
        &self,
        controller: &AnimationController<'_>,
        origins: &AnimationOrigins<'_>,
        bone: Option<usize>,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut capture = writer.capture_origins();
        capture.include(
            &origins.payload(
                controller.id_range(),
                "controllerId",
                SourceUseRole::Selector,
                writer,
            )?,
            writer,
        )?;
        capture.include(
            &skeleton_selection::bytes(
                &self.skeleton.input,
                self.skeleton.table.clone(),
                "skeletonChunkSelection",
                writer,
            )?,
            writer,
        )?;
        capture.add(
            &self.skeleton.input,
            InputScope::Payload,
            OriginSite::new(
                "animation-skeleton-controller-table",
                &[self.skeleton.source.chunk_index() as u64],
                "controllerIdInventory",
            ),
            SourceUseRole::Selector,
            writer,
        )?;
        if let Some(bone) = bone {
            let range = self
                .skeleton
                .source
                .controller_id_range(bone)
                .ok_or_else(|| MediaError::Contract("bound bone has no original ID span".into()))?;
            capture.include(
                &skeleton_selection::bytes(
                    &self.skeleton.input,
                    range,
                    "selectedBoneControllerId",
                    writer,
                )?,
                writer,
            )?;
        }
        Ok(capture.finish())
    }
}

fn combine(
    first: &OriginContributions,
    second: &OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = writer.capture_origins();
    capture.include(first, writer)?;
    capture.include(second, writer)?;
    Ok(capture.finish())
}

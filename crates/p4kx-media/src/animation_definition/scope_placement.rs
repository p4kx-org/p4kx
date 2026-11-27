// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{EntityRig, GearPlacementSource, RigPlacement};
use p4kx_extraction::SourceLocation;
use schemars::JsonSchema;
use serde::Serialize;

use crate::animation::entity_loadout_evidence::{self, EntityLoadoutEvidence};
use crate::audio_record_origins as records;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter};

#[derive(Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(super) enum PlacementDetail<'source> {
    Attachment {
        port_path: &'source str,
    },
    LandingGear {
        owner_path: &'source str,
        ordinal: usize,
        #[serde(skip_serializing_if = "Option::is_none")]
        bone: Option<&'source str>,
    },
}

pub(super) struct PlacementEvidence<'source, 'forge> {
    pub(super) rig: &'source EntityRig,
    pub(super) input: &'source MediaOriginInput,
    loadout: &'source EntityLoadoutEvidence<'forge>,
    pub(super) owner: OriginContributions,
}

impl<'source, 'forge> PlacementEvidence<'source, 'forge> {
    pub(super) fn new(
        rig: &'source EntityRig,
        input: &'source MediaOriginInput,
        loadout: &'source EntityLoadoutEvidence<'forge>,
        writer: &MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let owner = rig.attachment_target().map_or_else(
            || Ok(OriginContributions::independent(&writer.origins.owner)),
            |target| loadout.class(target, input, writer),
        )?;
        Ok(Self {
            rig,
            input,
            loadout,
            owner,
        })
    }

    pub(super) fn write(
        self,
        prepared: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<Option<PlacementDetail<'source>>, MediaError> {
        let detail = match &self.rig.placement {
            RigPlacement::Attachment { port_path } => {
                records::field(
                    prepared,
                    "/data/payload/placement_detail/port_path",
                    self.route(writer)?,
                    writer,
                )?;
                PlacementDetail::Attachment { port_path }
            }
            RigPlacement::LandingGear {
                owner_path,
                ordinal,
                bone,
            } => {
                self.gear(prepared, bone.is_some(), writer)?;
                PlacementDetail::LandingGear {
                    owner_path,
                    ordinal: *ordinal,
                    bone: bone.as_deref(),
                }
            }
            _ => return Ok(None),
        };
        records::field(
            prepared,
            "/data/payload/placement_detail/kind",
            OriginContributions::independent(&writer.origins.owner),
            writer,
        )?;
        Ok(Some(detail))
    }

    fn gear(
        &self,
        prepared: &mut PreparedRecordOrigins,
        has_bone: bool,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        records::field(
            prepared,
            "/data/payload/placement_detail/owner_path",
            self.route(writer)?,
            writer,
        )?;
        let source = self.rig.gear_source();
        records::field(
            prepared,
            "/data/payload/placement_detail/ordinal",
            self.location(
                source.map(GearPlacementSource::ordinal_selector).as_ref(),
                writer,
            )?,
            writer,
        )?;
        if has_bone {
            records::field(
                prepared,
                "/data/payload/placement_detail/bone",
                self.location(
                    source
                        .and_then(GearPlacementSource::declared_bone_selector)
                        .as_ref(),
                    writer,
                )?,
                writer,
            )?;
        }
        Ok(())
    }

    pub(super) fn route(
        &self,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        self.loadout.position_route(self.rig, self.input, writer)
    }

    fn location(
        &self,
        location: Option<&SourceLocation>,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut capture = writer.capture_origins();
        entity_loadout_evidence::record_location(&mut capture, self.input, location, writer)?;
        Ok(capture.finish())
    }
}

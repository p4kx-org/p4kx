// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{EntityRig, GearPlacementSource, RigPlacement};
use p4kx_extraction::{InputScope, SourceLocation, SourceUseRole};
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_record_origins as records;
use crate::origin_capture::{OriginCapture, OriginSite};
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

pub(super) struct PlacementEvidence<'source> {
    pub(super) rig: &'source EntityRig,
    pub(super) input: &'source MediaOriginInput,
}

impl<'source> PlacementEvidence<'source> {
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

    fn route(&self, writer: &MediaWriter<'_>) -> Result<OriginContributions, MediaError> {
        let mut capture = writer.capture_origins();
        for selector in self.rig.port_selectors() {
            self.add(&mut capture, selector.as_ref(), writer)?;
        }
        Ok(capture.finish())
    }

    fn location(
        &self,
        location: Option<&SourceLocation>,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut capture = writer.capture_origins();
        self.add(&mut capture, location, writer)?;
        Ok(capture.finish())
    }

    fn add(
        &self,
        capture: &mut OriginCapture,
        location: Option<&SourceLocation>,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let Some(SourceLocation::DataForge {
            record_index,
            field,
        }) = location
        else {
            return capture.include(&OriginContributions::unknown(&writer.origins.owner), writer);
        };
        capture.add(
            self.input,
            InputScope::Payload,
            OriginSite::new("dataforge-record", &[u64::from(*record_index)], field),
            SourceUseRole::Selector,
            writer,
        )
    }
}

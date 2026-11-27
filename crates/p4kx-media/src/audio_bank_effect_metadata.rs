// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::{BankError, HircKind};

use crate::audio_bank_node_declaration_payload::NodeDeclaration;
use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_origins::BankOrigins;
use crate::{MediaError, MediaIssue, Severity, Status};

#[path = "audio_bank_modulator_metadata.rs"]
pub(super) mod modulator;
#[path = "audio_bank_effect_metadata_payload.rs"]
pub(crate) mod payload;
#[path = "audio_bank_plugin_metadata.rs"]
pub(super) mod plugin;

use self::payload::MetadataCoverage;

fn coverage(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    kind: HircKind,
    checked: &Range<u64>,
    unprojected: Option<&Range<u64>>,
    has_issue: bool,
) -> Result<MetadataCoverage, MediaError> {
    sites.summary(prepared, Some(checked), has_issue)?;
    sites.range(prepared, "observed/coverage/checked_range", checked)?;
    if let Some(range) = unprojected {
        sites.range(prepared, "observed/coverage/unprojected_range", range)?;
    }
    let original_type = prepared.capture(sites.property_context())?;
    prepared.field("/data/payload/observed/coverage/object_kind", original_type)?;
    Ok(MetadataCoverage {
        object_kind: kind.slug(),
        checked_range: checked.into(),
        unprojected_range: unprojected.map(Into::into),
    })
}

fn declaration<T>(
    version: u32,
    error: Option<&BankError>,
    observed: T,
    issue_code: &'static str,
) -> (NodeDeclaration<T>, Status, Option<MediaIssue>) {
    match error {
        Some(error) => (
            NodeDeclaration::Malformed { version, observed },
            Status::Partial,
            Some(MediaIssue::new(
                issue_code,
                "HIRC",
                Severity::Error,
                error.to_string(),
            )),
        ),
        None => (
            NodeDeclaration::Decoded { version, observed },
            Status::Complete,
            None,
        ),
    }
}

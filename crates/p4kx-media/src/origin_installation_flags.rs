// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, FieldPath, SourceUse, SourceUses};

use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter, RootSource, origin_error};

#[derive(Debug, Clone)]
pub(crate) struct BoundFlags {
    value: u8,
    owner: SourceUses,
    used: SourceUse,
}

pub(crate) fn bind(
    archive: &Archive,
    root: &mut RootSource,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let observer = writer.origins.observer()?;
    if !observer.root.is_same_input(archive.input_origin()) {
        return Err(origin_error::foreign());
    }
    if let Some(value) = archive.installation_status().raw_flags()
        && let Some(used) = observer
            .owner
            .observe_root_installation_flags(archive)
            .map_err(origin_error::evidence)?
    {
        root.installation_flags_origin = Some(BoundFlags {
            value,
            owner: observer.owner,
            used,
        });
    }
    Ok(())
}

pub(crate) fn insert(
    root: &RootSource,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let Some(bound) = &root.installation_flags_origin else {
        return Ok(());
    };
    if Some(bound.value) != root.installation_flags
        || !bound.owner.is_same_inventory(&writer.origins.owner)
    {
        return Ok(());
    }
    let path = FieldPath::parse("/data/installation_flags")
        .map_err(|cause| origin_error::address(&cause))?;
    let mut capture = writer.capture_origins();
    capture.include(
        &OriginContributions {
            uses: [bound.used.clone()].into(),
            ..OriginContributions::independent(&bound.owner)
        },
        writer,
    )?;
    prepared.insert(&path, capture.finish(), writer)?;
    Ok(())
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{Attachment, EntityNode, PortNode};
use p4kx_extraction::{DataForge, DataForgeOrigin, RecordJsonLocation};

use crate::forge_origins::{EvidenceError, ForgeOrigins};
use crate::forge_shapes::ForgeFieldOrigins;

pub(super) fn identities<'port>(
    forge: &DataForge<'_>,
    origin: &DataForgeOrigin,
    root: &EntityNode,
    ports: impl IntoIterator<Item = &'port PortNode>,
) -> Result<ForgeFieldOrigins, EvidenceError> {
    let mut output = ForgeOrigins::new(origin)?;
    output.put("archive", output.uses.origin(&[], true))?;
    entity(forge, &mut output, root, "entity")?;
    for (row, port) in ports.into_iter().enumerate() {
        if let Attachment::Entity(child) = port.attachment() {
            entity(forge, &mut output, child, &format!("ports/{row}/entity"))?;
        }
    }
    output.finish()
}

fn entity(
    forge: &DataForge<'_>,
    output: &mut ForgeOrigins,
    entity: &EntityNode,
    pointer: &str,
) -> Result<(), EvidenceError> {
    let record = entity
        .record_index()
        .and_then(|index| forge.record(index))
        .filter(|record| record.guid() == entity.guid());
    let identity = RecordJsonLocation::identity(
        forge,
        record.as_ref(),
        &output.uses,
        &output.scope,
        output.complete,
    )?;
    for (member, origin) in identity {
        output.put(&format!("{pointer}/{member}"), origin)?;
    }
    Ok(())
}

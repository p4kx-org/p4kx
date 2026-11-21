// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{Attachment, EntityNode, PortNode};
use p4kx_extraction::{DataForge, DataForgeOrigin, RecordJsonLocation};

use crate::forge_origins::{EvidenceError, ForgeOrigins};
use crate::forge_shapes::ForgeFieldOrigins;

use super::ResolvedLoadout;

pub(super) fn capture<'port>(
    forge: &DataForge<'_>,
    origin: &DataForgeOrigin,
    resolved: &ResolvedLoadout,
    ports: impl IntoIterator<Item = &'port PortNode>,
) -> Result<ForgeFieldOrigins, EvidenceError> {
    let mut output = ForgeOrigins::new(origin)?;
    output.put("archive", output.uses.origin(&[], true))?;
    entity(forge, &mut output, &resolved.tree, "entity")?;
    for (row, port) in ports.into_iter().enumerate() {
        let name = resolved.port_name(&output, port)?;
        output.put(&format!("ports/{row}/name"), name)?;
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

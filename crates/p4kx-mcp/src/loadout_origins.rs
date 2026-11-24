// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{Attachment, EntityNode};
use p4kx_extraction::{DataForge, DataForgeOrigin, FieldOrigin, RecordJsonLocation, SourceUseRole};

use crate::forge_origins::{EvidenceError, ForgeOrigins};
use crate::forge_shapes::ForgeFieldOrigins;

use super::{PagePort, ResolvedLoadout};

pub(super) fn capture(
    forge: &DataForge<'_>,
    origin: &DataForgeOrigin,
    resolved: &ResolvedLoadout,
    ports: &[PagePort<'_>],
) -> Result<ForgeFieldOrigins, EvidenceError> {
    let mut output = ForgeOrigins::new(origin)?;
    output.put("archive", output.uses.origin(&[], true))?;
    entity(forge, &mut output, &resolved.tree, "entity")?;
    for (index, row) in ports.iter().enumerate() {
        let name = resolved.port_name(&output, row.port, SourceUseRole::Payload)?;
        let route = route_origin(&output, resolved, row)?;
        let address = format!("ports/{index}");
        output.put(&format!("{address}/name"), name)?;
        output.put(&format!("{address}/path"), route.clone())?;
        output.put(&format!("{address}/depth"), route)?;
        if let Attachment::Entity(child) = row.port.attachment() {
            entity(forge, &mut output, child, &format!("{address}/entity"))?;
        }
    }
    output.finish()
}

fn route_origin(
    output: &ForgeOrigins,
    resolved: &ResolvedLoadout,
    row: &PagePort<'_>,
) -> Result<FieldOrigin, EvidenceError> {
    let names = row
        .ancestors
        .iter()
        .copied()
        .chain(std::iter::once(row.port));
    let inputs = names
        .map(|port| resolved.port_name(output, port, SourceUseRole::Conversion))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(output.uses.origin_with_origins(&inputs, true)?)
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

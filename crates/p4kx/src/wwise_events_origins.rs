// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::audio::{AtlTriggerOrigins, AudioIndex, TriggerHit};
use p4kx_extraction::{
    Archive, FieldOrigin, FieldOrigins, FieldPath, SourceInventory, SourceUseRole, SourceUses,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DefinitionOrigins {
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

pub(super) fn capture(
    archive: &Archive,
    index: &AudioIndex,
    hits: &[TriggerHit<'_>],
) -> Result<DefinitionOrigins> {
    let uses = SourceUses::with_compact_use_refs();
    let mut fields = FieldOrigins::new();
    for (row, hit) in hits.iter().enumerate() {
        let definition = AtlTriggerOrigins::open(archive, index, &uses, hit.trigger())?;
        let path = format!("/triggers/{row}");
        put(
            &mut fields,
            &path,
            "name",
            definition.name(SourceUseRole::Payload)?,
        )?;
        put(&mut fields, &path, "definition", definition.path()?)?;
        for (ordinal, (event, _)) in hit.events().enumerate() {
            let path = format!("{path}/events/{ordinal}");
            let event = event.event();
            put(
                &mut fields,
                &path,
                "name",
                definition.event_name(event, SourceUseRole::Payload)?,
            )?;
            put(
                &mut fields,
                &path,
                "localised",
                definition.event_localised(event)?,
            )?;
        }
    }
    Ok(DefinitionOrigins {
        field_origins: fields,
        provenance: uses.freeze()?,
    })
}

fn put(fields: &mut FieldOrigins, row: &str, field: &str, origin: FieldOrigin) -> Result<()> {
    fields.insert(FieldPath::parse(&format!("{row}/{field}"))?, origin)?;
    Ok(())
}

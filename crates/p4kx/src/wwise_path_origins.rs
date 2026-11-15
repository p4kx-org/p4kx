// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;
use std::slice;

use anyhow::Result;
use p4kx_assembly::audio::{HircPathSource, TriggerSounds};
use p4kx_extraction::{
    Archive, FieldOrigin, FieldOrigins, FieldPath, InputScope, SourceInput, SourceInventory,
    SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
};

struct StepOrigins {
    kind: FieldOrigin,
    id: FieldOrigin,
}

struct Capture<'archive> {
    archive: &'archive Archive,
    uses: SourceUses,
    fields: FieldOrigins,
}

pub(crate) fn capture(
    archive: &Archive,
    sounds: &TriggerSounds<'_>,
) -> Result<(FieldOrigins, SourceInventory)> {
    let mut capture = Capture {
        archive,
        uses: SourceUses::with_compact_use_refs(),
        fields: FieldOrigins::new(),
    };
    for (index, variant) in sounds.variants().iter().enumerate() {
        let row = row_path("variants", index)?;
        let end = capture.path(&row, variant.path().len(), variant.path_sources())?;
        if let Some(origin) = end {
            capture.fields.insert(field(&row, "sound_id")?, origin.id)?;
        }
    }
    for (index, link) in sounds.breaks().iter().enumerate() {
        let row = row_path("breaks", index)?;
        let end = capture.path(&row, link.path().len(), link.path_sources())?;
        if let Some(origin) = end {
            capture.put_step(&field(&row, "object")?, &origin)?;
        }
    }
    Ok((capture.fields, capture.uses.freeze()?))
}

impl Capture<'_> {
    fn path(
        &mut self,
        row: &FieldPath,
        length: usize,
        sources: &[HircPathSource],
    ) -> Result<Option<StepOrigins>> {
        let path = field(row, "path")?;
        let mut end = None;
        for position in 0..length {
            let mut address = path.clone();
            address.push_index(position)?;
            let origin = self.step(sources.get(position))?;
            self.put_step(&address, &origin)?;
            end = Some(origin);
        }
        Ok(end)
    }

    fn step(&self, source: Option<&HircPathSource>) -> Result<StepOrigins> {
        let observed = source.and_then(|source| {
            let entry = self.archive.entries().get(source.entry_position())?;
            self.archive
                .observed_entry(entry)
                .map(|entry| (source, entry))
        });
        let Some((source, entry)) = observed else {
            let unknown = self.uses.origin(&[], false);
            return Ok(StepOrigins {
                kind: unknown.clone(),
                id: unknown,
            });
        };
        let complete = entry.ancestors().all(|ancestor| ancestor.is_some());
        let scope = self
            .uses
            .observe(SourceInput::ArchiveView(entry), InputScope::Payload)?;
        let id = used(
            &scope,
            "bnkHircObject",
            "id",
            source.id_bytes(),
            SourceUseRole::Payload,
        )?;
        let mut kind = vec![used(
            &scope,
            "bnkHircObject",
            "objectExtent",
            source.object_bytes(),
            SourceUseRole::Conversion,
        )?];
        let version = source.version_bytes();
        let kind_complete = complete && version.is_some();
        if let Some(version) = version {
            kind.push(used(
                &scope,
                "bnkBkhd",
                "version",
                version,
                SourceUseRole::Selector,
            )?);
        }
        Ok(StepOrigins {
            kind: self.uses.origin(&kind, kind_complete),
            id: self.uses.origin(slice::from_ref(&id), complete),
        })
    }

    fn put_step(&mut self, path: &FieldPath, origin: &StepOrigins) -> Result<()> {
        self.fields
            .insert(field(path, "kind")?, origin.kind.clone())?;
        self.fields.insert(field(path, "id")?, origin.id.clone())?;
        Ok(())
    }
}

fn row_path(section: &str, index: usize) -> Result<FieldPath> {
    let mut path = field(&FieldPath::root(), section)?;
    path.push_index(index)?;
    Ok(path)
}

fn field(base: &FieldPath, name: &str) -> Result<FieldPath> {
    let mut path = base.clone();
    path.push_field(name)?;
    Ok(path)
}

fn used(
    scope: &SourceScope,
    namespace: &str,
    field: &str,
    range: Range<usize>,
    role: SourceUseRole,
) -> Result<SourceUse> {
    Ok(scope.at(
        SourceLocation::Subresource {
            namespace: namespace.to_owned(),
            indices: vec![u64::try_from(range.start)?, u64::try_from(range.end)?],
            field: field.to_owned(),
        },
        role,
    )?)
}

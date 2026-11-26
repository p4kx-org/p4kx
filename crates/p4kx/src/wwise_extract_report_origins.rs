// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::{Context, Result, ensure};
use p4kx_assembly::audio::{AudioIndex, ExportReport, ExportedSound, SoundVariant, TriggerSounds};
use p4kx_extraction::{Archive, FieldOrigin, FieldOrigins, FieldPath, SourceInventory, SourceUses};

use crate::cli::AudioOutput;
use crate::cli_wwise::SoundExportArgs;

use super::{Capture, field, index_path};

pub(crate) fn capture_extract(
    archive: &Archive,
    index: &AudioIndex,
    sounds: &TriggerSounds<'_>,
    report: &ExportReport,
    args: &SoundExportArgs,
) -> Result<(FieldOrigins, SourceInventory)> {
    let uses = SourceUses::with_compact_use_refs();
    let mut capture = Capture::open(archive, index, &uses, sounds)?;
    capture.header(sounds)?;
    capture.break_count(sounds, index)?;
    capture.issues(index)?;
    capture_files(
        &mut capture.fields,
        (archive, index, &uses),
        &FieldPath::root(),
        report,
        &[FileTrigger {
            sounds,
            selector: None,
        }],
        args,
    )?;
    Ok((capture.fields, uses.freeze()?))
}

impl Capture<'_> {
    fn break_count(&mut self, sounds: &TriggerSounds<'_>, index: &AudioIndex) -> Result<()> {
        let searched = self.searched_banks(sounds, index)?;
        let mut observed = vec![
            self.definition.selection.clone(),
            self.definition.topology.clone(),
        ];
        for variant in sounds.variants() {
            let event = self.definition.event(&self.uses, variant.event())?;
            let route = self.path(
                None,
                variant.path().len(),
                variant.path_sources(),
                &event.id,
            )?;
            observed.push(route.path);
        }
        for link in sounds.breaks() {
            let event = self.definition.event(&self.uses, link.event())?;
            let route = self.path(None, link.path().len(), link.path_sources(), &event.id)?;
            let reason = self.observed_break(link, &route, &event.id, searched.as_ref())?;
            observed.extend([route.path, reason]);
        }
        let origin = self.union(&observed, true)?;
        self.put(&FieldPath::root(), "breaks", origin)
    }
}

pub(crate) struct FileTrigger<'a, 'archive> {
    pub(crate) sounds: &'a TriggerSounds<'archive>,
    pub(crate) selector: Option<&'a FieldOrigin>,
}

pub(crate) fn capture_files(
    fields: &mut FieldOrigins,
    (archive, index, uses): (&Archive, &AudioIndex, &SourceUses),
    root: &FieldPath,
    report: &ExportReport,
    triggers: &[FileTrigger<'_, '_>],
    args: &SoundExportArgs,
) -> Result<()> {
    let mut scopes = triggers
        .iter()
        .map(|trigger| {
            let mut capture = Capture::open(archive, index, uses, trigger.sounds)?;
            if let Some(selector) = trigger.selector {
                capture.definition.selection =
                    capture.union([&capture.definition.selection, selector], true)?;
            }
            Ok(capture)
        })
        .collect::<Result<Vec<_>>>()?;
    let files = field(root, "files")?;
    let mut origins = Vec::with_capacity(report.sounds.len());
    for (ordinal, sound) in report.sounds.iter().enumerate() {
        let selection = sound.selection;
        let trigger = triggers
            .get(selection.trigger_ordinal)
            .context("sound export trigger ordinal is missing")?;
        let capture = scopes
            .get_mut(selection.trigger_ordinal)
            .context("sound export trigger scope is missing")?;
        let row = index_path(&files, ordinal)?;
        let origin = capture.file(fields, &row, trigger.sounds, sound, args.format)?;
        fields.insert(row, origin.clone())?;
        origins.push(origin);
    }
    fields.insert(files, uses.origin_with_origins(&origins, true)?)?;
    Ok(())
}

impl Capture<'_> {
    fn file(
        &mut self,
        fields: &mut FieldOrigins,
        row: &FieldPath,
        sounds: &TriggerSounds<'_>,
        sound: &ExportedSound,
        format: AudioOutput,
    ) -> Result<FieldOrigin> {
        let selection = sound.selection;
        let variants = sounds.variants();
        let route = variants
            .get(selection.variant_ordinal)
            .context("sound export variant ordinal is missing")?;
        let media = variants
            .get(selection.media_variant_ordinal)
            .context("sound export media variant ordinal is missing")?;
        ensure!(
            route.wem_id() == Some(sound.wem_id) && media.wem_id() == Some(sound.wem_id),
            "sound export variants disagree with the reported WEM ID"
        );
        let name = self.union([&self.definition.name, &self.definition.selection], true)?;
        let id = self.exported_media_id(route)?;
        let format_origin = if matches!(format, AudioOutput::Raw) {
            self.uses.origin(&[], true)
        } else {
            let location = self.exported_location(media)?;
            self.union([&location], false)?
        };
        let file = self.union([&name, &id, &format_origin], false)?;
        let size = sound.written_size_origin(self.archive, &self.uses)?;
        let unknown = self.uses.origin(&[], false);
        let receipt = if sound.origin_receipt.is_some() {
            file.clone()
        } else {
            unknown.clone()
        };
        let members = [
            ("trigger", name),
            ("wem_id", id),
            ("file", file),
            ("origin_receipt", receipt),
            ("origin_receipt_failure", unknown.clone()),
            ("format", format_origin.clone()),
            ("size", size),
            ("kept", unknown),
            ("unconverted", format_origin),
        ];
        let origin = self.union(members.iter().map(|(_, origin)| origin), true)?;
        for (name, origin) in members {
            fields.insert(field(row, name)?, origin)?;
        }
        Ok(origin)
    }

    fn exported_media_id(&mut self, variant: &SoundVariant<'_>) -> Result<FieldOrigin> {
        let event = self.definition.event(&self.uses, variant.event())?;
        let route = self.path(
            None,
            variant.path().len(),
            variant.path_sources(),
            &event.id,
        )?;
        let terminal = variant.terminal_media_origins(self.archive, &self.uses)?;
        self.union([&route.path, &terminal.wem_id], true)
    }

    fn exported_location(&self, variant: &SoundVariant<'_>) -> Result<FieldOrigin> {
        let sites = variant.wem_location_origins(self.archive, &self.uses)?;
        self.union(
            [&sites.kind, &sites.path, &sites.len]
                .into_iter()
                .chain(sites.offset.as_ref()),
            true,
        )
    }
}

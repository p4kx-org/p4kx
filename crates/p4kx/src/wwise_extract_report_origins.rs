// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::{Context, Result, ensure};
use p4kx_assembly::audio::{
    AudioIndex, ExportFailure, ExportFailureKind, ExportFailureOrigin, ExportedSound, SoundVariant,
    TerminalMediaOrigins, TriggerSounds, TxtpReportWithOrigins, WemSelection,
};
use p4kx_extraction::{Archive, FieldOrigin, FieldOrigins, FieldPath, SourceInventory, SourceUses};

use crate::cli::AudioOutput;
use crate::cli_wwise::SoundExportArgs;
use crate::wwise_extract::SoundExport;

use super::{Capture, field, index_path};

pub(crate) fn capture_extract(
    archive: &Archive,
    index: &AudioIndex,
    sounds: &TriggerSounds<'_>,
    exported: &SoundExport,
    args: &SoundExportArgs,
    txtp: Option<&TxtpReportWithOrigins>,
) -> Result<(FieldOrigins, SourceInventory)> {
    let uses = SourceUses::with_compact_use_refs();
    let mut capture = Capture::open(archive, index, &uses, sounds)?;
    capture.header(sounds)?;
    capture.break_count(sounds, index)?;
    capture.issues(index)?;
    if let Some(outcome) = txtp {
        capture.txtp_skips(sounds, outcome)?;
    }
    capture_export(
        &mut capture.fields,
        (archive, index, &uses),
        &FieldPath::root(),
        exported,
        &[FileTrigger {
            sounds,
            selector: None,
        }],
        args,
    )?;
    Ok((capture.fields, uses.freeze()?))
}

impl Capture<'_> {
    fn txtp_skips(
        &mut self,
        sounds: &TriggerSounds<'_>,
        outcome: &TxtpReportWithOrigins,
    ) -> Result<()> {
        let skips = &outcome.report.skipped;
        ensure!(
            skips.len() == outcome.skipped_sources.len(),
            "TXTP skip positions do not match their report"
        );
        let variants = sounds.variants();
        let path = field(&field(&FieldPath::root(), "txtp")?, "skips")?;
        for (ordinal, (row, observed)) in skips.iter().zip(&outcome.skipped_sources).enumerate() {
            ensure!(
                observed.trigger_ordinal == 0 && row.trigger == sounds.trigger().name(),
                "TXTP skip does not match its source trigger"
            );
            let event = variants
                .get(observed.event_variant_ordinal)
                .context("TXTP skip event source is missing")?
                .event();
            let trigger = self.exported_trigger()?;
            let name = self.definition.event(&self.uses, event)?.name;
            let reason = if let Some(leaf) = observed.failed_variant_ordinal {
                let variant = variants.get(leaf).context("TXTP skipped leaf is missing")?;
                ensure!(
                    std::ptr::eq(variant.event(), event),
                    "TXTP skipped leaf is from another event"
                );
                let (route, terminal) = self.exported_route(variant)?;
                self.union(
                    [&route, &terminal.source_kind, &terminal.declared_source_id],
                    variant.wem_id().is_none(),
                )?
            } else {
                self.uses.origin(&[], false)
            };
            self.group(
                &index_path(&path, ordinal)?,
                [("trigger", trigger), ("event", name), ("reason", reason)],
            )?;
        }
        Ok(())
    }

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

pub(crate) fn capture_export(
    fields: &mut FieldOrigins,
    (archive, index, uses): (&Archive, &AudioIndex, &SourceUses),
    root: &FieldPath,
    exported: &SoundExport,
    triggers: &[FileTrigger<'_, '_>],
    args: &SoundExportArgs,
) -> Result<()> {
    let report = &exported.report;
    let captured = exported
        .origins
        .as_ref()
        .context("audio report origins were not captured")?;
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
    let mut file_origins = Vec::with_capacity(report.sounds.len());
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
        file_origins.push(origin);
    }
    fields.insert(files, uses.origin_with_origins(&file_origins, true)?)?;
    let failures = field(root, "failures")?;
    let failure_origins = capture_failures(
        fields,
        &failures,
        &report.failures,
        captured.failures(),
        (triggers, &mut scopes),
    )?;
    fields.insert(failures, uses.origin_with_origins(&failure_origins, true)?)?;
    Ok(())
}

fn capture_failures(
    fields: &mut FieldOrigins,
    failures: &FieldPath,
    rows: &[ExportFailure],
    provenance: &[ExportFailureOrigin],
    (triggers, scopes): (&[FileTrigger<'_, '_>], &mut [Capture<'_>]),
) -> Result<Vec<FieldOrigin>> {
    ensure!(
        rows.len() == provenance.len(),
        "audio failure positions do not match their report"
    );
    rows.iter()
        .zip(provenance)
        .enumerate()
        .map(|(ordinal, (failure, proof))| {
            let trigger = triggers
                .get(proof.trigger_ordinal)
                .context("audio failure trigger ordinal is missing")?;
            let capture = scopes
                .get_mut(proof.trigger_ordinal)
                .context("audio failure trigger scope is missing")?;
            let row = index_path(failures, ordinal)?;
            let origin = capture.failed_file(fields, &row, trigger.sounds, failure, proof)?;
            fields.insert(row, origin.clone())?;
            Ok(origin)
        })
        .collect()
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
            sounds.trigger().name() == sound.trigger
                && route.wem_id() == Some(sound.wem_id)
                && media.wem_id() == Some(sound.wem_id),
            "sound export variants disagree with the reported trigger or WEM ID"
        );
        let name = self.exported_trigger()?;
        let (path, terminal) = self.exported_route(route)?;
        let id = self.union([&path, &terminal.wem_id], true)?;
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
        self.insert_export_row(fields, row, members)
    }

    fn failed_file(
        &mut self,
        fields: &mut FieldOrigins,
        row: &FieldPath,
        sounds: &TriggerSounds<'_>,
        failure: &ExportFailure,
        proof: &ExportFailureOrigin,
    ) -> Result<FieldOrigin> {
        let variant = sounds
            .variants()
            .get(proof.variant_ordinal)
            .context("audio failure variant ordinal is missing")?;
        ensure!(
            sounds.trigger().name() == failure.trigger && variant.wem_id() == failure.wem_id,
            "audio failure variant disagrees with its reported trigger or WEM ID"
        );
        let name = self.exported_trigger()?;
        let (path, terminal) = self.exported_route(variant)?;
        let id = self.union([&path, &terminal.wem_id], true)?;
        let reason = match proof.kind {
            ExportFailureKind::FilelessSource => self.union(
                [&path, &terminal.source_kind, &terminal.declared_source_id],
                true,
            )?,
            ExportFailureKind::UnresolvedMedia => self.union(
                [&id, &terminal.source_kind],
                matches!(variant.wem_selection(), WemSelection::UnsupportedStorage),
            )?,
            ExportFailureKind::Job => self.uses.origin(&[], false),
        };
        self.insert_export_row(
            fields,
            row,
            [("trigger", name), ("wem_id", id), ("reason", reason)],
        )
    }

    fn insert_export_row<const N: usize>(
        &self,
        fields: &mut FieldOrigins,
        row: &FieldPath,
        members: [(&str, FieldOrigin); N],
    ) -> Result<FieldOrigin> {
        let origin = self.union(members.iter().map(|(_, origin)| origin), true)?;
        for (name, origin) in members {
            fields.insert(field(row, name)?, origin)?;
        }
        Ok(origin)
    }

    fn exported_trigger(&self) -> Result<FieldOrigin> {
        self.union([&self.definition.name, &self.definition.selection], true)
    }

    fn exported_route(
        &mut self,
        variant: &SoundVariant<'_>,
    ) -> Result<(FieldOrigin, TerminalMediaOrigins)> {
        let event = self.definition.event(&self.uses, variant.event())?;
        let route = self.path(
            None,
            variant.path().len(),
            variant.path_sources(),
            &event.id,
        )?;
        let terminal = variant.terminal_media_origins(self.archive, &self.uses)?;
        Ok((route.path, terminal))
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

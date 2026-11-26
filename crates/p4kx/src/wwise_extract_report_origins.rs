// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_assembly::audio::{AudioIndex, TriggerSounds};
use p4kx_extraction::{Archive, FieldOrigins, FieldPath, SourceInventory, SourceUses};

use super::Capture;

pub(crate) fn capture_extract(
    archive: &Archive,
    index: &AudioIndex,
    sounds: &TriggerSounds<'_>,
) -> Result<(FieldOrigins, SourceInventory)> {
    let uses = SourceUses::with_compact_use_refs();
    let mut capture = Capture::open(archive, index, &uses, sounds)?;
    capture.header(sounds)?;
    capture.break_count(sounds, index)?;
    capture.issues(index)?;
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

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use super::name::{StemSet, portable_stem};
use super::report_origins::FailedSound;
use super::source::{ResolvedMedia, SelectedMedia, WemExportSource};
use super::{ExportFailureKind, ExportFailureOrigin, ExportSoundSelection};
use crate::audio::TriggerSounds;

const FALLBACK_STEM: &str = "sound";

#[derive(Debug)]
pub(super) struct Job {
    pub(super) trigger: String,
    pub(super) stem: String,
    pub(super) wem_id: u32,
    pub(super) source: WemExportSource,
    pub(super) selection: ExportSoundSelection,
}

#[derive(Debug, Default)]
pub(super) struct Plan {
    pub(super) jobs: Vec<Job>,
    pub(super) failures: Vec<FailedSound>,
}

pub(super) fn build(triggers: &[&TriggerSounds<'_>]) -> Plan {
    let mut plan = Plan::default();
    let mut stems = StemSet::default();
    for (trigger_ordinal, sounds) in triggers.iter().enumerate() {
        plan.add_trigger(trigger_ordinal, sounds, &mut stems);
    }
    plan
}

impl Plan {
    fn add_trigger(
        &mut self,
        trigger_ordinal: usize,
        sounds: &TriggerSounds<'_>,
        stems: &mut StemSet,
    ) {
        let trigger = sounds.trigger().name();
        let media = self.collect_media(trigger_ordinal, sounds);
        let several = media.len() > 1;
        let base = portable_stem(trigger, FALLBACK_STEM);
        for PlannedMedia {
            variant_ordinal,
            wem_id,
            selected,
        } in media
        {
            let Some(selected) = selected else {
                self.failures.push(FailedSound::new(
                    trigger,
                    Some(wem_id),
                    "no soundbank or loose WEM Entry holds the WEM".to_owned(),
                    ExportFailureOrigin {
                        trigger_ordinal,
                        variant_ordinal,
                        kind: ExportFailureKind::UnresolvedMedia,
                    },
                ));
                continue;
            };
            let stem = if several {
                format!("{base}_{wem_id}")
            } else {
                base.clone()
            };
            self.jobs.push(Job {
                trigger: trigger.to_owned(),
                stem: stems.unique(&stem),
                wem_id,
                source: selected.source,
                selection: ExportSoundSelection {
                    trigger_ordinal,
                    variant_ordinal,
                    media_variant_ordinal: selected.donor_ordinal.unwrap_or(variant_ordinal),
                },
            });
        }
    }

    fn collect_media(
        &mut self,
        trigger_ordinal: usize,
        sounds: &TriggerSounds<'_>,
    ) -> Vec<PlannedMedia> {
        let trigger = sounds.trigger().name();
        let resolved = ResolvedMedia::new(sounds.variants());
        let mut media = Vec::new();
        let mut seen = HashSet::new();
        for (variant_ordinal, variant) in sounds.variants().iter().enumerate() {
            let Some(wem_id) = variant.wem_id() else {
                self.failures.push(FailedSound::new(
                    trigger,
                    None,
                    format!(
                        "{} {} has no file to write",
                        variant.source_kind().label(),
                        variant.source().source_id
                    ),
                    ExportFailureOrigin {
                        trigger_ordinal,
                        variant_ordinal,
                        kind: ExportFailureKind::FilelessSource,
                    },
                ));
                continue;
            };
            let selected = resolved.select(variant, wem_id);
            if seen.insert((wem_id, selected.as_ref().map(|media| media.source))) {
                media.push(PlannedMedia {
                    variant_ordinal,
                    wem_id,
                    selected,
                });
            }
        }
        media
    }
}

struct PlannedMedia {
    variant_ordinal: usize,
    wem_id: u32,
    selected: Option<SelectedMedia>,
}

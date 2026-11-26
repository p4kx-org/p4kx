// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use super::name::{StemSet, portable_stem};
use super::source::{ResolvedMedia, WemExportSource};
use super::{ExportFailure, ExportSoundSelection};
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
    pub(super) failures: Vec<ExportFailure>,
}

pub(super) fn build(triggers: &[&TriggerSounds<'_>]) -> Plan {
    let mut plan = Plan::default();
    let mut stems = StemSet::default();
    for (trigger_ordinal, sounds) in triggers.iter().enumerate() {
        let trigger = sounds.trigger().name();
        let resolved = ResolvedMedia::new(sounds.variants());
        let mut media = Vec::new();
        let mut seen = HashSet::new();
        for (variant_ordinal, variant) in sounds.variants().iter().enumerate() {
            let Some(wem_id) = variant.wem_id() else {
                plan.failures.push(ExportFailure {
                    trigger: trigger.to_owned(),
                    wem_id: None,
                    reason: format!(
                        "{} {} has no file to write",
                        variant.source_kind().label(),
                        variant.source().source_id
                    ),
                });
                continue;
            };
            let selected = resolved.select(variant, wem_id);
            if seen.insert((wem_id, selected.as_ref().map(|media| media.source))) {
                media.push((variant_ordinal, wem_id, selected));
            }
        }
        let several = media.len() > 1;
        let base = portable_stem(trigger, FALLBACK_STEM);
        for (variant_ordinal, wem_id, selected) in media {
            let Some(selected) = selected else {
                plan.failures.push(ExportFailure {
                    trigger: trigger.to_owned(),
                    wem_id: Some(wem_id),
                    reason: "no soundbank or loose WEM Entry holds the WEM".to_owned(),
                });
                continue;
            };
            let stem = if several {
                format!("{base}_{wem_id}")
            } else {
                base.clone()
            };
            plan.jobs.push(Job {
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
    plan
}

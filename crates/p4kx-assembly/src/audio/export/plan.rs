// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use super::ExportFailure;
use super::name::{StemSet, portable_stem};
use super::source::{ResolvedMedia, WemExportSource};
use crate::audio::TriggerSounds;

const FALLBACK_STEM: &str = "sound";

#[derive(Debug)]
pub(super) struct Job {
    pub(super) trigger: String,
    pub(super) stem: String,
    pub(super) wem_id: u32,
    pub(super) source: WemExportSource,
}

#[derive(Debug, Default)]
pub(super) struct Plan {
    pub(super) jobs: Vec<Job>,
    pub(super) failures: Vec<ExportFailure>,
}

pub(super) fn build(triggers: &[&TriggerSounds<'_>]) -> Plan {
    let mut plan = Plan::default();
    let mut stems = StemSet::default();
    for sounds in triggers {
        let trigger = sounds.trigger().name();
        let resolved = ResolvedMedia::new(sounds.variants());
        let mut media = Vec::new();
        let mut seen = HashSet::new();
        for variant in sounds.variants() {
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
            let identity = (wem_id, resolved.source(variant, wem_id));
            if seen.insert(identity) {
                media.push(identity);
            }
        }
        let several = media.len() > 1;
        let base = portable_stem(trigger, FALLBACK_STEM);
        for (wem_id, source) in media {
            let Some(source) = source else {
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
                source,
            });
        }
    }
    plan
}

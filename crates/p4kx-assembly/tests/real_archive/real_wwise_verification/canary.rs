// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::Entities;
use p4kx_assembly::audio::{ArchiveAudio, DEFAULT_LANGUAGE, EntityAudio, LanguageChoice};
use p4kx_testkit::media_fixture::{CaseOutcome, RunReport};

use crate::media_fixtures::{
    self, Case, Check, DockedVehicles, Family, Index, Requirements, SourceCheck,
};

#[derive(Default)]
struct Tally {
    triggers: usize,
    matched: usize,
    with_sounds: usize,
    variants: usize,
    with_media: usize,
    breaks: usize,
}

impl Tally {
    const fn add(&mut self, other: &Self) {
        self.triggers += other.triggers;
        self.matched += other.matched;
        self.with_sounds += other.with_sounds;
        self.variants += other.variants;
        self.with_media += other.with_media;
        self.breaks += other.breaks;
    }
}

pub(super) fn verify() -> media_fixtures::Result<()> {
    media_fixtures::with_source(|archive, forge, files| {
        let fixtures = Index::new(Arc::new(Entities::new(forge)), files)?;
        let audio = ArchiveAudio::new(archive);
        let index = audio.index();
        let choice = LanguageChoice::new(index.language(DEFAULT_LANGUAGE), None);
        let mut report = RunReport::default();
        let cases = Family::AUDIO.into_iter().enumerate();
        for (ordinal, family) in cases {
            let case = Case::from_family(
                "audio_source",
                family,
                Requirements {
                    geometry_lod: None,
                    item_size: None,
                    requires_interior: false,
                    requires_audio: true,
                    docked: DockedVehicles::Expand,
                },
            );
            report.run(format!("audio_{ordinal}"), || {
                let Some(selected) = fixtures.choose(archive, &case)? else {
                    return Ok(media_fixtures::absent());
                };
                media_fixtures::selected(&case, &selected);
                Ok(CaseOutcome::Completed(tally(
                    &audio,
                    &index.resolve(selected.capabilities.into_audio()),
                    &choice,
                )?))
            });
        }
        let mut total = Tally::default();
        for case in report.cases() {
            if let Ok(CaseOutcome::Completed(found)) = &case.outcome {
                total.add(found);
            }
        }
        measurements(&total);
        let coverage = media_fixtures::finish(&report);
        thresholds(&total)?;
        coverage
    })
}

fn tally(
    audio: &ArchiveAudio<'_>,
    entity: &EntityAudio<'_>,
    choice: &LanguageChoice,
) -> media_fixtures::Result<Tally> {
    let mut tally = Tally {
        triggers: entity.triggers().len(),
        ..Tally::default()
    };
    let matched = entity
        .triggers()
        .iter()
        .filter(|trigger| trigger.unmatched().is_none());
    for trigger in matched {
        tally.matched += 1;
        let sounds = audio
            .resolve_trigger_in(trigger.name(), choice)
            .ok_or(SourceCheck::SourceIdentity)?;
        tally.with_sounds += usize::from(!sounds.variants().is_empty());
        tally.variants += sounds.variants().len();
        tally.with_media += sounds
            .variants()
            .iter()
            .filter(|variant| super::located(variant))
            .count();
        tally.breaks += sounds.breaks().len();
    }
    Ok(tally)
}

fn thresholds(total: &Tally) -> media_fixtures::Result<()> {
    media_fixtures::require(total.triggers > 100, Check::TriggerCount)?;
    media_fixtures::require(
        total.matched * 100 >= total.triggers * 95,
        Check::TriggerMatchShare,
    )?;
    media_fixtures::require(
        total.with_sounds * 100 >= total.matched * 60,
        Check::TriggerSoundShare,
    )?;
    media_fixtures::require(total.variants > 1_000, Check::VariantCount)?;
    media_fixtures::require(
        total.with_media * 100 >= total.variants * 80,
        Check::MediaLocationShare,
    )
}

#[expect(
    clippy::print_stdout,
    reason = "only aggregate audio resolution counts are reported"
)]
fn measurements(tally: &Tally) {
    println!(
        "media audio totals: triggers={} matched={} with_sounds={} variants={} located={} broken_links={}",
        tally.triggers,
        tally.matched,
        tally.with_sounds,
        tally.variants,
        tally.with_media,
        tally.breaks
    );
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::Entities;
use p4kx_assembly::audio::{ArchiveAudio, DEFAULT_LANGUAGE, EntityAudio, LanguageChoice};
use p4kx_testkit::media_fixture::{CaseOutcome, RunReport};

use crate::media_fixtures::{
    self, Case, Check, DockedVehicles, Family, Index, Requirements, SourceCheck,
};

pub(super) fn verify() -> media_fixtures::Result<()> {
    media_fixtures::with_source(|archive, forge, files| {
        let fixtures = Index::new(Arc::new(Entities::new(forge)), files)?;
        let audio = ArchiveAudio::new(archive);
        let index = audio.index();
        let language = index
            .language(DEFAULT_LANGUAGE)
            .ok_or(SourceCheck::SourceIdentity)?;
        let choice = LanguageChoice::new(Some(language), None);
        let mut report = RunReport::default();
        let cases = Family::STREAMED.into_iter().enumerate();
        for (ordinal, family) in cases {
            let case = Case::from_family(
                "streamed_audio",
                family,
                Requirements {
                    geometry_lod: None,
                    item_size: None,
                    requires_interior: false,
                    requires_audio: true,
                    docked: DockedVehicles::Expand,
                },
            );
            report.run(format!("streamed_{ordinal}"), || {
                let Some(selected) = fixtures.choose(archive, &case)? else {
                    return Ok(media_fixtures::absent());
                };
                media_fixtures::selected(&case, &selected);
                let triggers = index.resolve(selected.capabilities.into_audio());
                Ok(CaseOutcome::Completed(tally(
                    &audio, &choice, &triggers, language,
                )?))
            });
        }
        let mut total = super::Tally::default();
        for case in report.cases() {
            if let Ok(CaseOutcome::Completed(found)) = &case.outcome {
                merge(&mut total, found);
            }
        }
        measurements(&total);
        let coverage = media_fixtures::finish(&report);
        media_fixtures::require(total.streamed > 100, Check::StreamedCount)?;
        media_fixtures::require(
            total.streamed_located == total.streamed,
            Check::StreamedLocation,
        )?;
        coverage
    })
}

fn tally(
    audio: &ArchiveAudio<'_>,
    choice: &LanguageChoice,
    triggers: &EntityAudio<'_>,
    language: &str,
) -> media_fixtures::Result<super::Tally> {
    let mut tally = super::Tally::default();
    let matched = triggers
        .triggers()
        .iter()
        .filter(|trigger| trigger.unmatched().is_none());
    for trigger in matched {
        let sounds = audio
            .resolve_trigger_in(trigger.name(), choice)
            .ok_or(SourceCheck::SourceIdentity)?;
        tally.add(&sounds, language);
    }
    Ok(tally)
}

const fn merge(total: &mut super::Tally, found: &super::Tally) {
    total.streamed += found.streamed;
    total.streamed_located += found.streamed_located;
    total.localised += found.localised;
    total.localised_located += found.localised_located;
    total.flagged += found.flagged;
    total.flagged_in_language += found.flagged_in_language;
    total.missing_localisation += found.missing_localisation;
}

#[expect(
    clippy::print_stdout,
    reason = "only aggregate media-location counts are reported"
)]
fn measurements(total: &super::Tally) {
    println!(
        "media location totals: streamed={} located={} localised={} localised_located={} flagged={} flagged_in_language={} missing_localisation={}",
        total.streamed,
        total.streamed_located,
        total.localised,
        total.localised_located,
        total.flagged,
        total.flagged_in_language,
        total.missing_localisation
    );
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_media::MediaRecord;

use super::{ArchiveWatch, Sample, TestResult, offset, require};

#[path = "audio_node_groups.rs"]
mod node_groups;
#[path = "audio_node_source.rs"]
mod source;

use self::node_groups::{Coverage, Visitor};

pub(super) fn verify_archive(path: &std::path::Path) -> TestResult {
    verify_selection(path, Selection::Broad)
}

pub(super) fn verify_inline_state(path: &std::path::Path) -> TestResult {
    verify_selection(path, Selection::InlineState)
}

#[derive(Clone, Copy)]
enum Selection {
    Broad,
    InlineState,
}

fn verify_selection(path: &std::path::Path, selection: Selection) -> TestResult {
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    let mut best: Option<(Sample, Visitor<'static>)> = None;
    let mut seen = Visitor::scan();
    for sample in super::sampling::samples(&archive, &watch, ".bnk") {
        let sample = sample?;
        let mut observed = Visitor::scan();
        source::scan(&sample.original, &mut observed)?;
        seen.include(&observed);
        if matches!(selection, Selection::InlineState) && !observed.has(Coverage::InlineState) {
            continue;
        }
        if best
            .as_ref()
            .is_none_or(|(_, current)| observed.score() > current.score())
        {
            best = Some((sample, observed));
        }
        if best.as_ref().is_some_and(|(_, observed)| {
            matches!(selection, Selection::InlineState) || observed.has_complete_node_groups()
        }) {
            break;
        }
    }
    let Some((sample, selected)) = best else {
        return match selection {
            Selection::Broad => Err("bounded root bank with original nodes".into()),
            Selection::InlineState => {
                report(&seen, &Visitor::scan());
                Ok(())
            }
        };
    };
    require(
        selected.score() > 0,
        "selected bank has source node evidence",
    )?;
    super::verify_sample(&archive, sample, &schema, verify_bank)?;
    watch.check()?;
    report(&seen, &selected);
    Ok(())
}

pub(super) fn verify_bank(input: &[u8], rows: &[MediaRecord]) -> TestResult {
    let mut visitor = Visitor::verify(input, rows)?;
    source::scan(input, &mut visitor)
}

#[expect(
    clippy::print_stderr,
    reason = "the opt-in test reports coverage without source values"
)]
fn report(seen: &Visitor<'_>, selected: &Visitor<'_>) {
    for (label, coverage) in [
        ("sound", Coverage::Sound),
        ("actor_mixer", Coverage::ActorMixer),
        ("container", Coverage::Container),
        ("priority_flags", Coverage::Priority),
        ("properties", Coverage::Properties),
        ("ranged_modifiers", Coverage::Ranged),
        ("fx", Coverage::Fx),
        ("positioning_paths", Coverage::Paths),
        ("routing", Coverage::Routing),
        ("advanced_settings", Coverage::Advanced),
        ("inline_state", Coverage::InlineState),
        ("rtpc", Coverage::Rtpc),
        ("standalone_state", Coverage::StandaloneState),
        ("typed_standalone_state", Coverage::TypedStandaloneState),
    ] {
        eprintln!(
            "{label}: probed={}, selected={}",
            seen.has(coverage),
            selected.has(coverage)
        );
    }
}

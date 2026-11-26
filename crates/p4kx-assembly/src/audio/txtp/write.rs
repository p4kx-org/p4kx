// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use p4kx_extraction::ConflictPolicy;

use crate::audio::export::ResolvedMedia;
use crate::audio::export::name::{StemSet, portable_stem};
use crate::audio::{
    ExportLoopOrigins, ExportReport, ExportedSound, SoundVariant, TriggerSounds, WemExportSource,
};

use super::origins::OriginInputs;
use super::render::{RenderedTxtp, SoundFile, comment_text, render};
use super::skip_source::EventFailure;
use super::{TxtpFile, TxtpReport, TxtpSkip, TxtpSkipSource, diagnostic, output, tree};

const EXTENSION: &str = "txtp";
const FALLBACK_STEM: &str = "event";

pub(super) struct Destination<'a> {
    pub(super) folder: &'a Path,
    pub(super) policy: ConflictPolicy,
    pub(super) origins: Option<output::OriginContext<'a>>,
}

pub(super) fn files(
    triggers: &[&TriggerSounds<'_>],
    exported: &ExportReport,
    destination: &Destination<'_>,
    mut on_skip: impl FnMut(TxtpSkipSource),
) -> TxtpReport {
    let sounds: HashMap<(&str, u32, WemExportSource), &ExportedSound> = exported
        .sounds
        .iter()
        .map(|sound| ((sound.trigger.as_str(), sound.wem_id, sound.source), sound))
        .collect();
    let loop_origins = destination
        .origins
        .as_ref()
        .and_then(|context| context.loop_origins);
    let mut report = TxtpReport::default();
    let mut stems = StemSet::default();
    for (trigger_ordinal, trigger) in triggers.iter().enumerate() {
        let name = trigger.trigger().name();
        let resolved = ResolvedMedia::new(trigger.variants());
        let mut next_variant = 0;
        for variants in trigger
            .variants()
            .chunk_by(|a, b| std::ptr::eq(a.event(), b.event()))
        {
            let Some(first) = variants.first() else {
                continue;
            };
            let event_variant_ordinal = next_variant;
            next_variant += variants.len();
            let event = first.event();
            let breaks = trigger
                .breaks()
                .iter()
                .filter(|broken| std::ptr::eq(broken.event(), event))
                .count();
            let files = files_of(name, variants, &resolved, &sounds, loop_origins);
            match write_event(
                name,
                event.name(),
                variants,
                &files,
                breaks,
                &mut stems,
                destination,
            ) {
                Ok(file) => report.files.push(file),
                Err(failure) => {
                    on_skip(TxtpSkipSource {
                        trigger_ordinal,
                        event_variant_ordinal,
                        failed_variant_ordinal: failure
                            .leaf
                            .map(|index| event_variant_ordinal + index),
                    });
                    report.skipped.push(TxtpSkip {
                        trigger: name.to_owned(),
                        event: comment_text(event.name()),
                        reason: failure.reason,
                    });
                }
            }
        }
    }
    report
}

fn publish_event(
    path: PathBuf,
    trigger: &str,
    event: &str,
    breaks: usize,
    rendered: RenderedTxtp<'_>,
    variants: &[SoundVariant<'_>],
    destination: &Destination<'_>,
) -> Result<TxtpFile, String> {
    let publication = if let Some(context) = &destination.origins {
        let diagnostics = diagnostic::collect(&rendered, variants);
        output::write_with_origins(
            &path,
            &rendered.text,
            destination.policy,
            context,
            &OriginInputs {
                diagnostics: &diagnostics,
                loops: &rendered.loops,
            },
        )?
    } else {
        output::write(&path, &rendered.text, destination.policy)?
    };
    let command_loops = if publication.verified_content {
        rendered.command_loops()
    } else {
        None
    };
    Ok(TxtpFile {
        trigger: trigger.to_owned(),
        event: event.to_owned(),
        path,
        size: publication.size,
        state: publication.state,
        breaks,
        approximations: publication
            .verified_content
            .then_some(rendered.approximations),
        command_loops,
        origins: publication.origins,
        origin_receipt: publication.origin_receipt,
    })
}

fn write_event(
    trigger: &str,
    event: &str,
    variants: &[SoundVariant<'_>],
    files: &[Option<SoundFile<'_>>],
    breaks: usize,
    stems: &mut StemSet,
    destination: &Destination<'_>,
) -> Result<TxtpFile, EventFailure> {
    let tree = tree::build(variants)
        .ok_or_else(|| EventFailure::unbound("the event has no object path"))?;
    let mut header = vec![format!("P4KX: event {event} of trigger {trigger}")];
    if breaks > 0 {
        header.push(format!(
            "incomplete: {breaks} broken links; the objects behind them are not played here"
        ));
    }
    let rendered = render(&tree, &header, files)?;
    let path = destination.folder.join(format!(
        "{}.{EXTENSION}",
        stems.unique(&portable_stem(event, FALLBACK_STEM))
    ));
    publish_event(
        path,
        trigger,
        event,
        breaks,
        rendered,
        variants,
        destination,
    )
    .map_err(EventFailure::unbound)
}

fn files_of<'a, 'archive>(
    trigger: &str,
    variants: &[SoundVariant<'archive>],
    resolved: &ResolvedMedia<'archive>,
    sounds: &HashMap<(&str, u32, WemExportSource), &'a ExportedSound>,
    loop_origins: Option<&'a ExportLoopOrigins>,
) -> Vec<Option<SoundFile<'a>>> {
    variants
        .iter()
        .map(|variant| {
            let id = variant.wem_id()?;
            let source = resolved.source(variant, id)?;
            let sound = sounds.get(&(trigger, id, source))?;
            let name = sound.path.file_name()?.to_str()?;
            Some(SoundFile {
                name,
                loop_points: sound.loop_points,
                loop_origin: loop_origins.and_then(|origins| origins.for_sound(sound)),
                media_origin: loop_origins.and_then(|origins| origins.media_for_sound(sound)),
                prefetch_only: sound.prefetch_only,
            })
        })
        .collect()
}

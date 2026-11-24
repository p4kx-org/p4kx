// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::io::{self, BufWriter, Write};
use std::process::ExitCode;

use anyhow::Result;
use p4kx_assembly::audio::{
    ArchiveAudio, AudioIndex, EventMatch, SearchPage, TriggerHit, TriggerUsers, Unmatched,
};
use p4kx_extraction::{DataForge, DataForgeEntry, Record};
use serde::Serialize;

use crate::cli::WwiseEventsArgs;
use crate::entity_audio::{IssueJson, issues_json, warn_open};
use crate::exit::PARTIAL_FAILURE;
use crate::json;
use crate::query_table::one_line;

#[path = "wwise_events_origins.rs"]
mod origins;

const CLASS_PREFIX: &str = "EntityClassDefinition.";

#[derive(Serialize)]
struct EventsJson<'a> {
    archive: String,
    search: Option<&'a str>,
    offset: usize,
    limit: usize,
    total: usize,
    next_offset: Option<usize>,
    entities_skipped: Option<&'a str>,
    unreadable_records: usize,
    triggers: Vec<HitJson<'a>>,
    issues: Vec<IssueJson<'a>>,
    #[serde(flatten)]
    origins: origins::DefinitionOrigins,
}

#[derive(Serialize)]
struct HitJson<'a> {
    name: &'a str,
    definition: Option<&'a str>,
    matched_trigger: bool,
    events: Vec<EventJson<'a>>,
    unmatched: Option<UnmatchedJson>,
    entities: Option<EntitiesJson<'a>>,
}

#[derive(Serialize)]
struct EventJson<'a> {
    name: &'a str,
    id: u32,
    localised: bool,
    matched: bool,
    soundbanks: Vec<SoundbankJson<'a>>,
}

#[derive(Serialize)]
struct SoundbankJson<'a> {
    path: &'a str,
    language: Option<&'a str>,
}

#[derive(Serialize)]
struct UnmatchedJson {
    kind: &'static str,
    message: String,
}

#[derive(Serialize)]
struct EntitiesJson<'a> {
    count: usize,
    listed: Vec<EntityJson<'a>>,
}

#[derive(Serialize)]
struct EntityJson<'a> {
    guid: String,
    record_path: &'a str,
    class_name: &'a str,
}

/// The Entities that name the triggers of a page: how many, and which ones to list.
struct Finder<'f> {
    forge: &'f DataForge<'f>,
    found: TriggerUsers,
    listed: usize,
}

/// Prints the audio triggers whose names or whose events' names match a search, a page at a time.
///
/// Exits with 1 when the Entities were asked for and could not be looked up.
pub(crate) fn run(args: &WwiseEventsArgs, as_json: bool) -> Result<ExitCode> {
    let archive = crate::key::open(&args.archive)?;
    let audio = ArchiveAudio::new(&archive);
    let index = audio.index();
    let limit = usize::try_from(args.limit)?;
    let offset = usize::try_from(args.offset)?;
    let page = index.search(args.search.as_deref().unwrap_or_default(), offset, limit);
    let mut failure = None;
    let entry = (!args.no_entities && !page.hits().is_empty())
        .then(|| DataForgeEntry::read(&archive))
        .and_then(|read| note(read, &mut failure));
    let forge = entry
        .as_ref()
        .and_then(|entry| note(entry.open(), &mut failure));
    let finder = forge.as_ref().map(|forge| Finder {
        forge,
        found: TriggerUsers::build(forge, &wanted_by(&page)),
        listed: usize::try_from(args.entities).unwrap_or(usize::MAX),
    });
    let unreadable = finder
        .as_ref()
        .map_or(0, |finder| finder.found.unreadable_records());
    if unreadable > 0 {
        let _ignored = writeln!(
            io::stderr().lock(),
            "warning: {unreadable} Entity Records cannot be read and name no trigger"
        );
    }
    let code = failure
        .as_ref()
        .map_or(ExitCode::SUCCESS, |_| ExitCode::from(PARTIAL_FAILURE));
    if as_json {
        json::print(&EventsJson {
            archive: json::path(archive.path()),
            search: args.search.as_deref(),
            offset,
            limit,
            total: page.total(),
            next_offset: page.next_offset(),
            entities_skipped: failure.as_deref(),
            unreadable_records: unreadable,
            triggers: page
                .hits()
                .iter()
                .map(|hit| hit_json(hit, finder.as_ref()))
                .collect(),
            issues: issues_json(index),
            origins: origins::capture(&archive, index, page.hits())?,
        })?;
        return Ok(code);
    }
    if !warn_open(index)? {
        return Ok(code);
    }
    match print_text(index, &page, finder.as_ref()) {
        Err(error) if error.kind() != io::ErrorKind::BrokenPipe => Err(error.into()),
        _ => Ok(code),
    }
}

fn note<T, E: std::fmt::Display>(result: Result<T, E>, failure: &mut Option<String>) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(error) => {
            let line = one_line(&format!("no Entities listed: {error}"));
            let _ignored = writeln!(io::stderr().lock(), "warning: {line}");
            failure.get_or_insert_with(|| error.to_string());
            None
        }
    }
}

fn wanted_by(page: &SearchPage<'_>) -> impl Fn(&str) -> bool + Sync + use<> {
    let wanted: HashSet<String> = page
        .hits()
        .iter()
        .map(|hit| hit.trigger().name().to_ascii_lowercase())
        .collect();
    move |name| wanted.contains(name)
}

fn hit_json<'a>(hit: &TriggerHit<'a>, users: Option<&Finder<'a>>) -> HitJson<'a> {
    HitJson {
        name: hit.trigger().name(),
        definition: hit.definition_path(),
        matched_trigger: hit.matched_by_trigger(),
        events: hit
            .events()
            .map(|(event, matched)| EventJson {
                name: event.event().name(),
                id: event.event().id(),
                localised: event.event().is_localised(),
                matched,
                soundbanks: event
                    .soundbanks()
                    .map(|bank| SoundbankJson {
                        path: bank.path(),
                        language: bank.language(),
                    })
                    .collect(),
            })
            .collect(),
        unmatched: hit.unmatched().map(|reason: Unmatched| UnmatchedJson {
            kind: reason.kind(),
            message: reason.to_string(),
        }),
        entities: users.map(|users| EntitiesJson {
            count: users.found.count(hit.trigger().name()),
            listed: users
                .found
                .entities(users.forge, hit.trigger().name())
                .take(users.listed)
                .map(|record| entity_json(&record))
                .collect(),
        }),
    }
}

fn entity_json<'a>(record: &Record<'a>) -> EntityJson<'a> {
    let name = record.name().unwrap_or_default();
    EntityJson {
        guid: record.guid().to_string(),
        record_path: record.record_path().unwrap_or_default(),
        class_name: name.strip_prefix(CLASS_PREFIX).unwrap_or(name),
    }
}

fn print_text(
    index: &AudioIndex,
    page: &SearchPage<'_>,
    users: Option<&Finder<'_>>,
) -> io::Result<()> {
    let mut out = BufWriter::new(io::stdout().lock());
    for hit in page.hits() {
        let header = format!(
            "{}  {}",
            hit.trigger().name(),
            hit.definition_path().unwrap_or_default()
        );
        writeln!(out, "{}", one_line(&header))?;
        for (event, _) in hit.events() {
            writeln!(out, "  {}", one_line(&event_text(&event)))?;
        }
        if let Some(reason) = hit.unmatched() {
            writeln!(out, "  unmatched: {reason}")?;
        }
        if let Some(users) = users {
            writeln!(out, "  {}", one_line(&entities_text(hit, users)))?;
        }
    }
    writeln!(out, "{}", summary(page, index))?;
    out.flush()
}

fn event_text(event: &EventMatch<'_>) -> String {
    let banks: Vec<String> = event
        .soundbanks()
        .map(|bank| {
            bank.language().map_or_else(
                || bank.file_name().to_owned(),
                |language| format!("{} ({language})", bank.file_name()),
            )
        })
        .collect();
    if banks.is_empty() {
        format!("{} in no soundbank", event.event().name())
    } else {
        format!("{} in {}", event.event().name(), banks.join(", "))
    }
}

fn entities_text(hit: &TriggerHit<'_>, users: &Finder<'_>) -> String {
    let name = hit.trigger().name();
    let count = users.found.count(name);
    let listed: Vec<String> = users
        .found
        .entities(users.forge, name)
        .take(users.listed)
        .map(|record| entity_json(&record).class_name.to_owned())
        .collect();
    let more = count.saturating_sub(listed.len());
    match (listed.is_empty(), more) {
        (true, 0) => "Entities (0)".to_owned(),
        (true, _) => format!("Entities ({count})"),
        (false, 0) => format!("Entities ({count}): {}", listed.join(", ")),
        (false, _) => format!("Entities ({count}): {}, and {more} more", listed.join(", ")),
    }
}

fn summary(page: &SearchPage<'_>, index: &AudioIndex) -> String {
    let total = page.total();
    let triggers = index.triggers().len();
    let shown = match (page.hits().first(), page.hits().last()) {
        (Some(_), Some(_)) => format!(
            "showing {}-{} of {total}",
            page.offset().saturating_add(1),
            page.offset().saturating_add(page.hits().len())
        ),
        _ => format!("showing none of {total}"),
    };
    let next = page.next_offset().map_or_else(String::new, |offset| {
        format!("; next page: --offset {offset}")
    });
    format!("{total} of {triggers} triggers match: {shown}{next}")
}

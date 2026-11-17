// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::process::ExitCode;

use anyhow::{Context as _, Result};
use p4kx_extraction::{
    Archive, AudioConversion, Entry, FieldOrigins, FieldPath, InputScope, SourceInput,
    SourceInventory, SourceLocation, SourceUse, SourceUseRole, SourceUses, TextureConversion,
};
use serde::Serialize;

use crate::cli::{SocpakAction, SocpakArgs, SocpakSource};
use crate::entry_listing_json::EntryRows;
use crate::extract::{self, Settings};
use crate::json;
use crate::{container_selection, exit, filter_pair, list, verify};

#[derive(Serialize)]
struct ListJson<'a> {
    archive: String,
    containers: Vec<String>,
    entries: EntryRows<'a>,
    #[serde(rename = "fieldOrigins")]
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

struct ListOrigins {
    names: Vec<String>,
    fields: FieldOrigins,
    uses: SourceUses,
    route: Vec<SourceUse>,
}

impl ListOrigins {
    fn new() -> Self {
        Self {
            names: Vec::new(),
            fields: FieldOrigins::new(),
            uses: SourceUses::with_compact_use_refs(),
            route: Vec::new(),
        }
    }

    fn retain(&mut self, parent: &Archive, entry: &Entry, has_child: bool) -> Result<()> {
        let original = parent
            .observed_entry(entry)
            .context("selected container is not an Entry of its owning Archive")?;
        let complete = original.ancestors().all(|ancestor| ancestor.is_some());
        let scope = self.uses.observe(
            SourceInput::ArchiveView(original),
            InputScope::ArchiveDirectoryMetadata,
        )?;
        let location = || SourceLocation::Metadata {
            field: "path".into(),
        };
        self.route
            .push(scope.at(location(), SourceUseRole::Payload)?);
        let origin = self.uses.origin(&self.route, complete);
        self.fields.insert(
            FieldPath::parse(&format!("/containers/{}", self.names.len()))?,
            origin,
        )?;
        let _ = self.route.pop();
        if has_child {
            self.route
                .push(scope.at(location(), SourceUseRole::Selector)?);
        }
        self.names.push(entry.path().to_owned());
        Ok(())
    }

    fn report<'archive>(
        mut self,
        archive: &'archive Archive,
        entries: &'archive [&'archive Entry],
    ) -> Result<ListJson<'archive>> {
        let chain = self
            .uses
            .origin_with_origins(self.fields.iter().map(|(_, origin)| origin), true)?;
        self.fields
            .insert(FieldPath::parse("/containers")?, chain)?;
        self.fields
            .insert(FieldPath::parse("/archive")?, self.uses.origin(&[], true))?;
        Ok(ListJson {
            archive: json::path(archive.path()),
            containers: self.names,
            entries: EntryRows::physical(archive, entries),
            field_origins: self.fields,
            provenance: self.uses.freeze()?,
        })
    }
}

/// Runs the selected `p4kx socpak` action.
pub(crate) fn run(args: &SocpakArgs, as_json: bool) -> Result<ExitCode> {
    match &args.action {
        SocpakAction::List(source) => {
            let mut listing = as_json.then(ListOrigins::new);
            let container = open(source, listing.as_mut())?;
            let entries: Vec<&Entry> = container.entries().iter().collect();
            if let Some(listing) = listing {
                json::print(&listing.report(&container, &entries)?)?;
            } else {
                list::print_table(&entries)?;
            }
            Ok(ExitCode::from(exit::SUCCESS))
        }
        SocpakAction::Verify {
            source,
            filter,
            exclude,
        } => {
            let container = open(source, None)?;
            let entries = filter_pair::verification(&container, filter.as_deref(), exclude)?;
            verify::execute(&container, &entries, as_json)
        }
        SocpakAction::Extract {
            source,
            filter,
            exclude,
            out,
            conflict,
            convert,
            verify,
        } => {
            let container = open(source, None)?;
            let entries = extracted(&container, filter.as_deref(), exclude, *convert)?;
            let settings = Settings {
                out: out.as_deref(),
                conflict: *conflict,
                convert: *convert,
                textures: TextureConversion {
                    merge: true,
                    png: Some(0),
                    keep_raw: false,
                },
                audio: AudioConversion::Ogg,
                verify: *verify,
                threads: None,
                cancel_after: None,
            };
            extract::execute(&container, &entries, &settings, as_json)
        }
    }
}

/// Returns the Entries of `container` that `p4kx socpak extract` writes: those the Filter `text`
/// matches, or all of them without a Filter, without those `exclude` matches; with `convert`,
/// notes the excluded parts of textures that stay. Exclusion Filters without a Filter act on all
/// Entries, the glob `*`.
fn extracted<'a>(
    container: &'a Archive,
    text: Option<&str>,
    exclude: &[String],
    convert: bool,
) -> Result<Vec<&'a Entry>> {
    match text {
        Some(text) => filter_pair::extracting(container, text, exclude, convert),
        None if exclude.is_empty() => Ok(container.entries().iter().collect()),
        None => filter_pair::extracting(container, "*", exclude, convert),
    }
}

/// Opens the Archive and each selected container in order.
fn open(source: &SocpakSource, mut listing: Option<&mut ListOrigins>) -> Result<Archive> {
    let mut current = crate::key::open(&source.archive)?;
    for (position, path) in source.containers.iter().enumerate() {
        let entry = container_selection::select(&current, path, position)?;
        if let Some(listing) = &mut listing {
            listing.retain(&current, entry, position + 1 < source.containers.len())?;
        }
        current = current.open_container(entry)?;
    }
    Ok(current)
}

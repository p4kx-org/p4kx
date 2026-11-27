// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::ops::Range;

use p4kx_assembly::{ClipArchiveSource, ClipSourceSelection, EntityClipRig, RigFile};
use p4kx_extraction::animation_headers::AnimationSourceHeader;
use p4kx_extraction::{InputScope, SourceUseRole};

use super::super::{BANK, BANK_MATCH, Bank, BankMatch, BankState, Payload, Scope};
use super::{Rows, positions, read_status};
use crate::animation::dba_metadata_lookup::{entry_range, name_key};
use crate::animation::entity_catalogue::Work;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::{MediaError, MediaWriter};

#[derive(Default)]
pub(super) struct Banks<'a> {
    by_source: HashMap<usize, BankNames<'a>>,
}

#[derive(Default)]
struct BankNames<'a> {
    names: HashMap<String, Vec<NamedClip<'a>>>,
    complete: bool,
}

struct NamedClip<'a> {
    authored: &'a str,
    name: &'a Range<u64>,
    clip: &'a AnimationSourceHeader,
}

pub(super) struct BankSelection<'a> {
    parameter: usize,
    selection: &'a ClipSourceSelection,
}

type BankLists<'a> = HashMap<usize, Vec<BankSelection<'a>>>;

pub(super) fn declarations<'a>(
    rig: &'a EntityClipRig,
    writer: &MediaWriter<'_>,
) -> Result<BankLists<'a>, MediaError> {
    let mut lists = BankLists::new();
    if let Some(parameters) = rig.rig.parameters.as_ref().and_then(RigFile::value) {
        for link in &rig.links {
            writer.checkpoint()?;
            if matches!(
                link.selection,
                ClipSourceSelection::Database { .. }
                    | ClipSourceSelection::DatabaseUnavailable { .. }
            ) {
                let parameter = parameters
                    .get(link.parameter)
                    .ok_or_else(crate::origin_error::foreign)?;
                lists
                    .entry(parameter.list)
                    .or_default()
                    .push(BankSelection {
                        parameter: link.parameter,
                        selection: &link.selection,
                    });
            }
        }
    }
    Ok(lists)
}

pub(super) struct Alias<'a> {
    rig: usize,
    parameter: usize,
    path: &'a str,
    proof: OriginContributions,
    input: &'a MediaOriginInput,
    declarations: &'a [p4kx_assembly::RigParameter],
}

impl<'a> Alias<'a> {
    pub(super) const fn new(
        [rig, parameter]: [usize; 2],
        path: &'a str,
        proof: OriginContributions,
        input: &'a MediaOriginInput,
        declarations: &'a [p4kx_assembly::RigParameter],
    ) -> Self {
        Self {
            rig,
            parameter,
            path,
            proof,
            input,
            declarations,
        }
    }
}

impl Rows<'_, '_> {
    pub(super) fn banks<'source>(
        &self,
        alias: &Alias<'_>,
        banks: &[BankSelection<'_>],
        sources: &'source [ClipArchiveSource],
        cache: &mut Banks<'source>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let key = alias.path.to_ascii_lowercase();
        for bank in banks {
            let work = Work {
                lookup: self.lookup,
                writer,
            };
            work.charge(1)?;
            let parameter = alias
                .declarations
                .get(bank.parameter)
                .ok_or_else(crate::origin_error::foreign)?;
            let declaration = positions(alias.input, &parameter.selectors, writer)?;
            let join = BankJoin {
                alias,
                parameter: bank.parameter,
                declaration,
            };
            match bank.selection {
                ClipSourceSelection::Database { source } => {
                    let original = sources
                        .get(*source)
                        .ok_or_else(crate::origin_error::foreign)?;
                    let names = cache.names(*source, original, &work)?;
                    self.write_selected_bank(&join, *source, original, names, &key, writer)?;
                }
                ClipSourceSelection::DatabaseUnavailable { reason } => {
                    let state = BankState::from_lookup(reason);
                    let mut proof = join.proof(writer)?;
                    if !matches!(state, BankState::InvalidPath) {
                        proof.include(
                            &crate::origin_lookup::directory(writer.origins.root.as_ref(), writer)?,
                            writer,
                        )?;
                    }
                    join.write_bank(state, None, 0, proof.finish(), writer)?;
                }
                _ => return Err(crate::origin_error::foreign()),
            }
        }
        Ok(())
    }

    fn write_selected_bank(
        &self,
        join: &BankJoin<'_>,
        source: usize,
        original: &ClipArchiveSource,
        names: Option<&BankNames<'_>>,
        key: &str,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let archive = self.lookup.index.archive;
        let entry = archive
            .entries()
            .get(original.archive_ordinal)
            .ok_or_else(crate::origin_error::foreign)?;
        let input = writer.origins.input(archive, entry)?;
        let mut selection = join.proof(writer)?;
        selection.add(
            &input,
            InputScope::ArchiveDirectoryMetadata,
            OriginSite::new("archive-entry", &[original.archive_ordinal as u64], "path"),
            SourceUseRole::Selector,
            writer,
        )?;
        let selection = selection.finish();
        let mut coverage = writer.capture_origins();
        coverage.include(&selection, writer)?;
        if original.file.source().is_some() {
            let range = original
                .file
                .value()
                .and_then(|headers| headers.database_metadata_range.clone())
                .unwrap_or_else(|| 0..entry.uncompressed_size());
            add_range(
                &mut coverage,
                &input,
                &range,
                "dbaMetadataInventory",
                writer,
            )?;
        }
        let state = match names {
            Some(names) if names.complete => BankState::Enumerated,
            Some(_) => BankState::Incomplete,
            None => BankState::from_read(read_status(&original.file)),
        };
        let matches: &[NamedClip<'_>] = names
            .and_then(|names| names.names.get(key))
            .map_or(&[], Vec::as_slice);
        join.write_bank(
            state,
            Some((source, &selection)),
            matches.len(),
            coverage.finish(),
            writer,
        )?;
        for matched in matches {
            Work {
                lookup: self.lookup,
                writer,
            }
            .charge(1)?;
            join.write_match(source, &input, &selection, matched, writer)?;
        }
        Ok(())
    }
}

struct BankJoin<'a> {
    alias: &'a Alias<'a>,
    parameter: usize,
    declaration: OriginContributions,
}

impl BankJoin<'_> {
    fn address(&self) -> Vec<u64> {
        vec![
            self.alias.rig as u64,
            self.alias.parameter as u64,
            self.parameter as u64,
        ]
    }

    fn proof(
        &self,
        writer: &MediaWriter<'_>,
    ) -> Result<crate::origin_capture::OriginCapture, MediaError> {
        let mut proof = writer.capture_origins();
        proof.include(&self.alias.proof, writer)?;
        proof.include(&self.declaration, writer)?;
        Ok(proof)
    }
}

impl<'a> Banks<'a> {
    fn names(
        &mut self,
        source: usize,
        original: &'a ClipArchiveSource,
        work: &Work<'_, '_, '_>,
    ) -> Result<Option<&BankNames<'a>>, MediaError> {
        let Some(headers) = original.file.value() else {
            return Ok(None);
        };
        let names = match self.by_source.entry(source) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let mut names = BankNames {
                    names: HashMap::new(),
                    complete: headers.database_metadata_complete,
                };
                for clip in &headers.clips {
                    work.charge(1)?;
                    let original_name = clip.database_name.as_ref().and_then(|name| {
                        name_key(&name.bytes).map(|(authored, key)| (key, authored, &name.range))
                    });
                    if let Some((key, authored, name)) = original_name {
                        names.names.entry(key).or_default().push(NamedClip {
                            authored,
                            name,
                            clip,
                        });
                    } else {
                        names.complete = false;
                    }
                }
                entry.insert(names)
            }
        };
        Ok(Some(names))
    }
}

impl BankJoin<'_> {
    fn write_bank(
        &self,
        state: BankState,
        selected: Option<(usize, &OriginContributions)>,
        observed_matches: usize,
        coverage: OriginContributions,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut scope = Scope::new(writer);
        scope.at("archive_path", self.alias.proof.clone(), writer)?;
        scope.at("database_parameter", self.declaration.clone(), writer)?;
        scope.at("state", coverage.clone(), writer)?;
        scope.at("observed_matches", coverage, writer)?;
        if let Some((_, selection)) = selected {
            scope.at("source", selection.clone(), writer)?;
        }
        scope.write(
            BANK,
            self.address(),
            &Payload::NamedDbaBank(Bank {
                archive_path: self.alias.path,
                database_parameter: self.parameter,
                source: selected.map(|(source, _)| source),
                state,
                observed_matches,
            }),
            &[],
            writer,
        )
    }

    fn write_match(
        &self,
        source: usize,
        input: &MediaOriginInput,
        selection: &OriginContributions,
        matched: &NamedClip<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let clip = matched.clip;
        let name = matched.name;
        let mut scope = Scope::new(writer);
        scope.at("database_parameter", self.declaration.clone(), writer)?;
        let mut match_proof = writer.capture_origins();
        match_proof.include(selection, writer)?;
        add_range(&mut match_proof, input, name, "databaseName", writer)?;
        scope.at("source", match_proof.finish(), writer)?;
        let mut stored_name = writer.capture_origins();
        add_range(&mut stored_name, input, name, "databaseName", writer)?;
        let stored_name = stored_name.finish();
        scope.at("authored_name", stored_name.clone(), writer)?;
        scope.at("name_range", stored_name, writer)?;
        let mut metadata = writer.capture_origins();
        add_range(
            &mut metadata,
            input,
            &clip.metadata_range,
            "clipMetadata",
            writer,
        )?;
        let metadata = metadata.finish();
        scope.at("clip_ordinal", metadata.clone(), writer)?;
        scope.at("metadata_range", metadata, writer)?;
        let mut address = self.address();
        address.push(clip.ordinal as u64);
        scope.write(
            BANK_MATCH,
            address,
            &Payload::NamedDbaMatch(BankMatch {
                database_parameter: self.parameter,
                source,
                clip_ordinal: clip.ordinal,
                authored_name: matched.authored,
                name_range: entry_range(name.clone()),
                metadata_range: entry_range(clip.metadata_range.clone()),
            }),
            &[],
            writer,
        )
    }
}

fn add_range(
    proof: &mut crate::origin_capture::OriginCapture,
    input: &MediaOriginInput,
    range: &Range<u64>,
    field: &str,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    proof.add(
        input,
        InputScope::Payload,
        OriginSite::new("animation-byte-range", &[range.start, range.end], field),
        SourceUseRole::Selector,
        writer,
    )
}

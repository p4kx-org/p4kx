// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, EntryOrigin, FieldOrigin, FieldOrigins, InputScope, SourceInput, SourceInventory,
    SourceScope, SourceUse, SourceUseRole, SourceUses,
};

use crate::audio_wem_input::{Candidate, LanguageInput, Source};

use super::super::payload_origins::{DidxWord, EvidenceError, metadata, put};
use super::Selection;

const ROOT: &str = "/structuredContent";
const WEM: &str = "/structuredContent/wem";
const DESCRIBED: &str = "/structuredContent/described";

type Result<T> = std::result::Result<T, EvidenceError>;

pub(super) fn capture(
    archive: &Archive,
    selection: &Selection<'_>,
    rows: &[Candidate],
    from_path: bool,
    has_next: bool,
) -> Result<(FieldOrigins, SourceInventory)> {
    let evidence = Evidence {
        uses: SourceUses::with_compact_use_refs(),
        archive,
    };
    let mut fields = FieldOrigins::new();
    let described = evidence.described(selection, &mut fields)?;
    let from_request = evidence.uses.origin(&[], true);
    put(&mut fields, ROOT, "archive", from_request.clone())?;
    let id = if from_path {
        described.path.clone()
    } else {
        from_request
    };
    put(&mut fields, ROOT, "wem_id", id.clone())?;
    put(&mut fields, WEM, "id", id)?;
    put(&mut fields, WEM, "size", described.len)?;
    evidence.locations(rows, from_path, has_next, described.path, &mut fields)?;
    Ok((fields, evidence.uses.freeze()?))
}

struct Evidence<'archive> {
    uses: SourceUses,
    archive: &'archive Archive,
}

struct Identity {
    path: FieldOrigin,
    len: FieldOrigin,
    location: FieldOrigin,
}

struct OriginScope {
    scope: SourceScope,
    complete: bool,
}

impl Evidence<'_> {
    fn described(&self, selection: &Selection<'_>, fields: &mut FieldOrigins) -> Result<Identity> {
        let description = &selection.description;
        let origin = description.owner.as_ref();
        let binding = origin.map_or(SourceInput::Unknown, SourceInput::Archive);
        let complete = origin.is_some_and(|source| source.ancestors().all(|step| step.is_some()));
        let payload = self.uses.observe(binding, InputScope::Payload)?;
        description
            .payload
            .record(&self.uses, &payload, complete, WEM, fields)?;
        let summary = description
            .payload
            .summary_origin(&self.uses, &payload, complete)?;
        put(fields, ROOT, "wem", summary)?;
        let triplet = origin.zip(description.payload.triplet());
        let location = self.location(selection.candidate, triplet, DESCRIBED, fields)?;
        let origin = if selection.choice_incomplete {
            self.uses.origin(&[], false)
        } else {
            location.location.clone()
        };
        put(fields, ROOT, "described", origin)?;
        Ok(location)
    }

    fn locations(
        &self,
        rows: &[Candidate],
        from_path: bool,
        has_next: bool,
        path: FieldOrigin,
        fields: &mut FieldOrigins,
    ) -> Result<()> {
        let list = if from_path {
            path
        } else {
            self.uses.origin(&[], false)
        };
        put(fields, ROOT, "locations", list.clone())?;
        put(fields, ROOT, "locations_total", list.clone())?;
        if has_next {
            put(fields, ROOT, "next_cursor", list)?;
        }
        for (index, candidate) in rows.iter().enumerate() {
            let parent = format!("{ROOT}/locations/{index}");
            let identity = self.location(candidate, None, &parent, fields)?;
            put(
                fields,
                "/structuredContent/locations",
                &index.to_string(),
                identity.location,
            )?;
        }
        Ok(())
    }

    fn location(
        &self,
        candidate: &Candidate,
        read_triplet: Option<(&EntryOrigin, u64)>,
        parent: &str,
        fields: &mut FieldOrigins,
    ) -> Result<Identity> {
        let owner = self.candidate_scope(candidate)?;
        let path = metadata(&owner.scope, "path", SourceUseRole::Payload)?;
        let language = self.language(candidate, &path, owner.complete)?;
        let path_origin = self.origin(&owner, &path);
        put(fields, parent, "path", path_origin.clone())?;
        put(fields, parent, "language", language)?;
        let (kind, length) = match candidate.source {
            Source::Entry(_) => {
                let length = metadata(&owner.scope, "uncompressed_size", SourceUseRole::Payload)?;
                (path_origin.clone(), self.origin(&owner, &length))
            }
            Source::Embedded { triplet_offset, .. } => {
                self.embedded_location(&owner, read_triplet, triplet_offset, parent, fields)?
            }
        };
        put(fields, parent, "kind", kind.clone())?;
        put(fields, parent, "len", length.clone())?;
        Ok(Identity {
            path: path_origin,
            len: length,
            location: kind,
        })
    }

    fn embedded_location(
        &self,
        index: &OriginScope,
        read: Option<(&EntryOrigin, u64)>,
        indexed_offset: u64,
        parent: &str,
        fields: &mut FieldOrigins,
    ) -> Result<(FieldOrigin, FieldOrigin)> {
        let actual_owner = read
            .map(|(origin, _)| {
                Ok::<_, EvidenceError>(OriginScope {
                    scope: self
                        .uses
                        .observe(SourceInput::Archive(origin), InputScope::Payload)?,
                    complete: origin.ancestors().all(|step| step.is_some()),
                })
            })
            .transpose()?;
        let owner = actual_owner.as_ref().unwrap_or(index);
        let triplet = read.map_or(indexed_offset, |(_, offset)| offset);
        let didx = |word: DidxWord, role| {
            word.bind(&owner.scope, triplet, role)
                .map(|used| self.origin(owner, &used))
        };
        let offset = didx(DidxWord::Offset, SourceUseRole::Payload)?;
        put(fields, parent, "offset", offset)?;
        Ok((
            didx(DidxWord::Id, SourceUseRole::Selector)?,
            didx(DidxWord::Length, SourceUseRole::Payload)?,
        ))
    }

    fn candidate_scope(&self, candidate: &Candidate) -> Result<OriginScope> {
        let position = match candidate.source {
            Source::Entry(position) | Source::Embedded { position, .. } => position,
        };
        let entry = self
            .archive
            .entries()
            .get(position)
            .and_then(|entry| self.archive.observed_entry(entry));
        Ok(OriginScope {
            scope: self.uses.observe(
                entry.map_or(SourceInput::Unknown, SourceInput::ArchiveView),
                InputScope::Payload,
            )?,
            complete: entry.is_some_and(|entry| entry.ancestors().all(|step| step.is_some())),
        })
    }

    fn language(
        &self,
        candidate: &Candidate,
        path: &SourceUse,
        complete: bool,
    ) -> Result<FieldOrigin> {
        let used = std::slice::from_ref(path);
        match candidate.observed_language(self.archive) {
            LanguageInput::SelectedPath => Ok(self.uses.origin(used, complete)),
            LanguageInput::IncompleteIndex => Ok(self.uses.origin(used, false)),
            LanguageInput::IndexEntry(entry) => {
                let scope = self.uses.observe(
                    SourceInput::ArchiveView(entry),
                    InputScope::ArchiveDirectoryMetadata,
                )?;
                let spelling = metadata(&scope, "path", SourceUseRole::Selector)?;
                let complete = complete && entry.ancestors().all(|step| step.is_some());
                Ok(self.uses.origin(&[path.clone(), spelling], complete))
            }
        }
    }

    fn origin(&self, source: &OriginScope, used: &SourceUse) -> FieldOrigin {
        self.uses
            .origin(std::slice::from_ref(used), source.complete)
    }
}

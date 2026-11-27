// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;
use std::ptr;

use p4kx_assembly::audio::{AtlTrigger, AudioIndex, EventMatch, SoundbankEventSource};
use p4kx_extraction::{
    Archive, FieldOrigin, InputScope, ObservedEntry, SourceInput, SourceLocation, SourceScope,
    SourceUse, SourceUseRole, SourceUses,
};

use crate::audio_shapes::EventShape;
use crate::error::ToolError;
use crate::forge_origins::{EvidenceError, ForgeOrigins, site};

type Result<T> = std::result::Result<T, EvidenceError>;

pub(crate) struct AudioEventOrigins<'a> {
    archive: &'a Archive,
    index: &'a AudioIndex,
    uses: SourceUses,
    bank_complete: bool,
    bank_survey: Option<&'a FieldOrigin>,
}

impl<'a> AudioEventOrigins<'a> {
    pub(crate) const fn new(
        archive: &'a Archive,
        index: &'a AudioIndex,
        uses: SourceUses,
        bank_complete: bool,
        bank_survey: Option<&'a FieldOrigin>,
    ) -> Self {
        Self {
            archive,
            index,
            uses,
            bank_complete,
            bank_survey,
        }
    }

    pub(crate) fn write(
        &self,
        output: &mut ForgeOrigins,
        path: &str,
        definition: &AtlTrigger,
        event: EventMatch<'_>,
        row: &EventShape,
        lookup: &FieldOrigin,
    ) -> std::result::Result<FieldOrigin, ToolError> {
        let declaration = self.declaration(definition, event);
        let location = SourceLocation::xml_attribute(event.event().name_position(), "wwise_name");
        let original_name =
            self.attribute(declaration, location.clone(), SourceUseRole::Payload)?;
        let original_id =
            self.attribute(declaration, location.clone(), SourceUseRole::Conversion)?;
        let original_flag = self.localised(declaration, event)?;
        let name = self.union([lookup, &original_name], true)?;
        let id = self.union([lookup, &original_id], true)?;
        let localised = self.union([lookup, &original_flag], true)?;
        let banks = self.banks(output, path, declaration, event, row.soundbanks.len(), &id)?;
        let fields = [
            ("name", name),
            ("id", id),
            ("localised", localised),
            ("soundbank_count", banks.count),
            ("soundbanks", banks.listed),
        ];
        let matched = row
            .matched
            .map(|_| {
                let authored = self.attribute(declaration, location, SourceUseRole::Conversion)?;
                self.union([lookup, &authored], true)
            })
            .transpose()?;
        let row = self.union(
            fields.iter().map(|(_, origin)| origin).chain(&matched),
            true,
        )?;
        if let Some(matched) = matched {
            output.put(&format!("{path}/matched"), matched)?;
        }
        for (field, origin) in fields {
            output.put(&format!("{path}/{field}"), origin)?;
        }
        output.put(path, row.clone())?;
        Ok(row)
    }

    pub(crate) fn matched_origin(
        &self,
        definition: &AtlTrigger,
        event: EventMatch<'_>,
    ) -> std::result::Result<FieldOrigin, ToolError> {
        let declaration = self.declaration(definition, event);
        let location = SourceLocation::xml_attribute(event.event().name_position(), "wwise_name");
        let id = self.attribute(declaration, location, SourceUseRole::Conversion)?;
        if !event.is_matched() {
            return Ok(self.absence(&id)?);
        }
        let witness = declaration.and_then(|_| {
            let bank = event.soundbanks().next()?;
            self.index
                .event_soundbank_sources(self.archive, event.event().id())
                .next()
                .filter(|source| ptr::eq(source.soundbank(), bank))
        });
        let Some(witness) = witness else {
            return Ok(self.union([&id], false)?);
        };
        let proof = self.hirc(witness, std::iter::empty())?;
        Ok(self.union([&id, &proof], true)?)
    }

    fn declaration(
        &self,
        definition: &AtlTrigger,
        event: EventMatch<'_>,
    ) -> Option<ObservedEntry<'a>> {
        let events = definition.events();
        let original = event.event();
        let offset = ptr::from_ref(original)
            .addr()
            .checked_sub(events.as_ptr().addr())?;
        let selected = events.get(offset / size_of_val(original))?;
        if !ptr::eq(selected, original) {
            return None;
        }
        self.index.definition_source(self.archive, definition)
    }

    fn attribute(
        &self,
        declaration: Option<ObservedEntry<'a>>,
        location: Option<SourceLocation>,
        role: SourceUseRole,
    ) -> Result<FieldOrigin> {
        let Some((entry, location)) = declaration.zip(location) else {
            return Ok(self.uses.origin(&[], false));
        };
        let scope = self
            .uses
            .observe(SourceInput::ArchiveView(entry), InputScope::Payload)?;
        let site = scope.at(location, role)?;
        Ok(self.uses.origin(&[site], complete(entry)))
    }

    fn localised(
        &self,
        declaration: Option<ObservedEntry<'a>>,
        event: EventMatch<'_>,
    ) -> Result<FieldOrigin> {
        if let Some(position) = event.event().localised_position() {
            return self.attribute(
                declaration,
                SourceLocation::xml_attribute(position, "wwise_localised"),
                SourceUseRole::Conversion,
            );
        }
        let element = SourceLocation::xml_element(event.event().position(), "WwiseEvent");
        self.attribute(declaration, element, SourceUseRole::Conversion)
    }

    fn banks(
        &self,
        output: &mut ForgeOrigins,
        path: &str,
        declaration: Option<ObservedEntry<'a>>,
        event: EventMatch<'_>,
        listed_banks: usize,
        id: &FieldOrigin,
    ) -> Result<BankOrigins> {
        let mut occurrences = self
            .index
            .event_soundbank_sources(self.archive, event.event().id())
            .take_while(|_| declaration.is_some())
            .peekable();
        let mut holders = Vec::new();
        let mut listed = Vec::new();
        for (position, bank) in event.soundbanks().enumerate() {
            let source = occurrences.next_if(|source| ptr::eq(source.soundbank(), bank));
            let holder = match source {
                Some(source) => self.hirc(
                    source,
                    std::iter::from_fn(|| {
                        occurrences.next_if(|source| ptr::eq(source.soundbank(), bank))
                    }),
                )?,
                None => self.uses.origin(&[], false),
            };
            if position < listed_banks {
                let row = self.bank_row(
                    output,
                    &format!("{path}/soundbanks/{position}"),
                    source,
                    &holder,
                    id,
                )?;
                listed.push(row);
            }
            holders.push(holder);
        }
        let count = if event.is_matched() {
            let complete = !holders.is_empty() && occurrences.peek().is_none();
            self.union(holders.iter().chain([id]), complete && self.bank_complete)?
        } else {
            self.absence(id)?
        };
        let listed = if event.is_matched() {
            self.union(listed.iter().chain([id]), listed.len() == listed_banks)?
        } else {
            count.clone()
        };
        Ok(BankOrigins { count, listed })
    }

    fn bank_row(
        &self,
        output: &mut ForgeOrigins,
        path: &str,
        source: Option<SoundbankEventSource<'_, 'a>>,
        holder: &FieldOrigin,
        id: &FieldOrigin,
    ) -> Result<FieldOrigin> {
        let directory_path = self.bank_path(source, SourceUseRole::Payload)?;
        let directory_language = self.bank_path(source, SourceUseRole::Conversion)?;
        let path_origin = self.union([&directory_path, holder, id], true)?;
        let language = self.union([&directory_language, holder, id], true)?;
        let row = self.union([&path_origin, &language], true)?;
        output.put(&format!("{path}/path"), path_origin)?;
        output.put(&format!("{path}/language"), language)?;
        output.put(path, row.clone())?;
        Ok(row)
    }

    fn bank_path(
        &self,
        source: Option<SoundbankEventSource<'_, 'a>>,
        role: SourceUseRole,
    ) -> Result<FieldOrigin> {
        let Some(source) = source else {
            return Ok(self.uses.origin(&[], false));
        };
        let entry = source.entry();
        let directory = self.uses.observe(
            SourceInput::ArchiveView(entry),
            InputScope::ArchiveDirectoryMetadata,
        )?;
        let path = directory.at(
            SourceLocation::Metadata {
                field: "path".into(),
            },
            role,
        )?;
        Ok(self.uses.origin(&[path], complete(entry)))
    }

    fn hirc<'index>(
        &self,
        first: SoundbankEventSource<'index, 'a>,
        rest: impl Iterator<Item = SoundbankEventSource<'index, 'a>>,
    ) -> Result<FieldOrigin> {
        let entry = first.entry();
        let scope = self
            .uses
            .observe(SourceInput::ArchiveView(entry), InputScope::Payload)?;
        let version = first.version_bytes();
        let complete = version.is_some() && complete(entry);
        let mut sites = Vec::new();
        if let Some(bytes) = version {
            sites.push(word(&scope, "bnkBkhd", "version", bytes)?);
        }
        for source in std::iter::once(first).chain(rest) {
            sites.push(word(
                &scope,
                "bnkHircObject",
                "typeCode",
                source.kind_bytes(),
            )?);
            sites.push(word(&scope, "bnkHircObject", "id", source.id_bytes())?);
        }
        Ok(self.uses.origin(&sites, complete))
    }

    fn absence(&self, id: &FieldOrigin) -> Result<FieldOrigin> {
        self.union(
            std::iter::once(id).chain(self.bank_survey),
            self.bank_complete && self.bank_survey.is_some(),
        )
    }

    fn union<'origin>(
        &self,
        origins: impl IntoIterator<Item = &'origin FieldOrigin>,
        complete: bool,
    ) -> Result<FieldOrigin> {
        Ok(self.uses.origin_with_origins(origins, complete)?)
    }
}

struct BankOrigins {
    count: FieldOrigin,
    listed: FieldOrigin,
}

fn complete(entry: ObservedEntry<'_>) -> bool {
    entry.ancestors().all(|ancestor| ancestor.is_some())
}

fn word(
    scope: &SourceScope,
    namespace: &str,
    field: &str,
    bytes: Range<usize>,
) -> Result<SourceUse> {
    Ok(site(
        scope,
        namespace,
        field,
        vec![bytes.start as u64, bytes.end as u64],
        SourceUseRole::Selector,
    )?)
}

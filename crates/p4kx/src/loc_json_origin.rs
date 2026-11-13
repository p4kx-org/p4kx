// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::{Context, Result};
use p4kx_export::Language;
use p4kx_extraction::{
    Archive, Entry, EntryDirectoryOrigins, FieldOrigin, FieldOrigins, FieldPath, InputScope,
    LocalizationEntry, LocalizationSet, Lookup, SourceInput, SourceInventory, SourceLocation,
    SourceScope, SourceUse, SourceUseRole, SourceUses, TextSet,
};
use serde::Serialize;

use super::{GetJson, LanguagesJson};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Report<T> {
    #[serde(flatten)]
    data: T,
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

pub(super) fn languages<'entry>(
    archive: &Archive,
    listed: &[(Language, &'entry Entry)],
    data: LanguagesJson<'entry>,
) -> Result<Report<LanguagesJson<'entry>>> {
    let uses = SourceUses::with_compact_use_refs();
    let mut fields = FieldOrigins::new();
    put(&mut fields, "/archive", uses.origin(&[], true))?;
    for (index, (_, entry)) in listed.iter().enumerate() {
        let observed = EntryDirectoryOrigins::observe(archive, entry, uses.clone())?;
        for field in ["code", "folder", "entry"] {
            put(
                &mut fields,
                &format!("/languages/{index}/{field}"),
                observed.path_origin().clone(),
            )?;
        }
    }
    let consulted = uses.observe_root_directory_paths(archive)?;
    let complete = consulted.is_some();
    let inputs: Vec<_> = consulted.into_iter().collect();
    put(&mut fields, "/languages", uses.origin(&inputs, complete))?;
    finish(data, fields, &uses)
}

pub(super) fn get<'output>(
    set: &LocalizationSet,
    texts: &TextSet<'_>,
    requested: Language,
    fallback: Option<Language>,
    lookup: Lookup<'_>,
    data: GetJson<'output>,
) -> Result<Report<GetJson<'output>>> {
    let uses = SourceUses::with_compact_use_refs();
    let mut fields = FieldOrigins::new();
    for path in ["/key", "/language", "/fallback"] {
        put(&mut fields, path, uses.origin(&[], true))?;
    }
    let mut selection = Vec::new();
    let mut complete = true;
    let mut payload = Vec::new();
    if !matches!(lookup, Lookup::Placeholder) {
        for language in std::iter::once(requested).chain(fallback.filter(|l| *l != requested)) {
            let entry = set
                .entry(language)
                .context("consulted localization Entry")?;
            let scope = LocaleScope::new(&uses, entry)?;
            let stored = texts.text(data.key, language);
            let location = scope.location(data.key, stored)?;
            selection.push(scope.scope.at(location.clone(), SourceUseRole::Selector)?);
            complete &= scope.complete;
            if lookup.language() == Some(language) {
                payload = scope.text_uses(location, stored)?;
                break;
            }
        }
    }
    let selected = uses.origin(&selection, complete);
    put(&mut fields, "/status", selected.clone())?;
    put(&mut fields, "/text_language", selected)?;
    selection.append(&mut payload);
    put(&mut fields, "/text", uses.origin(&selection, complete))?;
    finish(data, fields, &uses)
}

struct LocaleScope<'entry> {
    entry: &'entry LocalizationEntry,
    scope: SourceScope,
    complete: bool,
}

impl<'entry> LocaleScope<'entry> {
    fn new(uses: &SourceUses, entry: &'entry LocalizationEntry) -> Result<Self> {
        let origin = entry.origin();
        let input = origin.map_or(SourceInput::Unknown, SourceInput::Archive);
        Ok(Self {
            entry,
            scope: uses.observe(input, InputScope::Payload)?,
            complete: origin
                .is_some_and(|entry| entry.ancestors().all(|ancestor| ancestor.is_some())),
        })
    }

    fn location(&self, key: &str, stored: Option<&str>) -> Result<SourceLocation> {
        let Some(value) = stored else {
            return Ok(SourceLocation::Subresource {
                namespace: "globalIniLookup".to_owned(),
                indices: Vec::new(),
                field: key.trim_prefix('@').to_ascii_lowercase(),
            });
        };
        let range = self
            .entry
            .text()
            .text()
            .as_bytes()
            .subslice_range(value.as_bytes())
            .context("resolved localization value does not belong to its original Entry")?;
        Ok(SourceLocation::Subresource {
            namespace: "globalIniDecodedText".to_owned(),
            indices: vec![u64::try_from(range.start)?, u64::try_from(range.end)?],
            field: "value".to_owned(),
        })
    }

    fn text_uses(&self, location: SourceLocation, stored: Option<&str>) -> Result<Vec<SourceUse>> {
        let value = stored
            .filter(|text| !text.is_empty())
            .context("winning localization value")?;
        let mut inputs: Vec<SourceUse> =
            vec![self.scope.at(location.clone(), SourceUseRole::Payload)?];
        if value.contains("\\n")
            || self.entry.text().is_lossy()
            || self.entry.text().encoding() != p4kx_extraction::IniEncoding::Utf8
        {
            inputs.push(self.scope.at(location, SourceUseRole::Conversion)?);
        }
        Ok(inputs)
    }
}

fn put(fields: &mut FieldOrigins, path: &str, origin: FieldOrigin) -> Result<()> {
    fields.insert(FieldPath::parse(path)?, origin)?;
    Ok(())
}

fn finish<T>(data: T, field_origins: FieldOrigins, uses: &SourceUses) -> Result<Report<T>> {
    Ok(Report {
        data,
        field_origins,
        provenance: uses.freeze()?,
    })
}

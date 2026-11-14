// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Write as _;
use std::io::{self, Write};
use std::path::Path;

use anyhow::Result;
use p4kx_assembly::character::{Chf, ChfJsonName, ChfNameKind, NameSummary, NameTable};
use p4kx_assembly::{
    ArchiveModels, ChfNameContributions, ForgeItems, LoadoutResolver, ObservedChfNames,
    archive_names, archive_names_observed,
};
use p4kx_extraction::{
    DataForgeEntry, DataForgeOrigin, InputScope, SourceInput, SourceScope, SourceUses,
};

/// Returns the names of the built-in table, extended with those the Archive at `archive` knows
/// for the hashes of `chf`.
pub(crate) fn names_for(archive: Option<&Path>, chf: &Chf) -> Result<NameTable> {
    names_for_all(archive, &[chf])
}

/// Returns the names of the built-in table, extended with those the Archive at `archive` knows
/// for the hashes of any of `chfs`; the Archive is opened once.
pub(crate) fn names_for_all(archive: Option<&Path>, chfs: &[&Chf]) -> Result<NameTable> {
    let Some(path) = archive else {
        return Ok(NameTable::builtin());
    };
    let archive = crate::key::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let models = ArchiveModels::new(&archive);
    let mut table = NameTable::builtin();
    for chf in chfs {
        table.merge(&archive_names(
            &chf.character,
            &mut ForgeItems::new(&mut resolver),
            &models,
        ));
    }
    Ok(table)
}

pub(crate) struct ObservedNames {
    names: NameTable,
    tables: Vec<ObservedChfNames>,
    forge: SourceScope,
    complete: bool,
}

impl ObservedNames {
    pub(crate) const fn table(&self) -> &NameTable {
        &self.names
    }

    pub(crate) fn contributions(
        &self,
        name: ChfJsonName,
        input: &SourceScope,
    ) -> Result<Option<ChfNameContributions>> {
        self.lookup(name.kind(), name.hash(), &[input])
    }

    pub(crate) fn lookup(
        &self,
        kind: ChfNameKind,
        hash: u32,
        inputs: &[&SourceScope],
    ) -> Result<Option<ChfNameContributions>> {
        let winner = self
            .tables
            .iter()
            .zip(inputs)
            .find(|(table, _)| table.names().get(kind, hash).is_some());
        let Some((table, input)) = winner else {
            return Ok(None);
        };
        let mut contributions = table.contributions(kind, hash, input, &self.forge)?;
        if let Some(contributions) = &mut contributions {
            contributions.complete &= self.complete;
        }
        Ok(contributions)
    }
}

pub(crate) fn observed(path: &Path, chf: &Chf, uses: &SourceUses) -> Result<ObservedNames> {
    observed_all(path, &[chf], uses)
}

pub(crate) fn observed_all(path: &Path, chfs: &[&Chf], uses: &SourceUses) -> Result<ObservedNames> {
    let archive = crate::key::open(path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let origin = match entry.origin() {
        DataForgeOrigin::Archive(origin) => SourceInput::Archive(origin),
        _ => SourceInput::Unknown,
    };
    let source = uses.observe(origin, InputScope::Payload)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let models = ArchiveModels::new(&archive);
    let mut names = NameTable::builtin();
    let mut tables = Vec::with_capacity(chfs.len());
    for chf in chfs {
        let table = archive_names_observed(
            &chf.character,
            &mut ForgeItems::new(&mut resolver),
            &models,
            uses,
        )?;
        names.merge(table.names());
        tables.push(table);
    }
    Ok(ObservedNames {
        names,
        tables,
        forge: source,
        complete: matches!(origin, SourceInput::Archive(_)),
    })
}

/// Returns the hashes as `0x` and eight hexadecimal digits each.
pub(crate) fn hex_list(hashes: &[u32]) -> Vec<String> {
    hashes.iter().map(|hash| format!("{hash:#010x}")).collect()
}

/// Writes how many hashes got a name to stderr, so that stdout stays the JSON.
pub(crate) fn note(summary: &NameSummary) -> io::Result<()> {
    writeln!(io::stderr().lock(), "{}", sentence(summary))
}

/// Returns the sentence that counts the named and the unresolved hashes.
pub(crate) fn sentence(summary: &NameSummary) -> String {
    let mut text = format!(
        "{} hashes named, {} unresolved",
        summary.named, summary.unresolved
    );
    if !summary.unresolved_hashes.is_empty() {
        let _infallible = write!(
            text,
            " ({} distinct: {})",
            summary.unresolved_hashes.len(),
            hex_list(&summary.unresolved_hashes).join(", ")
        );
    }
    text
}

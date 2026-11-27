// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{Archive, Entry};
use p4kx_dataforge::{DataForge, Record};
use thiserror::Error;

use crate::{
    CoverageReport, DataForgeEntry, DataForgeOrigin, EntryDirectoryOrigins,
    EntryDirectoryOriginsError, FieldOrigin, FieldOrigins, FieldOriginsError, FieldPath,
    FieldPathError, InputScope, RecordJsonLocation, SourceEvidenceError, SourceInput,
    SourceInventory, SourceLocation, SourceScope, SourceUseRole, SourceUses,
};

/// The Coverage report's original sources could not be registered.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CoverageOriginError {
    /// A physical source or use could not be registered.
    #[error(transparent)]
    Source(#[from] SourceEvidenceError),
    /// An Entry directory field could not be bound.
    #[error(transparent)]
    Directory(#[from] EntryDirectoryOriginsError),
    /// A field path could not be constructed.
    #[error(transparent)]
    Path(#[from] FieldPathError),
    /// A field path was bound more than once.
    #[error(transparent)]
    Fields(#[from] FieldOriginsError),
}

/// A Coverage report with original sources for its sample Record and failure Entry paths.
#[derive(Debug)]
pub struct ObservedCoverage {
    report: CoverageReport,
    fields: FieldOrigins,
    inventory: SourceInventory,
}

impl ObservedCoverage {
    pub(crate) fn new(
        report: CoverageReport,
        archive: &Archive,
        forge: &DataForge<'_>,
        input: &DataForgeEntry,
        sites: CoverageSites<'_, '_>,
    ) -> Result<Self, CoverageOriginError> {
        let uses = SourceUses::with_compact_use_refs();
        let mut fields = FieldOrigins::new();
        if !sites.samples.is_empty() {
            let input = match input.origin_of(forge) {
                Some(DataForgeOrigin::Archive(entry)) => SourceInput::Archive(entry),
                Some(DataForgeOrigin::File(file)) => SourceInput::File(file),
                _ => SourceInput::Unknown,
            };
            let complete = !matches!(input, SourceInput::Unknown);
            let scope = uses.observe(input, InputScope::Payload)?;
            for sample in sites.samples {
                let path = FieldPath::parse(&format!(
                    "/{}/{}/sample/record_path",
                    sample.section.field(),
                    sample.row
                ))?;
                let origin = sample_origin(sample.record, forge, &scope, &uses, complete)?;
                fields.insert(path, origin)?;
            }
        }

        for failure in sites.failures {
            let path = FieldPath::parse(&format!(
                "/entries/{}/failures/{}/path",
                failure.folder, failure.row
            ))?;
            let origin = EntryDirectoryOrigins::observe(archive, failure.entry, uses.clone())?;
            fields.insert(path, origin.path_origin().clone())?;
        }

        Ok(Self {
            report,
            fields,
            inventory: uses.freeze()?,
        })
    }

    /// Returns the Coverage report.
    #[must_use]
    pub const fn report(&self) -> &CoverageReport {
        &self.report
    }

    /// Returns the recorded per-field origins.
    #[must_use]
    pub const fn fields(&self) -> &FieldOrigins {
        &self.fields
    }

    /// Returns the original inputs and uses referenced by the field origins.
    #[must_use]
    pub const fn inventory(&self) -> &SourceInventory {
        &self.inventory
    }
}

#[derive(Default)]
pub(crate) struct CoverageSites<'archive, 'forge> {
    pub(crate) samples: Vec<SampleSite<'forge>>,
    pub(crate) failures: Vec<FailureSite<'archive>>,
}

pub(crate) struct FailureSite<'archive> {
    pub(crate) folder: usize,
    pub(crate) row: usize,
    pub(crate) entry: &'archive Entry,
}

pub(crate) struct SampleSite<'forge> {
    pub(crate) section: SampleSection,
    pub(crate) row: usize,
    pub(crate) record: Option<Record<'forge>>,
}

pub(crate) enum SampleSection {
    Types,
    StarMapTypes,
}

impl SampleSection {
    const fn field(&self) -> &'static str {
        match self {
            Self::Types => "types",
            Self::StarMapTypes => "starmap_types",
        }
    }
}

fn sample_origin(
    record: Option<Record<'_>>,
    forge: &DataForge<'_>,
    scope: &SourceScope,
    uses: &SourceUses,
    complete: bool,
) -> Result<FieldOrigin, SourceEvidenceError> {
    let Some(record) = record else {
        return Ok(uses.origin(&[], false));
    };
    let header = SourceLocation::DataForge {
        record_index: record.index(),
        field: "fileName".into(),
    };
    let Ok(file_name) = record.stored_file_name() else {
        scope.at(header, SourceUseRole::Attempted)?;
        return Ok(uses.origin(&[], false));
    };
    let header = scope.at(header, SourceUseRole::Conversion)?;
    let Some(text) = RecordJsonLocation::borrowed_text(forge, file_name) else {
        return Ok(uses.origin(&[header], false));
    };
    let text = scope.at(text, SourceUseRole::Conversion)?;
    Ok(uses.origin(&[header, text], complete))
}

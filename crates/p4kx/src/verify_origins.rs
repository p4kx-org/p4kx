// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::{Context as _, Result, bail};
use p4kx_extraction::{
    Archive, Checksum, Entry, EntryFailure, FieldOrigin, FieldOrigins, FieldPath, InputScope,
    ObservedVerification, SourceCoordinateOmission, SourceInput, SourceInventory, SourceLocation,
    SourceScope, SourceUse, SourceUseRole, SourceUses, VerificationObservation,
    VerificationOutcome, VerificationReport,
};
use serde::Serialize;

#[derive(Serialize)]
pub(super) struct VerificationOrigins {
    #[serde(rename = "fieldOrigins")]
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

impl VerificationOrigins {
    pub(super) fn capture(archive: &Archive, observed: &ObservedVerification<'_>) -> Result<Self> {
        let stable = !observed.report().archive_changed();
        let mut capture = Capture {
            uses: SourceUses::with_compact_use_refs(),
            fields: FieldOrigins::new(),
            stable,
            verified_complete: stable,
            verified: Vec::new(),
            mismatches: Vec::new(),
            failures: Vec::new(),
            cancelled: Vec::new(),
        };
        for observation in observed.observations() {
            capture.record(archive, observed.report(), observation)?;
        }
        capture.finish(archive)
    }
}

struct Capture {
    uses: SourceUses,
    fields: FieldOrigins,
    stable: bool,
    verified_complete: bool,
    verified: Vec<SourceUse>,
    mismatches: Vec<FieldOrigin>,
    failures: Vec<FieldOrigin>,
    cancelled: Vec<FieldOrigin>,
}

impl Capture {
    fn record(
        &mut self,
        archive: &Archive,
        report: &VerificationReport,
        observation: VerificationObservation<'_>,
    ) -> Result<()> {
        let entry = observation.entry();
        let original = archive
            .observed_entry(entry)
            .context("verification Entry must belong to the checked Archive")?;
        let complete = original.ancestors().all(|step| step.is_some());
        let directory = self.uses.observe(
            SourceInput::ArchiveView(original),
            InputScope::ArchiveDirectoryMetadata,
        )?;
        if observation.outcome() == VerificationOutcome::Verified {
            let domain = CheckDomain::of(entry.checksum())?;
            self.verified
                .extend(domain.contributions(entry, &directory)?);
            if entry.is_sha256_verified() {
                self.verified
                    .extend(CheckDomain::Stored.contributions(entry, &directory)?);
            }
            self.verified_complete &= complete;
            return Ok(());
        }

        let (used, complete) = path_uses(
            &directory,
            original.ancestors().len(),
            SourceUseRole::Selector,
            SourceUseRole::Payload,
        )?;
        let path = self.uses.origin(&used, complete);
        match observation.outcome() {
            VerificationOutcome::Failed { failure_index } => {
                let failure = report
                    .failures()
                    .get(failure_index)
                    .context("missing observed verification failure")?;
                self.record_failure(entry, &directory, path, failure)?;
            }
            VerificationOutcome::Cancelled => {
                self.put(
                    &format!("/cancelled/{}", self.cancelled.len()),
                    path.clone(),
                )?;
                self.cancelled.push(path);
            }
            _ => bail!("unsupported observed verification outcome"),
        }
        Ok(())
    }

    fn record_failure(
        &mut self,
        entry: &Entry,
        directory: &SourceScope,
        path: FieldOrigin,
        failure: &EntryFailure,
    ) -> Result<()> {
        if let Some((expected, _)) = failure.checksum_mismatch() {
            let row = self.mismatch(
                &format!("/mismatched/{}", self.mismatches.len()),
                path,
                entry,
                directory,
                expected,
            )?;
            self.mismatches.push(row);
        } else {
            let row = self.failure(&format!("/failed/{}", self.failures.len()), path, directory)?;
            self.failures.push(row);
        }
        Ok(())
    }

    fn finish(mut self, archive: &Archive) -> Result<VerificationOrigins> {
        let verified = self.uses.origin(&self.verified, self.verified_complete);
        self.put("/verified", verified)?;
        self.put("/archive", archive_location(archive, &self.uses)?)?;
        self.put("/archive_changed", self.uses.origin(&[], true))?;
        for (path, origins) in [
            ("/mismatched", self.mismatches.as_slice()),
            ("/failed", self.failures.as_slice()),
            ("/cancelled", self.cancelled.as_slice()),
        ] {
            self.fields.insert(
                FieldPath::parse(path)?,
                self.uses.origin_with_origins(origins, true)?,
            )?;
        }
        Ok(VerificationOrigins {
            field_origins: self.fields,
            provenance: self.uses.freeze()?,
        })
    }

    fn put(&mut self, path: &str, origin: FieldOrigin) -> Result<()> {
        self.fields.insert(FieldPath::parse(path)?, origin)?;
        Ok(())
    }

    fn mismatch(
        &mut self,
        row: &str,
        path: FieldOrigin,
        entry: &Entry,
        directory: &SourceScope,
        expected: Checksum,
    ) -> Result<FieldOrigin> {
        let complete = path.source_set_complete();
        let domain = CheckDomain::of(expected)?;
        let [expectation, actual] = domain.contributions(entry, directory)?;
        let expected = self.uses.origin(&[expectation], complete);
        let actual = self.uses.origin(&[actual], self.stable && complete);
        let selector = directory.at(metadata(domain.declaration()), SourceUseRole::Selector)?;
        let kind = self.uses.origin(&[selector], complete);
        let origin = self
            .uses
            .origin_with_origins([&path, &kind, &expected, &actual], true)?;
        for (member, origin) in [
            ("path", path),
            ("kind", kind),
            ("expected", expected),
            ("actual", actual),
        ] {
            self.put(&format!("{row}/{member}"), origin)?;
        }
        self.put(row, origin.clone())?;
        Ok(origin)
    }

    fn failure(
        &mut self,
        row: &str,
        path: FieldOrigin,
        directory: &SourceScope,
    ) -> Result<FieldOrigin> {
        let attempted = directory.for_scope(InputScope::Payload).at_opaque(
            SourceUseRole::Attempted,
            SourceCoordinateOmission::Unavailable,
        )?;
        let error = self.uses.origin(&[attempted], false);
        let origin = self.uses.origin_with_origins([&path, &error], true)?;
        self.put(&format!("{row}/path"), path)?;
        self.put(&format!("{row}/error"), error)?;
        self.put(row, origin.clone())?;
        Ok(origin)
    }
}

fn archive_location(archive: &Archive, uses: &SourceUses) -> Result<FieldOrigin> {
    let Some(container) = archive.container_input_origin() else {
        return Ok(uses.origin(&[], archive.root_directory_entry_count().is_some()));
    };
    let directory = uses.observe(
        SourceInput::Archive(&container),
        InputScope::ArchiveDirectoryMetadata,
    )?;
    let (names, complete) = path_uses(
        &directory,
        container.ancestors().len(),
        SourceUseRole::Conversion,
        SourceUseRole::Conversion,
    )?;
    Ok(uses.origin(&names, complete))
}

fn path_uses(
    directory: &SourceScope,
    ancestors: usize,
    ancestor_role: SourceUseRole,
    entry_role: SourceUseRole,
) -> Result<(Vec<SourceUse>, bool)> {
    let mut names = Vec::new();
    let mut complete = true;
    for index in 0..ancestors {
        if let Some(ancestor) = directory.ancestor(index)? {
            names.push(ancestor.at(metadata("path"), ancestor_role)?);
        } else {
            complete = false;
        }
    }
    names.push(directory.at(metadata("path"), entry_role)?);
    Ok((names, complete))
}

#[derive(Clone, Copy)]
enum CheckDomain {
    Decoded,
    Stored,
}

impl CheckDomain {
    fn of(checksum: Checksum) -> Result<Self> {
        match checksum {
            Checksum::Crc32(_) | Checksum::Crc32c(_) => Ok(Self::Decoded),
            Checksum::Sha256(_) => Ok(Self::Stored),
            _ => bail!("unsupported verification checksum"),
        }
    }

    const fn declaration(self) -> &'static str {
        match self {
            Self::Decoded => "checksum",
            Self::Stored => "sha256",
        }
    }

    fn contributions(self, entry: &Entry, directory: &SourceScope) -> Result<[SourceUse; 2]> {
        let (namespace, end) = match self {
            Self::Decoded => ("entryDecodedBytes", entry.uncompressed_size()),
            Self::Stored => ("entryStoredBytes", entry.compressed_size()),
        };
        let expected = directory.at(metadata(self.declaration()), SourceUseRole::Conversion)?;
        let actual = directory.for_scope(InputScope::Payload).at(
            SourceLocation::Subresource {
                namespace: namespace.into(),
                indices: vec![0, end],
                field: "checksum".into(),
            },
            SourceUseRole::Conversion,
        )?;
        Ok([expected, actual])
    }
}

fn metadata(field: &str) -> SourceLocation {
    SourceLocation::Metadata {
        field: field.into(),
    }
}

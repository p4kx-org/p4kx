// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::model::{
    MaterialError, MaterialLibrary, MaterialReadError, MaterialSourceLimits,
};
use p4kx_extraction::{Archive, ArchiveWatch, Entry, Filter};
use p4kx_media::{
    Algorithm, Artifact, ByteScope, ContainerIdentity, Digest, ExportOptions, Float32, Identity,
    Index, KindSummary, Limits, MediaKind, MediaRecord, Source, Status, Tally,
    reconstruct_material_library,
};
use serde_json::Value;

use super::TestResult;
use super::archive_replay::{Lines, generation};
use super::shader_accounting::require;

#[path = "material_accounting_fields.rs"]
mod fields;
#[path = "material_accounting_raw.rs"]
mod raw;

#[cfg(test)]
#[path = "material_accounting_fixtures.rs"]
mod fixtures;
use fields::Counts;

fn verify(archive: &Archive, limits: Limits) -> TestResult<Counts> {
    let watch = ArchiveWatch::new(archive);
    let directory = tempfile::tempdir()?;
    let options = ExportOptions {
        kinds: vec![MediaKind::Material],
        include: Some(Filter::parse("*.mtl")?),
        diagnostic: true,
        limits,
        ..ExportOptions::default()
    };
    let (index, base) = generation(
        directory.path(),
        p4kx_media::export(archive, directory.path(), &options)?,
    )?;
    watch.check()?;
    let counts = reconcile(archive, &watch, &base, &index)?;
    directory.close()?;
    watch.check()?;
    Ok(counts)
}

fn reconcile(
    archive: &Archive,
    watch: &ArchiveWatch,
    base: &Path,
    index: &Index,
) -> TestResult<Counts> {
    let [kind] = index.kinds.as_slice() else {
        return Err("one material provider summary required".into());
    };
    let schema = jsonschema::validator_for(&p4kx_media::material_record_schema()?)?;
    let mut census = Census {
        base,
        watch,
        root: &index.source.digest,
        schema: &schema,
        sources: Lines::open(&base.join(&index.source_manifest.path))?,
        records: Lines::open(&base.join(&kind.artifact.path))?,
        artifacts: Lines::open(&base.join(&index.artifact_manifest.path))?,
        counts: Counts::default(),
    };
    census.walk(archive, &mut Vec::new())?;
    require(
        census.sources.next::<Source>()?.is_none(),
        "unexpected source-manifest rows",
    )?;
    require(
        census.records.next::<Value>()?.is_none(),
        "unexpected material records",
    )?;
    require(
        census.artifacts.next::<Artifact>()?.is_none(),
        "unexpected material artifact receipt",
    )?;
    let counts = census.counts;
    require(
        index.enumeration_complete,
        "incomplete traversal cannot prove total material coverage",
    )?;
    counts.check_totals(&index.source_entries, kind)?;
    require(
        index.complete == (counts.partial == 0 && counts.failed == 0),
        "material generation completeness differs",
    )?;
    Ok(counts)
}

struct Census<'a> {
    base: &'a Path,
    watch: &'a ArchiveWatch,
    root: &'a Digest,
    schema: &'a jsonschema::Validator,
    sources: Lines,
    records: Lines,
    artifacts: Lines,
    counts: Counts,
}

impl Census<'_> {
    fn walk(&mut self, archive: &Archive, chain: &mut Vec<ContainerIdentity>) -> TestResult {
        for entry in archive.entries() {
            self.watch.poll()?;
            self.counts.enumerated += 1;
            let extension = entry
                .path()
                .rsplit_once('.')
                .map(|(_, extension)| extension);
            if extension.is_some_and(|value| value.eq_ignore_ascii_case("mtl")) {
                self.entry(archive, entry, chain)?;
            }
            if extension.is_some_and(|value| {
                value.eq_ignore_ascii_case("pak") || value.eq_ignore_ascii_case("socpak")
            }) {
                let nested = archive.open_container(entry)?;
                chain.push(ContainerIdentity::try_new_with_entry(archive, entry)?);
                self.walk(&nested, chain)?;
                let _parent = chain.pop();
            }
        }
        Ok(())
    }

    fn entry(
        &mut self,
        archive: &Archive,
        entry: &Entry,
        chain: &[ContainerIdentity],
    ) -> TestResult {
        let source: Source = self
            .sources
            .next()?
            .ok_or("missing selected material source")?;
        let identity = ContainerIdentity::try_new_with_entry(archive, entry)?;
        require(
            matches!(&source.identity, Identity::Entry { entry, containers }
                if *entry == identity && containers == chain),
            "complete original container and Entry identity",
        )?;
        require(
            source.root == *self.root,
            "source belongs to the verified Archive",
        )?;
        let row = self.library_rows(&source)?;
        let raw = source
            .raw
            .as_ref()
            .ok_or("original raw material artifact")?;
        let artifact: Artifact = self
            .artifacts
            .next()?
            .ok_or("missing material artifact receipt")?;
        fields::check_artifact(&artifact, &source, entry.uncompressed_size())?;
        let limits = MaterialSourceLimits::default();
        if let Some(bytes) = raw::check_payload(
            archive,
            self.watch,
            entry,
            self.base,
            raw,
            limits.source_bytes,
        )? {
            self.check_fields(entry, &row, &bytes, limits)?;
        } else {
            self.counts.account(
                &row,
                Err(&MaterialError::SourceBytes {
                    limit: limits.source_bytes,
                }),
            )?;
        }
        self.counts.selected += 1;
        self.counts.nested += u64::from(!chain.is_empty());
        Ok(())
    }

    fn library_rows(&mut self, source: &Source) -> Result<MediaRecord, Box<dyn std::error::Error>> {
        let mut rows = Vec::new();
        loop {
            self.watch.poll()?;
            let wire: Value = self
                .records
                .next()?
                .ok_or("missing material library record")?;
            require(
                self.schema.is_valid(&wire),
                "actual material record failed schema validation",
            )?;
            let row: MediaRecord = serde_json::from_value(wire)?;
            require(
                row.kind == MediaKind::Material,
                "actual material record kind",
            )?;
            fields::check_source(&row.source, source)?;
            let end = matches!(
                row.subresource.namespace.as_str(),
                "material-library" | "material-library-failure" | "material-library-end"
            );
            rows.push(row);
            self.counts.records += 1;
            if end {
                return Ok(reconstruct_material_library(&rows)?);
            }
        }
    }

    fn check_fields(
        &mut self,
        entry: &Entry,
        row: &MediaRecord,
        bytes: &[u8],
        limits: MaterialSourceLimits,
    ) -> TestResult {
        match MaterialLibrary::parse_with(entry.path(), bytes, limits, || self.watch.poll()) {
            Ok(library) => self.counts.account(row, Ok(&library)),
            Err(MaterialReadError::Material(error)) => self.counts.account(row, Err(&error)),
            Err(error) => Err(error.into()),
        }
    }
}

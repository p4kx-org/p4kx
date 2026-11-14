// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::model::{
    MaterialError, MaterialLibrary, MaterialReadError, MaterialSourceLimits,
};
use p4kx_extraction::{Archive, ArchiveWatch, Entry, Filter};
use p4kx_media::{
    Artifact, ContainerIdentity, Digest, ExportOptions, Identity, Index, Limits, MediaKind,
    MediaRecord, Source, Tally, reconstruct_material_library,
};
use serde_json::Value;

use super::archive_replay::{Lines, generation};
use super::shader_accounting::require;
use super::{TestResult, privacy};

#[path = "material_accounting_fields.rs"]
mod fields;
#[path = "material_accounting_raw.rs"]
mod raw;

#[cfg(test)]
#[path = "material_accounting_fixtures.rs"]
mod fixtures;

#[derive(Default)]
struct Counts {
    enumerated: u64,
    selected: u64,
    nested: u64,
    complete: u64,
    partial: u64,
    failed: u64,
    materials: usize,
    records: u64,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn all_selected_material_libraries_and_raw_artifacts_reconcile()
-> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify_configured())
}

fn verify_configured() -> TestResult {
    let primary = p4kx_testkit::real_archive_path()?;
    let secondary = p4kx_testkit::real_archive_b_path();
    let key = p4kx_testkit::real_archive_key()?;
    for (ordinal, path) in std::iter::once(primary).chain(secondary).enumerate() {
        let archive = Archive::open_with_key(path, Some(key.clone()))?;
        let counts = verify(&archive, Limits::default())?;
        report(ordinal + 1, &counts);
        require(counts.selected > 0, "selected material inventory is empty")?;
    }
    Ok(())
}

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
    require(
        kind.provider.kind == MediaKind::Material,
        "actual material provider",
    )?;
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
    require(
        counters_match(index, &counts),
        "material Entry or generation counts differ",
    )?;
    require(
        records_match(&kind.counts.source_entries, &counts),
        "material provider callback counts differ",
    )?;
    require(
        kind.counts.records.enumerated == counts.records
            && kind.counts.records.written == counts.records - counts.partial - counts.failed
            && kind.counts.records.partial == counts.partial
            && kind.counts.records.failed == counts.failed
            && kind.counts.records.skipped == 0,
        "material record counts differ",
    )?;
    require(
        kind.counts.converted_outputs.enumerated == 0,
        "material metadata creates no converted output",
    )?;
    Ok(counts)
}

const fn counters_match(index: &Index, counts: &Counts) -> bool {
    let tally = &index.source_entries;
    let selected = counts.selected;
    tally.enumerated == counts.enumerated
        && tally.written == selected
        && tally.skipped == counts.enumerated - selected
        && tally.failed == 0
        && tally.partial == 0
        && index.complete == (counts.partial == 0 && counts.failed == 0)
}

const fn records_match(tally: &Tally, counts: &Counts) -> bool {
    tally.enumerated == counts.selected
        && tally.written == counts.complete
        && tally.partial == counts.partial
        && tally.failed == counts.failed
        && tally.skipped == 0
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
        require(
            artifact.path == raw.path && artifact.size == raw.size && artifact.digest == raw.digest,
            "exact raw artifact receipt for this presented physical source",
        )?;
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
            self.check_failure(
                &MaterialError::SourceBytes {
                    limit: limits.source_bytes,
                },
                &row,
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
            check_row_source(&row, source)?;
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
            Ok(library) => {
                let complete = fields::library(&library, row)?;
                self.counts.materials += library.materials.len();
                self.counts.complete += u64::from(complete);
                self.counts.partial += u64::from(!complete);
            }
            Err(MaterialReadError::Material(error)) => self.check_failure(&error, row)?,
            Err(error) => return Err(error.into()),
        }
        Ok(())
    }

    fn check_failure(&mut self, error: &MaterialError, row: &MediaRecord) -> TestResult {
        fields::failure(error, row)?;
        self.counts.failed += 1;
        Ok(())
    }
}

fn check_row_source(row: &MediaRecord, source: &Source) -> TestResult {
    require(
        row.kind == MediaKind::Material,
        "actual material record kind",
    )?;
    require(
        row.source.identity == source.identity
            && row.source.raw == source.raw
            && row.source.root == source.root
            && row.source.size == source.size
            && row.source.declared_integrity == source.declared_integrity
            && row.source.verification == source.verification,
        "provider and manifest agree on the actual original input",
    )
}

#[expect(
    clippy::print_stderr,
    reason = "the opt-in test prints anonymous coverage counts"
)]
fn report(archive: usize, counts: &Counts) {
    eprintln!(
        "material-source-export archive={archive} entries={} libraries={} nested={} complete={} partial={} failed={} materials={}",
        counts.enumerated,
        counts.selected,
        counts.nested,
        counts.complete,
        counts.partial,
        counts.failed,
        counts.materials,
    );
}

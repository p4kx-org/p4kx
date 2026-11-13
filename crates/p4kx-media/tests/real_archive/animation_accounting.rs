// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch, Entry, Filter};
use p4kx_media::{
    ContainerIdentity, Digest, ExportOptions, ExportOutcome, Identity, Index, MediaKind,
    MediaRecord, Source,
};
use serde::de::DeserializeOwned;

use super::shader_accounting::require;
use super::{TestResult, manifests, privacy};

#[path = "animation_rows.rs"]
mod rows;

use self::rows::{AnimationRows, Totals};

#[derive(Clone, Copy)]
enum Family {
    Caf,
    Dba,
    Aim,
}

impl Family {
    const fn suffix(self) -> &'static str {
        match self {
            Self::Caf => "caf",
            Self::Dba => "dba",
            Self::Aim => "aim",
        }
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn all_selected_animation_entries_clips_and_keys_reconcile()
-> Result<(), privacy::VerificationFailure> {
    privacy::redact(verify_configured())
}

fn verify_configured() -> TestResult {
    verify_archive(&p4kx_testkit::real_archive_path()?)?;
    if let Some(second) = p4kx_testkit::real_archive_b_path() {
        verify_archive(&second)?;
    }
    Ok(())
}

fn verify_archive(path: &Path) -> TestResult {
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::real_archive_key()?))?;
    let mut complete = true;
    let mut any_clips = false;
    for family in [Family::Caf, Family::Dba, Family::Aim] {
        let totals = verify_family(&archive, family)?;
        report(family, &totals);
        any_clips |= totals.clips > 0;
        complete &= totals.complete && totals.partial == 0;
    }
    require(
        any_clips && complete,
        "selected animation inventory is empty or incomplete",
    )
}

fn verify_family(archive: &Archive, family: Family) -> TestResult<Totals> {
    let watch = ArchiveWatch::new(archive);
    let output = tempfile::tempdir()?;
    let options = ExportOptions {
        kinds: vec![MediaKind::Animation],
        include: Some(Filter::parse(&format!("*.{}", family.suffix()))?),
        diagnostic: true,
        ..ExportOptions::default()
    };
    let outcome = p4kx_media::export(archive, output.path(), &options)?;
    let (index, base) = generation(output.path(), outcome)?;
    watch.check()?;
    let totals = reconcile(archive, &watch, family, &base, &index)?;
    output.close()?;
    watch.check()?;
    Ok(totals)
}

fn generation(
    output: &Path,
    outcome: ExportOutcome,
) -> TestResult<(Box<Index>, std::path::PathBuf)> {
    match outcome {
        ExportOutcome::Published { index, .. } => {
            let base = output.join(&index.artifact_base);
            Ok((index, base))
        }
        ExportOutcome::Incomplete { index, diagnostic } => {
            let base = diagnostic
                .parent()
                .ok_or("diagnostic generation directory")?;
            Ok((index, base.to_path_buf()))
        }
        _ => Err("animation verification did not run an export".into()),
    }
}

fn reconcile(
    archive: &Archive,
    watch: &ArchiveWatch,
    family: Family,
    base: &Path,
    index: &Index,
) -> TestResult<Totals> {
    let kind = index
        .kinds
        .iter()
        .find(|kind| kind.provider.kind == MediaKind::Animation)
        .ok_or("animation kind index")?;
    let schema = jsonschema::validator_for(&p4kx_media::builtin_record_schema()?)?;
    let mut census = Census {
        base,
        family,
        watch,
        root: &index.source.digest,
        schema: &schema,
        sources: Lines::open(&base.join(&index.source_manifest.path))?,
        records: Lines::open(&base.join(&kind.artifact.path))?,
        totals: Totals {
            complete: index.complete && index.enumeration_complete,
            ..Totals::default()
        },
    };
    census.walk(archive, &mut Vec::new())?;
    require(
        census.sources.next::<Source>()?.is_none(),
        "complete physical source manifest",
    )?;
    require(
        census.records.next::<MediaRecord>()?.is_none(),
        "complete animation record stream",
    )?;
    require(
        census.totals.entries == index.source_entries.written,
        "index selected Entry tally",
    )?;
    Ok(census.totals)
}

struct Census<'a> {
    base: &'a Path,
    family: Family,
    watch: &'a ArchiveWatch,
    root: &'a Digest,
    schema: &'a jsonschema::Validator,
    sources: Lines,
    records: Lines,
    totals: Totals,
}

impl Census<'_> {
    fn walk(&mut self, archive: &Archive, chain: &mut Vec<ContainerIdentity>) -> TestResult {
        for entry in archive.entries() {
            self.watch.poll()?;
            let path = entry.path();
            let (has_suffix, extension) = path
                .rsplit_once('.')
                .map_or((false, path), |(_, suffix)| (true, suffix));
            if has_suffix && extension.eq_ignore_ascii_case(self.family.suffix()) {
                self.entry(archive, entry, chain)?;
            }
            if extension.eq_ignore_ascii_case("pak") || extension.eq_ignore_ascii_case("socpak") {
                let identity = ContainerIdentity::try_new_with_entry(archive, entry)?;
                let nested = archive.open_container(entry)?;
                chain.push(identity);
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
            .ok_or("missing selected animation source")?;
        let expected = ContainerIdentity::try_new_with_entry(archive, entry)?;
        require(
            matches!(&source.identity, Identity::Entry { entry, containers }
                if entry == &expected && containers == chain),
            "manifest retains the complete original container and Entry identity",
        )?;
        require(
            source.root == *self.root,
            "manifest belongs to the verified Archive",
        )?;
        let raw = source
            .raw
            .as_ref()
            .ok_or("selected animation has retained bytes")?;
        require(
            raw.size == entry.uncompressed_size(),
            "exact original animation length",
        )?;
        let bytes =
            manifests::check_payload(self.base, &raw.path, raw.size, &raw.digest, raw.size)?;
        compare_original(archive, entry, &bytes)?;
        let mut rows = AnimationRows {
            base: self.base,
            records: &mut self.records,
            source: &source,
            schema: self.schema,
            totals: &mut self.totals,
        };
        rows.verify(self.family, entry.path(), &bytes)?;
        self.totals.entries += 1;
        self.watch.poll()?;
        Ok(())
    }
}

fn compare_original(archive: &Archive, entry: &Entry, retained: &[u8]) -> TestResult {
    let mut original = archive.open_entry_verified(entry)?;
    let mut buffer = [0_u8; 16 << 10];
    for bytes in retained.chunks(buffer.len()) {
        let observed = buffer.get_mut(..bytes.len()).ok_or("comparison buffer")?;
        original.read_exact(observed)?;
        require(
            observed == bytes,
            "retained payload matches original verified source bytes",
        )?;
    }
    require(
        original.read(&mut buffer)? == 0,
        "retained payload includes the complete Entry",
    )
}

struct Lines {
    input: BufReader<File>,
    line: String,
}

impl Lines {
    fn open(path: &Path) -> TestResult<Self> {
        Ok(Self {
            input: BufReader::new(File::open(path)?),
            line: String::new(),
        })
    }

    fn next<T: DeserializeOwned>(&mut self) -> TestResult<Option<T>> {
        self.line.clear();
        if self.input.read_line(&mut self.line)? == 0 {
            return Ok(None);
        }
        Ok(Some(serde_json::from_str(&self.line)?))
    }
}

#[expect(
    clippy::print_stderr,
    reason = "the opt-in census reports only anonymous aggregate counts"
)]
fn report(family: Family, totals: &Totals) {
    eprintln!(
        "animation {} entries={} chunks={} clips={} metadata_only={} controllers={} declared_keys={} decoded_keys={} partial_rows={} header_budget_refusals={}",
        family.suffix(),
        totals.entries,
        totals.chunks,
        totals.clips,
        totals.metadata_only_clips,
        totals.controllers,
        totals.declared,
        totals.decoded,
        totals.partial,
        totals.header_budget_refusals
    );
}

#[test]
fn animation_accounting_checks_every_selected_entry_including_nested_duplicates() -> TestResult {
    use p4kx_testkit::{
        ArchiveBuilder, TestAnimation, TestChunkFile, TestContainer, TestDba, TestEntry, TestTrack,
    };

    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("Data.p4k");
    let clip = TestAnimation::new("duplicate.caf", 30)
        .track(TestTrack::new(7).positions(&[(1, [0.0; 3]), (7, [1.0, 2.0, 3.0])]));
    let caf = clip.caf();
    let dba = TestDba::new()
        .animation(clip.clone())
        .animation(clip)
        .bytes();
    let nested = TestContainer::new()
        .entry(TestEntry::stored("duplicate.caf", caf.as_slice()))
        .entry(TestEntry::stored("pair.dba", dba))
        .build();
    let aim = TestChunkFile::ivo()
        .chunk(0x0BAD_BEEF, 0x900, b"opaque")
        .bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("duplicate.caf", caf))
        .entry(TestEntry::stored("nested.pak", nested))
        .entry(TestEntry::stored("opaque.aim", aim))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    for (family, entries, clips, keys) in [
        (Family::Caf, 2, 2, 4),
        (Family::Dba, 1, 2, 4),
        (Family::Aim, 1, 0, 0),
    ] {
        let totals = verify_family(&archive, family)?;
        assert!(
            totals.complete,
            "complete generation for each synthetic family"
        );
        assert_eq!(
            (
                totals.entries,
                totals.clips,
                totals.declared,
                totals.decoded,
                totals.partial
            ),
            (entries, clips, keys, keys, 0),
            "census reconciles every selected physical source"
        );
    }
    Ok(())
}

#[test]
fn animation_accounting_distinguishes_diagnostic_rows_and_usable_channels() -> TestResult {
    use p4kx_testkit::{
        ArchiveBuilder, TestAnimation, TestChunkFile, TestDba, TestEntry, TestTrack,
    };

    let temporary = tempfile::tempdir()?;
    let path = temporary.path().join("Data.p4k");
    let damaged = TestAnimation::new("damaged.caf", 0).track(
        TestTrack::new(7)
            .rotation_code(0x85)
            .rotations(&[(0, [0.0, 0.0, 0.0, 1.0])])
            .positions(&[(1, [0.0; 3]), (7, [1.0, 2.0, 3.0])]),
    );
    let dba = TestDba::new()
        .animation(damaged)
        .animation(TestAnimation::new("later.caf", 30))
        .bytes();
    let file = p4kx_extraction::chunks::ChunkFile::parse(dba.as_slice())?;
    let kind = p4kx_extraction::chunks::ivo::DBA_METADATA;
    let (index, metadata) = file.chunks_of(kind).next().ok_or("DBA metadata chunk")?;
    let metadata_only = TestChunkFile::ivo()
        .chunk(
            kind.id(),
            metadata.version(),
            file.chunk_data(index).ok_or("DBA metadata bytes")?,
        )
        .bytes();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("unreadable.caf", b"unreadable container"))
        .entry(TestEntry::stored("damage.dba", dba))
        .entry(TestEntry::stored("metadata-only.dba", metadata_only))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let invalid = verify_family(&archive, Family::Caf)?;
    assert_eq!(
        (invalid.entries, invalid.clips, invalid.partial),
        (1, 0, 1),
        "unreadable source is accounted for"
    );
    let partial = verify_family(&archive, Family::Dba)?;
    assert!(
        !invalid.complete && !partial.complete,
        "diagnostic outputs remain incomplete"
    );
    assert_eq!(
        (
            partial.entries,
            partial.clips,
            partial.metadata_only_clips,
            partial.declared,
            partial.decoded,
            partial.partial
        ),
        (2, 4, 2, 3, 2, 5),
        "complete clip, recovered metadata and usable translation inventories are verified"
    );
    Ok(())
}

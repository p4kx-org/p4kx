// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch, Entry, Filter};
use p4kx_media::{
    Artifact, ByteOrder, ChunkReference, ContainerIdentity, DataType, Digest, ExportOptions,
    Identity, Index, MediaKind, MediaRecord, OffsetBasis, Source, Status,
};

use super::archive_replay::{Lines, compare_original, generation};
use super::shader_accounting::require;
use super::{TestResult, manifests};

#[path = "animation_fixture.rs"]
mod fixture;
#[path = "animation_rows.rs"]
mod rows;

use self::rows::{AnimationRows, Evidence, Family, Totals};

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
        let mut evidence = FileEvidence {
            base: self.base,
            records: &mut self.records,
        };
        let mut rows = AnimationRows {
            evidence: &mut evidence,
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

struct FileEvidence<'a> {
    base: &'a Path,
    records: &'a mut Lines,
}

impl Evidence for FileEvidence<'_> {
    fn record(&mut self) -> TestResult<Option<MediaRecord>> {
        self.records.next()
    }

    fn sample(&mut self, artifact: &Artifact, expected: &[u8]) -> TestResult {
        let bytes = manifests::check_payload(
            self.base,
            &artifact.path,
            artifact.size,
            &artifact.digest,
            expected.len() as u64,
        )?;
        require(
            bytes == expected,
            "every decoded element agrees with the original parser",
        )
    }
}

#[test]
fn animation_accounting_checks_every_selected_entry_including_nested_duplicates() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let archive = fixture::complete(&temporary.path().join("Data.p4k"))?;
    for expected in fixture::EXPECTED {
        expected.check(&verify_family(&archive, expected.family)?)?;
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

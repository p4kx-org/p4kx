// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::sync::Arc;
use std::thread::sleep;
use std::time::{Duration, Instant};

use p4kx_extraction::{
    Archive, PhysicalSourceKind, SectionId, Soundbank, SourceLocation, SourceUseRole,
};
use p4kx_testkit::{
    ArchiveBuilder, TestBank, TestCodec, TestContainer, TestEntry, TestWem, test_key, test_signal,
    wwise_short_id,
};
use tempfile::TempDir;

use super::bank_cache::CachedBank;
use super::{AudioPreview, AudioStatus, BankCache, WemHeaderProvenance};
use crate::EntryIndex;

type TestResult = Result<(), Box<dyn Error>>;

const BANK_PATH: &str = "Data\\Sounds\\wwise\\Test.bnk";
const WEM: u32 = 100;

fn bank_bytes() -> Vec<u8> {
    let wem = TestWem::pcm(TestCodec::WwisePcm, 1, 48_000, &test_signal(4_800, 1)).bytes();
    TestBank::new(wwise_short_id("Test")).wem(WEM, wem).bytes()
}

fn opened() -> Result<(TempDir, Arc<EntryIndex>), Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(BANK_PATH, bank_bytes()))
        .write_to(&path)?;
    let index = EntryIndex::build(Archive::open_with_key(&path, Some(test_key()))?);
    Ok((folder, Arc::new(index)))
}

fn parsed() -> Result<Arc<CachedBank>, Box<dyn Error>> {
    Ok(Arc::new(CachedBank {
        bank: Soundbank::parse(bank_bytes())?,
        original: None,
    }))
}

fn header(preview: &AudioPreview) -> Result<&WemHeaderProvenance, Box<dyn Error>> {
    Ok(preview
        .header_provenance()
        .ok_or("WEM header proof")?
        .map_err(ToString::to_string)?)
}

fn played(preview: &mut AudioPreview) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while preview.player().is_none() && Instant::now() < deadline {
        preview.poll();
        sleep(Duration::from_millis(5));
    }
    preview.player().is_some()
}

#[test]
fn the_cache_keeps_only_the_last_soundbank() -> TestResult {
    let cache = BankCache::default();
    let bank = parsed()?;

    cache.keep(1, &bank);
    cache.keep(2, &bank);

    assert!(cache.get(1).is_none(), "the first was replaced");
    assert!(cache.get(2).is_some(), "the last is kept");
    assert_eq!(cache.kept(), Some(2), "its Entry");
    cache.clear();
    assert!(cache.get(2).is_none(), "cleared");
    Ok(())
}

#[test]
fn a_kept_soundbank_plays_without_reading_its_entry() -> TestResult {
    let (_folder, index) = opened()?;
    let cache = BankCache::default();
    let missing = u32::MAX;
    cache.keep(missing, &parsed()?);

    let mut preview = AudioPreview::start_in_bank(&index, (missing, WEM), &cache, || {});

    assert!(
        played(&mut preview),
        "the kept soundbank decodes: {:?}",
        preview.status()
    );
    let header = header(&preview)?;
    assert_eq!(
        header.duration.contains_encrypted_source(),
        None,
        "unbound cached bytes have unknown origin"
    );
    assert!(
        !header.duration.source_set_complete(),
        "original bank binding is unavailable"
    );
    Ok(())
}

#[test]
fn a_soundbank_that_was_read_is_kept_for_the_next_sound() -> TestResult {
    let (_folder, index) = opened()?;
    let cache = BankCache::default();
    let id = index.id_of_path(BANK_PATH).ok_or("no soundbank Entry")?;

    let mut first = AudioPreview::start_in_bank(&index, (id, WEM), &cache, || {});

    assert!(played(&mut first), "decodes: {:?}", first.status());
    assert_eq!(cache.kept(), Some(id), "the soundbank is kept");
    Ok(())
}

fn duplicate_banks() -> Result<(TempDir, Arc<EntryIndex>, u32), Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let inner = TestContainer::new()
        .entry(TestEntry::stored(BANK_PATH, bank_bytes()))
        .entry(TestEntry::stored(BANK_PATH, bank_bytes()));
    ArchiveBuilder::new()
        .entry(TestEntry::stored("outer.socpak", inner.build()).encrypted())
        .write_to(&path)?;
    let root = Archive::open_with_key(path, Some(test_key()))?;
    let [outer] = root.entries() else {
        return Err("one encrypted Container".into());
    };
    let archive = root.open_container(outer)?;
    let [_, selected] = archive.entries() else {
        return Err("two physical bank duplicates".into());
    };
    let position = archive
        .observed_entry(selected)
        .ok_or("selected physical Bank")?
        .entry_index();
    let index = Arc::new(EntryIndex::build(archive));
    let id = (0..u32::try_from(index.len())?)
        .find(|&id| {
            index
                .archive()
                .zip(index.entry(id))
                .and_then(|(archive, entry)| archive.observed_entry(entry))
                .is_some_and(|entry| entry.entry_index() == position)
        })
        .ok_or("the selected duplicate has a GUI index id")?;
    Ok((folder, index, id))
}

fn assert_encrypted_header(preview: &AudioPreview, ordinal: usize) -> TestResult {
    let proof = header(preview)?;
    for field in [
        &proof.codec,
        &proof.channels,
        &proof.sample_rate,
        &proof.duration,
    ] {
        assert_eq!(
            field.contains_encrypted_source(),
            Some(true),
            "encrypted Container"
        );
        assert!(
            field.source_set_complete(),
            "both physical entries were observed"
        );
    }
    assert_eq!(
        proof.inventory().source_count(),
        2,
        "only the selected Bank and its Container"
    );
    let plain = Soundbank::parse(bank_bytes())?;
    let start = plain
        .source_section_bodies()
        .find_map(|(section, bytes)| (section == SectionId::DIDX).then_some(bytes.start))
        .ok_or("independent DIDX body")?;
    let media_id = SourceLocation::Subresource {
        namespace: "bnkDidx".into(),
        indices: vec![start, start + u64::try_from(size_of::<u32>())?],
        field: "mediaId".into(),
    };
    let selected = proof
        .inventory()
        .uses()
        .iter()
        .find(|used| {
            proof.codec.source_uses_refs().contains(used.id())
                && used.location() == &media_id
                && used.role() == SourceUseRole::Selector
        })
        .ok_or("the original DIDX word selected the WEM")?;
    let bank = proof
        .inventory()
        .sources()
        .iter()
        .find(|source| source.id() == selected.source())
        .ok_or("the selected physical Bank")?;
    assert!(
        matches!(bank.kind(), PhysicalSourceKind::ArchiveEntry(entry) if entry.entry_index() == ordinal),
        "the origin is the selected duplicate"
    );
    assert_eq!(bank.ancestors().len(), 1, "ordered Container ancestry");
    Ok(())
}

#[test]
fn bank_cache_preserves_the_original_wem_selection_and_encrypted_ancestor() -> TestResult {
    let (folder, index, id) = duplicate_banks()?;
    let cache = BankCache::default();
    for _ in 0..2 {
        let mut preview = AudioPreview::start_in_bank(&index, (id, WEM), &cache, || {});
        assert!(played(&mut preview), "the embedded WEM still decodes");
        assert_eq!(cache.kept(), Some(id), "same selected Bank");
        assert_encrypted_header(&preview, 1)?;
    }

    let mut preview = AudioPreview::start_in_bank(&index, (id, WEM), &cache, || {});
    assert!(played(&mut preview), "the existing Bank remains cached");
    std::fs::OpenOptions::new()
        .write(true)
        .open(folder.path().join("Data.p4k"))?
        .set_len(0)?;
    assert!(index.watch().check().is_err(), "Archive change observed");
    assert!(
        matches!(preview.status(), AudioStatus::ArchiveChanged),
        "the cached header is retired"
    );
    assert!(
        preview.header_provenance().is_none(),
        "changed Archive cannot supply a marker"
    );
    Ok(())
}

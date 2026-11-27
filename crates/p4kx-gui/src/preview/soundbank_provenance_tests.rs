// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::io::Read;

use p4kx_extraction::{
    Archive, FieldOrigin, SectionId, Soundbank, SourceInventory, SourceLocation, SourceUseRole,
    SourceUses,
};
use p4kx_testkit::{ArchiveBuilder, TestBank, TestEntry, test_key};

use super::{BankSoundIdentityProvenance, SoundbankSource, SoundbankSummaryProvenance, WORD_BYTES};

const BANK_PATH: &str = "Data\\duplicate.bnk";
const WEM: u32 = 101;

fn contains(
    inventory: &SourceInventory,
    origin: &FieldOrigin,
    expected: &SourceLocation,
    role: SourceUseRole,
) -> bool {
    inventory.iter_uses().any(|used| {
        origin.source_uses_refs().contains(used.id())
            && used.location() == expected
            && used.role() == role
    })
}

fn range(namespace: &str, field: &str, indices: impl Into<Vec<u64>>) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: namespace.into(),
        field: field.into(),
        indices: indices.into(),
    }
}

fn embedded_banks() -> (Vec<u8>, Vec<u8>) {
    let plain = TestBank::new(1).wem(WEM, b"plain".to_vec()).bytes();
    let encrypted = TestBank::new(2)
        .wem(WEM, b"first".to_vec())
        .wem(WEM, b"discarded".to_vec())
        .bytes();
    (plain, encrypted)
}

#[test]
fn bank_summary_and_first_physical_didx_triplet_use_their_actual_domains()
-> Result<(), Box<dyn Error>> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    let (plain, encrypted) = embedded_banks();
    ArchiveBuilder::new()
        .entry(TestEntry::stored(BANK_PATH, plain))
        .entry(TestEntry::deflated(BANK_PATH, encrypted).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(&path, Some(test_key()))?;
    let entry = archive.entries().get(1).ok_or("second physical bank")?;
    let mut bytes = Vec::new();
    archive.open_entry(entry)?.read_to_end(&mut bytes)?;
    let bank = Soundbank::parse(bytes)?;
    let uses = SourceUses::new();
    let source = SoundbankSource::observe(&uses, archive.observed_entry(entry))?;
    let summary = source.summary(&bank)?;
    let wem = bank.wem(WEM).ok_or("admitted WEM")?;
    let bound = source.embedded(&wem);
    let identity = bound.identity()?;
    let mut selectors = Vec::from(bound.selectors()?);
    selectors.push(bound.sampled_bytes(SourceUseRole::Conversion)?);
    let placement = source.origin(&selectors);
    let inventory = uses.freeze()?;

    assert_eq!(bank.wems().len(), 1, "one admitted media ID");
    assert_eq!(wem.bytes(), b"first", "selected physical DIDX triplet");
    for field in [&summary.path, &summary.size] {
        assert_eq!(
            field.contains_encrypted_source(),
            Some(false),
            "directory fact"
        );
    }
    for field in [&summary.wem_count, &identity.id, &identity.size, &placement] {
        assert_eq!(
            field.contains_encrypted_source(),
            Some(true),
            "payload fact"
        );
        assert!(field.source_set_complete(), "exact bank source");
    }

    assert_didx(&bank, &inventory, (&summary, &identity, &placement))?;
    let bytes = wem.source().byte_range();
    assert!(
        contains(
            &inventory,
            &placement,
            &range("bnkEmbeddedWem", "metadataRead", [bytes.start, bytes.end]),
            SourceUseRole::Conversion,
        ),
        "embedded observations are scoped to the selected DATA range"
    );
    assert_eq!(inventory.source_count(), 1, "only the exact bank read");
    Ok(())
}

fn assert_didx(
    bank: &Soundbank<Vec<u8>>,
    inventory: &SourceInventory,
    (summary, identity, placement): (
        &SoundbankSummaryProvenance,
        &BankSoundIdentityProvenance,
        &FieldOrigin,
    ),
) -> Result<(), Box<dyn Error>> {
    use SourceUseRole::{Conversion, Payload, Selector};

    let didx = bank
        .source_section_bodies()
        .find_map(|(section, bytes)| (section == SectionId::DIDX).then_some(bytes))
        .ok_or("original DIDX body")?;
    assert!(
        contains(
            inventory,
            &summary.wem_count,
            &range("bnkDidx", "triplets", [didx.start, didx.end]),
            Conversion,
        ),
        "the distinct count reads the whole DIDX table"
    );
    let start = didx.start;
    let end = start + WORD_BYTES;
    assert!(
        contains(
            inventory,
            &identity.id,
            &range("bnkDidx", "mediaId", [start, end]),
            Payload,
        ),
        "the visible ID uses the first triplet's ID"
    );
    assert!(
        contains(
            inventory,
            placement,
            &range("bnkDidx", "dataOffset", [end, end + WORD_BYTES]),
            Selector,
        ),
        "the same triplet selected the playable range"
    );
    Ok(())
}

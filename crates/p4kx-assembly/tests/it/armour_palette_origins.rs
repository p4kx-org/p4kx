// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    ArchiveModels, Armour, AssemblyOptions, BodyVariant, ForgePalettes, PaletteRecordSelection,
    PaletteRecords, PartOrigin, Piece,
};
use p4kx_extraction::{Archive, DataForgeEntry, DataForgeOrigin, OpenOptions, SourceEncryption};
use p4kx_testkit::{LIVERY_DEFAULT_PALETTE, armour_entries, livery_entries, test_key};

use crate::assembly_support::{TestResult, fixture};

#[test]
fn an_applied_default_palette_retains_the_actual_encrypted_record_for_each_tinted_slot()
-> TestResult {
    const DATABASE: &str = r"Data\Game2.dcb";
    let entries = livery_entries()
        .into_iter()
        .map(|entry| {
            if entry.path() == DATABASE {
                entry.encrypted()
            } else {
                entry
            }
        })
        .chain(
            armour_entries()
                .into_iter()
                .filter(|entry| entry.path() != DATABASE),
        )
        .collect();
    let fixture = fixture(entries)?;
    let archive = Archive::open_with(&fixture.path, OpenOptions::new().key(Some(test_key())))?;
    let source = DataForgeEntry::read(&archive)?;
    let DataForgeOrigin::Archive(origin) = source.origin() else {
        return Err("the palette DataForge should have a physical origin".into());
    };
    assert_eq!(origin.encryption(), SourceEncryption::Encrypted);
    let forge = source.open()?;
    let record = forge
        .records_at_path(LIVERY_DEFAULT_PALETTE)
        .next()
        .ok_or("default palette Record")?;
    let expected = PaletteRecordSelection {
        index: record.index(),
        guid: record.guid(),
    };
    let armour = Armour::build_with_palettes(
        BodyVariant::Male,
        &[Piece::file(
            "painted",
            "Objects/Test/livery_part.cgf",
            Some("Objects/Test/livery_default".to_owned()),
        )],
        &ArchiveModels::new(&archive),
        &ForgePalettes::new(&forge),
        &AssemblyOptions::default(),
    )?;
    let painted = armour
        .parts()
        .iter()
        .find(|part| part.origin() == PartOrigin::Piece)
        .ok_or("painted part")?;
    let records = Some(PaletteRecords {
        selector: Some(expected),
        palette: Some(expected),
    });
    assert_eq!(painted.palette_records_for(0), None);
    assert_eq!(painted.palette_records_for(1), records);
    assert_eq!(painted.palette_records_for(2), records);
    assert_eq!(painted.palette_records_for(3), None);
    Ok(())
}

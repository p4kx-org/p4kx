// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Armour drawn with the palette of its worn geometry, main geometry or material library.

use p4kx_assembly::{
    ArchiveModels, Armour, AssemblyOptions, BodyVariant, IssueKind, LoadoutResolver,
    PaletteFailure, PaletteProvider, PartOrigin, Piece,
};
use p4kx_dataforge::Guid;
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_model::{Material, PaletteSlot, TintEntry, TintPalette};
use p4kx_testkit::{MAIN_PALETTE_SUIT, PAINTED_LIBRARY_PALETTE, PAINTED_SUIT, painted_entries};

use crate::assembly_support::{TestResult, fixture};

const LIBRARY_GUID: [u8; 16] = [7; 16];
const FROM_MALE: [f32; 3] = [0.03, 0.03, 0.03];
const FROM_FEMALE: [f32; 3] = [0.02, 0.2, 0.4];
const FROM_MAIN: [f32; 3] = [0.21, 0.14, 0.35];
const FROM_LIBRARY: [f32; 3] = [0.8, 0.7, 0.1];

struct Stub {
    geometry_exists: bool,
}

fn palette(name: &str, tint: [f32; 3]) -> TintPalette {
    let entry = TintEntry {
        tint,
        specular: [0.05; 3],
        glossiness: 200.0,
    };
    TintPalette::new(name, [entry; 3], [1.0; 3])
}

impl PaletteProvider for Stub {
    fn by_guid(&self, guid: Guid) -> Result<TintPalette, PaletteFailure> {
        let text = guid.to_string();
        if guid == Guid::from_stored(LIBRARY_GUID) {
            Ok(palette("from_library", FROM_LIBRARY))
        } else if text.ends_with("7c") {
            Ok(palette("from_main", FROM_MAIN))
        } else if self.geometry_exists && text.ends_with("7a") {
            Ok(palette("from_male", FROM_MALE))
        } else if self.geometry_exists && text.ends_with("7b") {
            Ok(palette("from_female", FROM_FEMALE))
        } else {
            Err(PaletteFailure::Missing)
        }
    }

    fn guid_at(&self, path: &str) -> Result<Guid, PaletteFailure> {
        if path.eq_ignore_ascii_case(PAINTED_LIBRARY_PALETTE) {
            Ok(Guid::from_stored(LIBRARY_GUID))
        } else {
            Err(PaletteFailure::Missing)
        }
    }
}

fn wear(
    name: &str,
    variant: BodyVariant,
    palettes: Option<&Stub>,
) -> Result<Armour, Box<dyn std::error::Error>> {
    let fixture = fixture(painted_entries())?;
    let archive = Archive::open(&fixture.path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge);
    let record = resolver.entities().find(name)?;
    let pieces = vec![Piece::wearable(resolver.wearable(&record)?)];
    let models = ArchiveModels::new(&archive);
    let options = AssemblyOptions::default();
    Ok(match palettes {
        Some(stub) => Armour::build_with_palettes(variant, &pieces, &models, stub, &options)?,
        None => Armour::build(variant, &pieces, &models, &options)?,
    })
}

fn caps_of(armour: &Armour, origin: PartOrigin) -> Result<Material, &'static str> {
    armour
        .parts()
        .iter()
        .find(|part| part.origin() == origin)
        .and_then(|part| part.materials().library())
        .and_then(|library| library.materials.iter().find(|m| m.name == "caps"))
        .cloned()
        .ok_or("the caps material")
}

fn caps(armour: &Armour) -> Result<Material, &'static str> {
    caps_of(armour, PartOrigin::Piece)
}

const fn known() -> Stub {
    Stub {
        geometry_exists: true,
    }
}

#[test]
fn a_piece_takes_the_palette_its_geometry_names() -> TestResult {
    let armour = wear(PAINTED_SUIT, BodyVariant::Male, Some(&known()))?;

    let caps = caps(&armour)?;

    assert_eq!(caps.palette, Some(PaletteSlot::Primary));
    assert!(caps.tinted, "drawn with the palette");
    assert_eq!(caps.diffuse, FROM_MALE);
    assert_eq!(
        caps.layers
            .first()
            .and_then(|layer| layer.palette_entry)
            .map(|entry| entry.tint),
        Some(FROM_MALE)
    );
    assert!(armour.issues().is_empty(), "{:?}", armour.issues());
    Ok(())
}

#[test]
fn each_body_takes_the_palette_of_its_own_sub_geometry() -> TestResult {
    let female = wear(PAINTED_SUIT, BodyVariant::Female, Some(&known()))?;
    let male = wear(PAINTED_SUIT, BodyVariant::Male, Some(&known()))?;

    assert_eq!(caps(&female)?.diffuse, FROM_FEMALE);
    assert_eq!(caps(&male)?.diffuse, FROM_MALE);
    Ok(())
}

#[test]
fn a_worn_variant_without_a_palette_uses_the_items_main_palette() -> TestResult {
    let armour = wear(MAIN_PALETTE_SUIT, BodyVariant::Male, Some(&known()))?;
    let part = armour
        .parts()
        .iter()
        .find(|part| part.origin() == PartOrigin::Piece)
        .ok_or("the worn part")?;

    assert!(part.file().ends_with("/m_suit.skin"), "{}", part.file());
    assert_eq!(caps(&armour)?.diffuse, FROM_MAIN);
    assert!(armour.issues().is_empty(), "{:?}", armour.issues());
    Ok(())
}

#[test]
fn the_body_takes_no_palette_from_the_piece_worn_on_it() -> TestResult {
    for (variant, own) in [
        (BodyVariant::Male, FROM_MALE),
        (BodyVariant::Female, FROM_FEMALE),
    ] {
        let armour = wear(PAINTED_SUIT, variant, Some(&known()))?;

        let body = caps_of(&armour, PartOrigin::Body)?;

        assert_ne!(body.diffuse, own, "{variant:?}");
    }
    Ok(())
}

#[test]
fn a_piece_without_a_palette_of_its_own_takes_the_default_of_its_library() -> TestResult {
    let armour = wear("TEST_Suit", BodyVariant::Male, Some(&known()))?;

    assert_eq!(caps(&armour)?.diffuse, FROM_LIBRARY);
    Ok(())
}

#[test]
fn a_palette_that_cannot_be_read_is_listed_and_the_library_default_is_used() -> TestResult {
    let stub = Stub {
        geometry_exists: false,
    };
    let armour = wear(PAINTED_SUIT, BodyVariant::Male, Some(&stub))?;

    assert_eq!(caps(&armour)?.diffuse, FROM_LIBRARY);
    let listed = armour
        .issues()
        .iter()
        .filter(|issue| matches!(issue.kind(), IssueKind::Palette { .. }))
        .count();
    assert_eq!(listed, 1);
    Ok(())
}

#[test]
fn without_palettes_nothing_is_painted() -> TestResult {
    let armour = wear(PAINTED_SUIT, BodyVariant::Male, None)?;

    let caps = caps(&armour)?;

    assert!(!caps.tinted);
    assert_ne!(caps.diffuse, FROM_MALE);
    Ok(())
}

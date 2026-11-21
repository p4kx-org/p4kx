// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::entity_armour::{FEMALE_ARMOUR, MALE_ARMOUR, armour_database, armour_files};
use crate::{TestEntity, TestEntry, TestMtl, TestMtlMaterial};

const LIBRARY: &str = "Data\\objects\\test\\cube.mtl";

/// The class name of the suit whose male skin names a tint palette, see [`painted_entries`].
pub const PAINTED_SUIT: &str = "TEST_PaintedSuit";

/// The GUID of the tint palette Record the male skin of [`PAINTED_SUIT`] names.
pub const PAINTED_PALETTE_GUID: u128 = 0x7a;

/// The GUID of the tint palette Record the female skin of [`PAINTED_SUIT`] names.
pub const PAINTED_FEMALE_PALETTE_GUID: u128 = 0x7b;

/// The class name of the suit whose worn skin uses the main geometry's palette.
pub const MAIN_PALETTE_SUIT: &str = "TEST_MainPaletteSuit";

/// The GUID of the palette on the main geometry of the painted suits.
const PAINTED_MAIN_PALETTE_GUID: u128 = 0x7c;

/// The path of the tint palette the material library of [`painted_entries`] names as its default.
pub const PAINTED_LIBRARY_PALETTE: &str = "libs/foundry/records/tintpalettes/test_armour";

/// Returns the Entries of [`armour_entries`](crate::armour_entries) with painted armour.
///
/// The material library of the cubes has a material `caps` with a layer that takes the primary
/// palette entry, and names [`PAINTED_LIBRARY_PALETTE`] as its default palette.
/// [`PAINTED_SUIT`] is worn like `TEST_Suit`; its male skin names the Record
/// [`PAINTED_PALETTE_GUID`] and its female skin names [`PAINTED_FEMALE_PALETTE_GUID`]. Both painted
/// suits have a main palette. [`MAIN_PALETTE_SUIT`] names
/// only the main palette. The Archive does not hold these palette Records.
#[must_use]
pub fn painted_entries() -> Vec<TestEntry> {
    let male_skin = format!("{MALE_ARMOUR}/m_suit.skin");
    let suit = TestEntity::new(0x60, PAINTED_SUIT, "entities/scitem/test_painted_suit.xml")
        .geometry("Objects/Test/crate.cgf", None)
        .palette(PAINTED_MAIN_PALETTE_GUID, "")
        .geometry_variant("Male", male_skin.as_str(), None)
        .variant_palette(PAINTED_PALETTE_GUID, "")
        .geometry_variant("Female", format!("{FEMALE_ARMOUR}/f_suit.skin"), None)
        .variant_palette(PAINTED_FEMALE_PALETTE_GUID, "");
    let main_palette_suit = TestEntity::new(
        0x61,
        MAIN_PALETTE_SUIT,
        "entities/scitem/test_main_palette_suit.xml",
    )
    .geometry("Objects/Test/crate.cgf", Some("Objects/Test/crate.mtl"))
    .palette(PAINTED_MAIN_PALETTE_GUID, "")
    .geometry_variant("Male", male_skin, Some("Objects/Test/cube.mtl"));
    let database = armour_database()
        .record(suit.record())
        .record(main_palette_suit.record())
        .build();
    let library = TestMtl::multi(&[
        TestMtlMaterial::new("unused").diffuse([0.5; 3]),
        TestMtlMaterial::new("caps")
            .diffuse([1.0; 3])
            .layer("paint", "1"),
        TestMtlMaterial::new("sides").diffuse([0.5; 3]),
    ])
    .with_default_palette(PAINTED_LIBRARY_PALETTE);
    let mut entries = vec![TestEntry::zstd("Data\\Game2.dcb", database)];
    entries.extend(
        armour_files()
            .into_iter()
            .filter(|entry| !entry.path.eq_ignore_ascii_case(LIBRARY)),
    );
    entries.push(TestEntry::stored(LIBRARY, library.cryxml()));
    entries
}

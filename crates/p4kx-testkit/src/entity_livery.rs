// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{
    DataForgeBuilder, SplitDds, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry, TestModel,
    TestMtl, TestMtlMaterial, TestTextureFormat, entity_schema, tint_palette_record,
    tint_palette_record_with_decal,
};

/// The record path of the palette the library `Objects/Test/livery_default` names as its default.
pub const LIVERY_DEFAULT_PALETTE: &str = "Libs/Foundry/Records/TintPalettes/brand/test/p_default";

/// The sRGB bytes of the tint colours A to C of the palette `p_ship` of `TEST_Livery`.
pub const LIVERY_SHIP_TINTS: [[u8; 3]; 3] = [[255, 0, 0], [0, 255, 0], [0, 0, 255]];

/// The sRGB bytes of the decal colours red, green and blue of the palette `p_ship` of
/// `TEST_Livery`.
pub const LIVERY_SHIP_DECALS: [[u8; 3]; 3] = [[100, 15, 25], [10, 10, 10], [144, 144, 144]];

/// The sRGB bytes of the tint colours A to C of the palette `p_gun` of `TEST_Painted`.
pub const LIVERY_GUN_TINTS: [[u8; 3]; 3] = [[0, 255, 255], [255, 0, 255], [255, 255, 0]];

/// The sRGB bytes of the tint colours A to C of the palette `p_default`, the default palette of
/// the library `Objects/Test/livery_default`.
pub const LIVERY_DEFAULT_TINTS: [[u8; 3]; 3] = [[255, 255, 255], [0, 0, 0], [255, 255, 255]];

const SIDE: u32 = 8;

fn part(guid: u128, class: &str, material: &str) -> TestEntity {
    TestEntity::new(
        guid,
        class,
        format!("entities/scitem/{}.xml", class.to_ascii_lowercase()),
    )
    .geometry("Objects/Test/livery_part.cgf", Some(material))
}

fn ship(guid: u128, class: &str, has_palette: bool, ports: &[(&str, &str)]) -> TestEntity {
    let entity = TestEntity::new(
        guid,
        class,
        format!("entities/spaceships/{}.xml", class.to_ascii_lowercase()),
    )
    .geometry("Objects/Test/livery_ship.cgf", Some("Objects/Test/livery"))
    .loadout(loadout(ports));
    if has_palette {
        entity.palette(0x31, "")
    } else {
        entity
    }
}

fn loadout(ports: &[(&str, &str)]) -> TestLoadout {
    TestLoadout::Manual(
        ports
            .iter()
            .map(|(port, class)| TestLoadoutEntry::class(*port, *class))
            .collect(),
    )
}

fn entities() -> [TestEntity; 9] {
    [
        ship(
            0x21,
            "TEST_Livery",
            true,
            &[
                ("hardpoint_painted", "TEST_Painted"),
                ("hardpoint_twin", "TEST_Painted"),
                ("hardpoint_default", "TEST_Default"),
                ("hardpoint_lost", "TEST_Lost"),
                ("hardpoint_child", "TEST_Child"),
                ("hardpoint_plain", "TEST_Plain"),
            ],
        ),
        ship(
            0x27,
            "TEST_Bare",
            false,
            &[
                ("hardpoint_painted", "TEST_Painted"),
                ("hardpoint_default", "TEST_Default"),
                ("hardpoint_plain", "TEST_Plain"),
            ],
        ),
        ship(
            0x28,
            "TEST_Carrier",
            true,
            &[("hardpoint_mount", "TEST_Mount")],
        ),
        part(0x29, "TEST_Mount", "Objects/Test/livery_default").loadout(loadout(&[
            ("hardpoint_inner", "TEST_Plain"),
            ("hardpoint_gun", "TEST_Painted"),
        ])),
        part(0x22, "TEST_Painted", "Objects/Test/livery").palette(0x32, ""),
        part(0x23, "TEST_Default", "Objects/Test/livery_default"),
        part(0x24, "TEST_Lost", "Objects/Test/livery").palette(0x3fff, ""),
        part(0x25, "TEST_Child", "Objects/Test/livery").palette(0x31, "left"),
        part(0x26, "TEST_Plain", "Objects/Test/livery"),
    ]
}

fn database() -> DataForgeBuilder {
    let palettes = [
        tint_palette_record_with_decal(
            0x31,
            "p_ship",
            LIVERY_SHIP_TINTS,
            [255; 3],
            [51, 102, 153],
            LIVERY_SHIP_DECALS,
        ),
        tint_palette_record(0x32, "p_gun", LIVERY_GUN_TINTS, [255; 3], [51, 102, 153]),
        tint_palette_record(
            0x33,
            "p_default",
            LIVERY_DEFAULT_TINTS,
            [255; 3],
            [51, 102, 153],
        ),
    ];
    let builder = entities()
        .iter()
        .fold(entity_schema(DataForgeBuilder::new()), |builder, entity| {
            builder.record(entity.record())
        });
    palettes.into_iter().fold(builder, DataForgeBuilder::record)
}

fn library(default_palette: Option<&str>) -> Vec<u8> {
    let painted = |name: &str, tint: &str| {
        TestMtlMaterial::new(name)
            .shader("HardSurface")
            .diffuse([0.5; 3])
            .texture("TexSlot1", "Objects/Test/livery_diff.tif")
            .layer("Primary", tint)
    };
    let file = TestMtl::multi(&[
        TestMtlMaterial::new("unused"),
        painted("caps", "1"),
        painted("sides", "2"),
    ]);
    match default_palette {
        Some(palette) => file.with_default_palette(palette).cryxml(),
        None => file.cryxml(),
    }
}

/// Returns the Entries of a synthetic Archive with a ship Entity `TEST_Livery` in liveries.
///
/// The ship and its parts draw the cube with a library whose `caps` takes the primary and
/// `sides` the secondary palette colour, both over the diffuse texture
/// `Objects/Test/livery_diff` (8 by 8 pixels, 4 mips). The ship chooses the palette `p_ship`
/// ([`LIVERY_SHIP_TINTS`]). Its Loadout holds
///
/// - `TEST_Painted` twice, in `hardpoint_painted` and `hardpoint_twin`, with the palette `p_gun`
///   ([`LIVERY_GUN_TINTS`]);
/// - `TEST_Default` in `hardpoint_default`, which names no palette, and its library
///   `Objects/Test/livery_default` names `p_default` ([`LIVERY_DEFAULT_TINTS`]) as the default;
/// - `TEST_Lost` in `hardpoint_lost`, whose palette does not exist;
/// - `TEST_Child` in `hardpoint_child`, which names a child of `p_ship`, which is not supported;
/// - `TEST_Plain` in `hardpoint_plain`, without a palette.
///
/// A part that names no palette takes the one of its nearest ancestor (ADR-0223), so the parts
/// without a palette of their own all draw with `p_ship` here. Two more ships show the rest:
///
/// - `TEST_Bare` names no palette; its `hardpoint_painted` (`p_gun`), `hardpoint_default` (the
///   library default `p_default`) and `hardpoint_plain` (none) keep what they have;
/// - `TEST_Carrier` names `p_ship`; its `hardpoint_mount` (`TEST_Mount`, no palette, library with
///   the default `p_default`) holds `hardpoint_inner` (`TEST_Plain`, no palette) and `hardpoint_gun`
///   (`TEST_Painted`, `p_gun`).
#[must_use]
pub fn livery_entries() -> Vec<TestEntry> {
    let model = TestModel::cube();
    let mut entries = vec![TestEntry::zstd("Data\\Game2.dcb", database().build())];
    for name in ["livery_ship", "livery_part"] {
        entries.push(TestEntry::stored(
            format!("Data\\Objects\\Test\\{name}.cgf"),
            model.cgf(),
        ));
        entries.push(TestEntry::stored(
            format!("Data\\Objects\\Test\\{name}.cgfm"),
            model.cgfm(),
        ));
    }
    entries.push(TestEntry::stored(
        "Data\\Objects\\Test\\livery.mtl",
        library(None),
    ));
    entries.push(TestEntry::stored(
        "Data\\Objects\\Test\\livery_default.mtl",
        library(Some(LIVERY_DEFAULT_PALETTE)),
    ));
    let texture = SplitDds::new(TestTextureFormat::LegacyDxt1, SIDE, SIDE)
        .mips(4)
        .split(1);
    entries.extend(texture.entries("Data\\Objects\\Test\\livery_diff.dds"));
    entries
}

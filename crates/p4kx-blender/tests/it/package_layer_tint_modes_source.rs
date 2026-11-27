// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{
    DataForgeBuilder, SplitDds, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry, TestModel,
    TestMtl, TestMtlMaterial, TestTextureFormat, entity_schema, tint_palette_record_with_specular,
};

pub(super) const ROOT_CLASS: &str = "TEST_PaletteMode_Zero";
pub(super) const PALETTE_DIFFUSE: [u8; 3] = [192, 128, 64];
pub(super) const PALETTE_METAL: [u8; 3] = [208, 240, 192];
pub(super) const PALETTE_DIELECTRIC: [u8; 3] = [64, 48, 32];
pub(super) const AUTHORED_METAL: [f32; 3] = [0.7, 0.6, 0.9];
pub(super) const AUTHORED_DIELECTRIC: [f32; 3] = [0.031_25, 0.0625, 0.015_625];
const MODEL: &str = "Objects/Test/tint_modes.cgf";
const COMPANION: &str = "Objects/Test/tint_mode_layers.mtl";
const SPECULAR_MAP: &str = "Objects/Test/tint_mode_spec.dds";
const PALETTE_GUID: u128 = 0xA550;
const FIRST_ENTITY_GUID: u128 = 0xA600;

#[derive(Clone, Copy)]
pub(super) struct Case {
    pub(super) name: &'static str,
    pub(super) parameters: &'static [(&'static str, &'static str)],
    pub(super) selected: bool,
    crossed_reflectance: bool,
}

impl Case {
    const fn with_crossed_reflectance(mut self) -> Self {
        self.crossed_reflectance = true;
        self
    }

    pub(super) const fn authored_reflectance(self, lane: Lane) -> [f32; 3] {
        let authored_lane = match (lane, self.crossed_reflectance) {
            (Lane::Dielectric, false) | (Lane::Metal, true) => Lane::Dielectric,
            (Lane::Metal, false) | (Lane::Dielectric, true) => Lane::Metal,
        };
        authored_lane.authored_reflectance()
    }
}

pub(super) const CASES: [Case; 10] = [
    case("Zero", &[("TintMode", "0")]),
    case("One", &[("TintMode", "1")]).with_crossed_reflectance(),
    case("Two", &[("TintMode", "2")]).with_crossed_reflectance(),
    case("Three", &[("TintMode", "3")]),
    case("Absent", &[]),
    case("Malformed", &[("TintMode", "2x")]),
    case("Nonfinite", &[("TintMode", "NaN")]),
    case("Unknown", &[("TintMode", "4")]),
    case("Repeated", &[("TintMode", "1"), ("tintmode", "2")]),
    Case {
        name: "Unselected",
        parameters: &[("TintMode", "2")],
        selected: false,
        crossed_reflectance: false,
    },
];

const fn case(name: &'static str, parameters: &'static [(&'static str, &'static str)]) -> Case {
    Case {
        name,
        parameters,
        selected: true,
        crossed_reflectance: false,
    }
}

#[derive(Clone, Copy)]
pub(super) enum Lane {
    Dielectric,
    Metal,
}

impl Lane {
    pub(super) const ALL: [Self; 2] = [Self::Dielectric, Self::Metal];

    pub(super) fn material_name(self, case: Case) -> String {
        let lane = match self {
            Self::Dielectric => "Dielectric",
            Self::Metal => "Metal",
        };
        format!("TintMode_{}_{lane}", case.name)
    }

    fn source_name(self, case: Case) -> String {
        format!("{}_Source", self.material_name(case))
    }

    const fn palette(self) -> &'static str {
        match self {
            Self::Dielectric => "1",
            Self::Metal => "3",
        }
    }

    const fn authored_reflectance(self) -> [f32; 3] {
        match self {
            Self::Dielectric => AUTHORED_DIELECTRIC,
            Self::Metal => AUTHORED_METAL,
        }
    }
}

pub(super) fn entries() -> Result<Vec<TestEntry>, Box<dyn Error>> {
    let model = TestModel::cube();
    let mut entries = vec![
        TestEntry::stored(format!("Data/{MODEL}"), model.cgf()),
        TestEntry::stored("Data/Objects/Test/tint_modes.cgfm", model.cgfm()),
        TestEntry::stored(format!("Data/{COMPANION}"), layer_library()),
        TestEntry::stored(format!("Data/{SPECULAR_MAP}"), white_specular()?),
    ];
    let database = database(&mut entries);
    entries.push(TestEntry::zstd("Data/Game2.dcb", database).encrypted());
    Ok(entries)
}

fn database(entries: &mut Vec<TestEntry>) -> Vec<u8> {
    let palette = tint_palette_record_with_specular(
        PALETTE_GUID,
        "p_tint_modes",
        [PALETTE_DIFFUSE, [255, 0, 255], PALETTE_DIFFUSE],
        [255; 3],
        [184; 3],
        [PALETTE_DIELECTRIC, [0, 0, 255], PALETTE_METAL],
    );
    let mut database = entity_schema(DataForgeBuilder::new()).record(palette);
    let mut root = None;
    let mut children = Vec::new();
    for (id, case) in (FIRST_ENTITY_GUID..).zip(CASES) {
        let class = format!("TEST_PaletteMode_{}", case.name);
        let material = format!("Objects/Test/tint_mode_{}", case.name);
        entries.push(TestEntry::stored(
            format!("Data/{material}.mtl"),
            hull_library(case),
        ));
        let entity = TestEntity::new(id, &class, format!("entities/scitem/{class}.xml"))
            .geometry(MODEL, Some(&material))
            .palette(PALETTE_GUID, "");
        if class == ROOT_CLASS {
            root = Some(entity);
        } else {
            children.push(TestLoadoutEntry::class(class.as_str(), class.as_str()));
            database = database.record(entity.record());
        }
    }
    if let Some(root) = root {
        database = database.record(root.loadout(TestLoadout::Manual(children)).record());
    }
    database.build()
}

fn hull_library(case: Case) -> Vec<u8> {
    let make = |lane: Lane| {
        let source = lane.source_name(case);
        let mut layer = vec![
            ("Name", "BaseLayer1"),
            ("Path", COMPANION),
            ("Submtl", source.as_str()),
        ];
        if case.selected {
            layer.push(("PaletteTint", lane.palette()));
        }
        TestMtlMaterial::new(lane.material_name(case))
            .shader("LayerBlend_V2")
            .layer_with(&layer)
    };
    TestMtl::multi(&[
        TestMtlMaterial::new("unused"),
        make(Lane::Dielectric),
        make(Lane::Metal),
    ])
    .cryxml()
}

fn layer_library() -> Vec<u8> {
    let layers: Vec<_> = CASES
        .into_iter()
        .flat_map(|case| Lane::ALL.into_iter().map(move |lane| child(case, lane)))
        .collect();
    TestMtl::multi(&layers).cryxml()
}

fn child(case: Case, lane: Lane) -> TestMtlMaterial {
    let mut child = TestMtlMaterial::new(lane.source_name(case))
        .shader("Layer")
        .diffuse([1.0; 3])
        .specular(case.authored_reflectance(lane))
        .public_param("LayerTiling", "1");
    for &(name, value) in case.parameters {
        child = child.public_param(name, value);
    }
    if matches!(lane, Lane::Dielectric) && case.selected {
        child = child.texture("TexSlot4", SPECULAR_MAP);
    }
    child
}

fn white_specular() -> Result<Vec<u8>, Box<dyn Error>> {
    let fixture = SplitDds::new(TestTextureFormat::LegacyBgra8, 2, 2).mips(1);
    let mut bytes = fixture.merged_bytes();
    bytes
        .get_mut(fixture.header_len()..)
        .ok_or("fixture DDS payload")?
        .fill(255);
    Ok(bytes)
}

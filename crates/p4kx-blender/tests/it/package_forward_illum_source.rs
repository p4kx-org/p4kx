// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry, TestModel, TestMtl,
    TestMtlMaterial, entity_schema,
};

use super::super::package_glow::emission_dds;

pub(super) const ROOT_CLASS: &str = "TEST_ForwardIllum";
pub(super) const DIFFUSE: &str = "Objects/Test/forward_diff.dds";
pub(super) const DIFFUSE_SLOT: &str = "TexSlot1";
pub(super) const EMITTANCE: &str = "Objects/Test/forward_emittance.dds";
pub(super) const DIFFUSE_PIXEL: [u8; 4] = [204, 153, 102, 128];
pub(super) const EMITTANCE_PIXEL: [u8; 4] = [128, 64, 32, 255];
pub(super) const TINT: [f32; 3] = [0.2, 0.4, 0.8];
pub(super) const OPACITY: f32 = 0.45;
const CHILD: &str = "TEST_ForwardIllumControl";
const MODEL: &str = "Objects/Test/forward_illum.cgf";
const FIRST_LIBRARY: &str = "Objects/Test/forward_illum_base";
const SECOND_LIBRARY: &str = "Objects/Test/forward_illum_control";

pub(super) const CASES: [Case; 4] = [
    Case {
        name: "ForwardAbsent",
        slot: 1,
        glow: None,
        cutoff: None,
        mapped: false,
    },
    Case {
        name: "ForwardZero",
        slot: 2,
        glow: Some("0"),
        cutoff: Some("0"),
        mapped: false,
    },
    Case {
        name: "ForwardPositive",
        slot: 1,
        glow: Some("2"),
        cutoff: Some("0.5"),
        mapped: false,
    },
    Case {
        name: "ForwardMapped",
        slot: 2,
        glow: Some("2"),
        cutoff: Some("0.5"),
        mapped: true,
    },
];

#[derive(Clone, Copy)]
pub(super) struct Case {
    pub(super) name: &'static str,
    pub(super) slot: usize,
    pub(super) glow: Option<&'static str>,
    pub(super) cutoff: Option<&'static str>,
    pub(super) mapped: bool,
}

pub(super) fn entries() -> Result<Vec<TestEntry>, Box<dyn Error>> {
    let root = TestEntity::new(0x1001, ROOT_CLASS, "entities/forward/root.xml")
        .geometry(MODEL, Some(FIRST_LIBRARY))
        .loadout(TestLoadout::Manual(vec![TestLoadoutEntry::class(
            "control", CHILD,
        )]));
    let child = TestEntity::new(0x1002, CHILD, "entities/forward/control.xml")
        .geometry(MODEL, Some(SECOND_LIBRARY));
    let database = entity_schema(DataForgeBuilder::new())
        .record(root.record())
        .record(child.record())
        .build();
    let model = TestModel::cube();
    let [absent, zero, positive, mapped] = CASES;
    Ok(vec![
        TestEntry::zstd("Data/Game2.dcb", database),
        TestEntry::stored(format!("Data/{MODEL}"), model.cgf()),
        TestEntry::stored("Data/Objects/Test/forward_illum.cgfm", model.cgfm()),
        TestEntry::stored(format!("Data/{FIRST_LIBRARY}.mtl"), library(absent, zero)).encrypted(),
        TestEntry::stored(
            format!("Data/{SECOND_LIBRARY}.mtl"),
            library(positive, mapped),
        )
        .encrypted(),
        TestEntry::stored(format!("Data/{DIFFUSE}"), emission_dds(DIFFUSE_PIXEL)?).encrypted(),
        TestEntry::stored(format!("Data/{EMITTANCE}"), emission_dds(EMITTANCE_PIXEL)?),
    ])
}

fn library(first: Case, second: Case) -> Vec<u8> {
    TestMtl::multi(&[
        TestMtlMaterial::new("unused"),
        material(first),
        material(second),
    ])
    .text_xml()
}

fn material(case: Case) -> TestMtlMaterial {
    let mut input = TestMtlMaterial::new(case.name)
        .attribute("MtlFlags", "32768")
        .diffuse([1.0; 3])
        .emissive(TINT)
        .opacity(OPACITY)
        .texture(DIFFUSE_SLOT, DIFFUSE);
    if let Some(glow) = case.glow {
        input = input.attribute("Glow", glow);
    }
    if let Some(cutoff) = case.cutoff {
        input = input.public_param("AlphaGlowCutoff", cutoff);
    }
    if case.mapped {
        input = input
            .attribute("StringGenMask", "%EMITTANCE_MAP")
            .texture("Emittance", EMITTANCE);
    }
    input
}

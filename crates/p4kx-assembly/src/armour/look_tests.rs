// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_character::{
    BODY_COLOR, ColorParam, EYE_COLOR, FloatParam, Guid, HAIR_DYE_COLOR, HAIR_TINT_COLOR, Label,
    Material as Choice, SubMaterial, lowered_hash, name_hash,
};
use p4kx_extraction::SourceLocation;
use p4kx_model::{MaterialLibrary, MaterialStatus};

use super::Look;

pub(super) fn library(xml: &str) -> MaterialStatus {
    MaterialStatus::Loaded(MaterialLibrary::parse("test", xml.as_bytes()).expect("a valid library"))
}

fn look(name: &str, colors: Vec<ColorParam>, floats: Vec<FloatParam>) -> Look {
    Look::new(&Choice {
        attachment: 1,
        label: Label,
        base_material: Guid::default(),
        flags: 0,
        sub_materials: vec![SubMaterial {
            name: lowered_hash(name),
            label: Label,
            textures: Vec::new(),
            floats,
            colors,
        }],
    })
}

const fn colour(name: u32, rgba: [u8; 4]) -> ColorParam {
    ColorParam {
        name,
        label: Label,
        rgba,
    }
}

pub(super) fn diffuse(status: &MaterialStatus, at: usize) -> [f32; 3] {
    status.library().expect("a library").materials[at].diffuse
}

const SKIN: &str = r#"<Material><SubMaterials>
    <Material Name="shader_head" Shader="HumanSkin_V2" Diffuse="1,1,1">
      <Textures><Texture Map="TexSlot1" File="a/head_diff.tif"/></Textures>
      <PublicParams SourceAverageColor="0.2,0.1,0.05"/>
    </Material>
    <Material Name="grey" Shader="HumanSkin_V2" Diffuse="1,1,1"/>
    </SubMaterials></Material>"#;

#[test]
fn a_skin_with_a_diffuse_texture_is_rebased_by_the_ratio_of_the_skin_tones() {
    let skin = look(
        "shader_head",
        vec![colour(BODY_COLOR, [255, 255, 255, 255])],
        Vec::new(),
    );

    let status = skin.apply(&library(SKIN)).expect("the material changes");

    let [red, green, blue] = diffuse(&status, 0);
    assert!((red - 0.25).abs() < 1e-5, "{red} keeps the hue");
    assert!((green - 0.5).abs() < 1e-5, "{green} keeps the hue");
    assert!((blue - 1.0).abs() < 1e-5, "{blue} is the brightest channel");
}

#[test]
fn a_skin_without_a_diffuse_texture_takes_the_skin_tone() {
    let skin = look(
        "grey",
        vec![colour(BODY_COLOR, [255, 0, 0, 255])],
        Vec::new(),
    );

    let status = skin.apply(&library(SKIN)).expect("the material changes");

    assert_eq!(diffuse(&status, 1), [1.0, 0.0, 0.0]);
    assert_eq!(
        diffuse(&status, 0),
        [1.0; 3],
        "the other material is as it was"
    );
}

#[test]
fn the_iris_colour_tints_an_eye_and_other_materials_stay() {
    let eyes = library(
        r#"<Material><SubMaterials>
        <Material Name="shader_eyeInner" Shader="Eye" Diffuse="1,1,1"/>
        <Material Name="eyeWet" Shader="Eye" Diffuse="1,1,1"/>
        </SubMaterials></Material>"#,
    );
    let iris = look(
        "shader_eyeInner",
        vec![colour(EYE_COLOR, [0, 0, 255, 255])],
        Vec::new(),
    );

    let status = iris.apply(&eyes).expect("the material changes");

    assert_eq!(diffuse(&status, 0), [0.0, 0.0, 1.0]);
    assert_eq!(diffuse(&status, 1), [1.0; 3]);
}

const DYEABLE: &str = r#"<Material><SubMaterials>
    <Material Name="hair_m" Shader="HairPBR" Diffuse="1,1,1">
      <PublicParams DyeAmount="0" BaseMelanin="0.5"/>
    </Material></SubMaterials></Material>"#;
const PLAIN: &str = r#"<Material><SubMaterials>
    <Material Name="hair_m" Shader="HairPBR" Diffuse="1,1,1">
      <PublicParams BaseMelanin="0.5"/>
    </Material></SubMaterials></Material>"#;

fn hair(melanin: f32, dye_amount: f32, dye: [u8; 4]) -> [f32; 3] {
    hair_of(DYEABLE, melanin, dye_amount, dye)
}

fn hair_of(xml: &str, melanin: f32, dye_amount: f32, dye: [u8; 4]) -> [f32; 3] {
    let cards = library(xml);
    let hair = look(
        "hair_m",
        vec![colour(HAIR_DYE_COLOR, dye)],
        vec![
            FloatParam {
                name: name_hash("BaseMelanin"),
                label: Label,
                value: melanin,
            },
            FloatParam {
                name: name_hash("DyeAmount"),
                label: Label,
                value: dye_amount,
            },
        ],
    );
    diffuse(&hair.apply(&cards).expect("the material changes"), 0)
}

#[test]
fn hair_goes_from_blond_over_brown_to_black_with_the_melanin() {
    let (blond, brown, black) = (
        hair(0.0, 0.0, [0; 4]),
        hair(0.5, 0.0, [0; 4]),
        hair(1.0, 0.0, [0; 4]),
    );

    assert!(blond[0] > brown[0] && brown[0] > black[0]);
    assert!(blond[0] > blond[2], "blond is warm");
}

fn redness_only(value: f32) -> [f32; 3] {
    let choice = look(
        "hair_m",
        Vec::new(),
        vec![FloatParam {
            name: name_hash("BaseMelaninRedness"),
            label: Label,
            value,
        }],
    );
    let status = choice
        .apply(&library(DYEABLE))
        .expect("a choice that sets only the redness is applied");
    diffuse(&status, 0)
}

#[test]
fn a_choice_that_only_sets_the_redness_is_applied() {
    let plain = redness_only(0.0);
    let red = redness_only(1.0);

    assert_ne!(
        plain, [1.0; 3],
        "the natural colour replaces the card's white"
    );
    assert!(red[0] > plain[0] && red[2] < plain[2], "{plain:?} {red:?}");
}

#[test]
fn a_full_dye_replaces_the_natural_colour() {
    let dyed = hair(1.0, 1.0, [0, 0, 255, 255]);

    assert_eq!(dyed, [0.0, 0.0, 1.0]);
}

#[test]
fn a_hair_whose_shader_has_no_dye_ignores_the_dye() {
    let plain = hair_of(PLAIN, 0.5, 1.0, [0, 0, 255, 255]);

    assert_eq!(plain, hair_of(PLAIN, 0.5, 0.0, [0; 4]));
    assert!(plain[2] < 0.5, "not blue");
}

#[test]
fn a_hair_takes_the_first_choice_when_no_sub_material_name_matches() {
    let renamed = look(
        "facial_hair_043",
        vec![colour(HAIR_DYE_COLOR, [0, 255, 0, 255])],
        vec![FloatParam {
            name: name_hash("DyeAmount"),
            label: Label,
            value: 1.0,
        }],
    );

    let status = renamed
        .apply(&library(DYEABLE))
        .expect("the hair takes the choice");

    assert_eq!(diffuse(&status, 0), [0.0, 1.0, 0.0]);
}

const LASHES: &str = r#"<Material><SubMaterials>
    <Material Name="male_lash_up_m" Shader="HairPBR" Diffuse="1,1,1">
      <PublicParams BaseMelanin="0.8" BaseMelaninRedness="0" DyeAmount="0" DyeColor="0,0,1" BaseTintColor="1,1,1"/>
    </Material>
    <Material Name="teeth" Shader="Illum" Diffuse="1,1,1"/>
    </SubMaterials></Material>"#;

#[test]
fn a_hair_the_character_chose_nothing_for_takes_the_colour_of_its_own_parameters() {
    let status = Look::natural()
        .apply(&library(LASHES))
        .expect("the lashes change");

    let [red, green, blue] = diffuse(&status, 0);
    assert!(
        red < 0.05 && green < 0.05 && blue < 0.05,
        "dark: {red} {green} {blue}"
    );
    assert!(red >= blue, "not blue: the dye amount is 0");
    assert_eq!(diffuse(&status, 1), [1.0; 3], "other shaders stay");
}

#[test]
fn a_natural_look_leaves_a_library_without_hair_alone() {
    assert!(Look::natural().apply(&library(SKIN)).is_none());
}

#[test]
fn a_choice_that_names_no_material_changes_nothing() {
    let skin = look(
        "other",
        vec![colour(BODY_COLOR, [1, 2, 3, 255])],
        Vec::new(),
    );

    assert!(skin.apply(&library(SKIN)).is_none());
    assert!(skin.apply(&MaterialStatus::Unnamed).is_none());
}

#[test]
fn look_sources_preserve_applied_rgb_and_numeric_slots_without_unused_dye() {
    let choice = look(
        "another_hair",
        vec![
            colour(HAIR_DYE_COLOR, [0, 0, 255, 255]),
            colour(HAIR_TINT_COLOR, [255, 0, 0, 255]),
        ],
        vec![
            FloatParam {
                name: name_hash("BaseMelanin"),
                label: Label,
                value: 0.5,
            },
            FloatParam {
                name: name_hash("DyeAmount"),
                label: Label,
                value: 1.0,
            },
        ],
    )
    .observed_at(4);
    let selected = choice
        .apply_observed(&library(PLAIN))
        .expect("applied hair tint");
    let paths: Vec<_> = selected
        .fields
        .get(&0)
        .expect("the first material slot")
        .iter()
        .filter_map(|input| match input {
            SourceLocation::Subresource { field, .. } => Some(field.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(
        paths,
        [
            "/character/materials/4/sub_materials/0/colors/1/rgba/0",
            "/character/materials/4/sub_materials/0/colors/1/rgba/1",
            "/character/materials/4/sub_materials/0/colors/1/rgba/2",
            "/character/materials/4/sub_materials/0/floats/0/value",
        ],
        "the unsupported dye and ignored alpha do not contribute"
    );
}

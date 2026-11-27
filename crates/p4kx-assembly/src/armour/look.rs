// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_character::{
    BODY_COLOR, EYE_COLOR, HAIR_DYE_COLOR, HAIR_TINT_COLOR, Material as Choice, SubMaterial,
    lowered_hash, name_hash,
};
use p4kx_extraction::SourceLocation;
use p4kx_model::{DIFFUSE_SLOT, Material, MaterialLibrary, MaterialStatus};

#[path = "look_inputs.rs"]
mod inputs;

use inputs::{ChosenFields, colour_of};

const SKIN_SHADER: &str = "HumanSkin_V2";
const EYE_SHADER: &str = "Eye";
const HAIR_SHADERS: [&str; 1] = ["HairPBR"];
const HAIR_TINT_PARAM: &str = "BaseTintColor";
const HAIR_DYE_PARAM: &str = "DyeColor";

pub(super) const BLOND: [f32; 3] = [0.45, 0.28, 0.10];
const BROWN: [f32; 3] = [0.08, 0.045, 0.02];
const BLACK: [f32; 3] = [0.012, 0.010, 0.009];
const AUBURN: [f32; 3] = [0.25, 0.06, 0.02];

/// The material choices a Character made for one piece: colours and numbers that replace the
/// parameters of the piece's shaders.
///
/// What a shader does with them is approximated by one colour per material, because glTF has no
/// skin, iris or hair shader: the skin tone rebases the skin texture, the eye colour tints the
/// iris, and the melanin and dye of a hair give its colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Look {
    source: Source,
    position: Option<usize>,
}

pub(super) struct AppliedLook {
    pub(super) status: MaterialStatus,
    pub(super) fields: BTreeMap<usize, Vec<SourceLocation>>,
}

/// Where the colours of a [`Look`] come from.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Source {
    /// The shader parameters of the materials themselves.
    Natural,
    /// The material choices of a Character.
    Chosen(Choice),
}

impl Look {
    /// Takes the material choices `choice` of a Character.
    #[must_use]
    pub fn new(choice: &Choice) -> Self {
        Self {
            source: Source::Chosen(choice.clone()),
            position: None,
        }
    }

    /// Returns the look of a piece the Character made no choice for: every hair material takes
    /// the colour its own shader parameters give, such as the lashes' melanin.
    #[must_use]
    pub const fn natural() -> Self {
        Self {
            source: Source::Natural,
            position: None,
        }
    }

    pub(super) const fn observed_at(mut self, position: usize) -> Self {
        self.position = Some(position);
        self
    }

    /// Returns how many textures the choice sets, which the export does not draw.
    #[must_use]
    pub fn texture_choices(&self) -> usize {
        match &self.source {
            Source::Natural => 0,
            Source::Chosen(choice) => choice
                .sub_materials
                .iter()
                .map(|sub| sub.textures.len())
                .sum(),
        }
    }

    /// Returns `status` with the look applied: a chosen look changes the materials the choices
    /// name, a natural look every hair material. `None` when the status is no library or no
    /// material changes.
    #[must_use]
    pub fn apply(&self, status: &MaterialStatus) -> Option<MaterialStatus> {
        self.apply_observed(status).map(|applied| applied.status)
    }

    pub(super) fn apply_observed(&self, status: &MaterialStatus) -> Option<AppliedLook> {
        let library = status.library()?;
        let dyes = library
            .materials
            .iter()
            .any(|material| is_hair(material) && material.public_param("DyeAmount").is_some());
        let mut fields = BTreeMap::new();
        let materials = match &self.source {
            Source::Natural => natural_materials(&library.materials, dyes),
            Source::Chosen(choice) => {
                chosen_materials(&library.materials, choice, dyes, self.position, &mut fields)
            }
        }?;
        let mut library: MaterialLibrary = library.clone();
        library.materials = materials;
        Some(AppliedLook {
            status: MaterialStatus::Loaded(library),
            fields,
        })
    }
}

/// Returns `materials` with every hair material in its own colour, or `None` when there is none.
fn natural_materials(materials: &[Material], takes_dye: bool) -> Option<Vec<Material>> {
    if !materials.iter().any(is_hair) {
        return None;
    }
    let mut painted = materials.to_vec();
    for material in painted.iter_mut().filter(|material| is_hair(material)) {
        own_hair(material, takes_dye);
    }
    Some(painted)
}

/// Returns `materials` with the choices applied to the materials they name, or `None` when none
/// changes.
fn chosen_materials(
    materials: &[Material],
    choice: &Choice,
    takes_dye: bool,
    position: Option<usize>,
    fields: &mut BTreeMap<usize, Vec<SourceLocation>>,
) -> Option<Vec<Material>> {
    let mut painted = materials.to_vec();
    let mut changed = false;
    for (slot, material) in painted.iter_mut().enumerate() {
        if let Some((index, sub)) = sub_for(choice, material) {
            let mut read = ChosenFields::new(position, index);
            if customise(material, sub, takes_dye, &mut read) {
                changed = true;
                if let Some(observed) = read.finish() {
                    fields.insert(slot, observed);
                }
            }
        }
    }
    changed.then_some(painted)
}

/// Returns the sub-material choice that `material` takes: the one named by the hash of its name.
/// A hair material takes the first choice that sets a colour or a number when none is named so,
/// because the choice of a beard or brow keeps the name of the sub-material of the style chosen
/// before.
fn sub_for<'c>(choice: &'c Choice, material: &Material) -> Option<(usize, &'c SubMaterial)> {
    let hash = lowered_hash(&material.name);
    choice
        .sub_materials
        .iter()
        .enumerate()
        .find(|(_, sub)| sub.name == hash)
        .or_else(|| {
            is_hair(material)
                .then(|| {
                    choice
                        .sub_materials
                        .iter()
                        .enumerate()
                        .find(|(_, sub)| !sub.colors.is_empty() || !sub.floats.is_empty())
                })
                .flatten()
        })
}

fn is_hair(material: &Material) -> bool {
    HAIR_SHADERS
        .iter()
        .any(|name| material.shader.eq_ignore_ascii_case(name))
}

fn has_diffuse_texture(material: &Material) -> bool {
    material
        .textures
        .iter()
        .any(|texture| texture.map == DIFFUSE_SLOT)
}

/// Applies `sub` to `material`; `takes_dye` tells whether the library of a hair takes a dye, which
/// the materials of brows do not: their shader has no dye parameters, so the choice's dye is unused.
fn customise(
    material: &mut Material,
    sub: &SubMaterial,
    takes_dye: bool,
    read: &mut ChosenFields,
) -> bool {
    if material.shader.eq_ignore_ascii_case(SKIN_SHADER) {
        let Some(tone) = colour_of(sub, BODY_COLOR) else {
            return false;
        };
        rebase(material, read.colour(tone), "SourceAverageColor");
        true
    } else if material.shader.eq_ignore_ascii_case(EYE_SHADER) {
        let Some(iris) = colour_of(sub, EYE_COLOR) else {
            return false;
        };
        rebase(material, read.colour(iris), "IrisColor");
        true
    } else if is_hair(material) {
        let dye = colour_of(sub, HAIR_DYE_COLOR);
        let tint = colour_of(sub, HAIR_TINT_COLOR);
        let natural = [name_hash("BaseMelanin"), name_hash("BaseMelaninRedness")];
        if dye.is_none()
            && tint.is_none()
            && sub
                .floats
                .iter()
                .all(|param| !natural.contains(&param.name))
        {
            return false;
        }
        let dye = dye.filter(|_| takes_dye).map(|colour| read.colour(colour));
        let tint = tint.map(|colour| read.colour(colour));
        material.diffuse = hair_colour(material, Some(sub), dye, tint, read);
        true
    } else {
        false
    }
}

/// Gives a hair material the colour its own shader parameters describe, as when the Character
/// chose nothing for it.
fn own_hair(material: &mut Material, takes_dye: bool) {
    let tint = colour_param(material, HAIR_TINT_PARAM);
    let dye = colour_param(material, HAIR_DYE_PARAM).filter(|_| takes_dye);
    material.diffuse = hair_colour(material, None, dye, tint, &mut ChosenFields::default());
}

/// Returns the colour parameter `name` of `material` with each channel clamped to 0..1; `None`
/// when it is not set or has fewer than three components.
fn colour_param(material: &Material, name: &str) -> Option<[f32; 3]> {
    material
        .public_colour(name)
        .map(|colour| colour.map(|channel| channel.clamp(0.0, 1.0)))
}

/// Moves the diffuse colour of `material` from the colour its shader parameter `source` names to
/// `target`: a material with a diffuse texture is scaled by the ratio, keeping the hue, and one
/// without takes `target`.
fn rebase(material: &mut Material, target: [f32; 3], source: &str) {
    if !has_diffuse_texture(material) {
        material.diffuse = target;
        return;
    }
    let from = material.public_param(source).unwrap_or([1.0; 3]);
    let ratio = [0, 1, 2].map(|channel| {
        let was = from.get(channel).copied().unwrap_or(1.0).max(1e-4);
        target.get(channel).copied().unwrap_or(1.0) / was
    });
    let brightest = ratio.into_iter().fold(1.0_f32, f32::max);
    material.diffuse = ratio.map(|channel| channel / brightest);
}

/// Returns the colour of a hair material: the natural colour of its melanin and redness times the
/// tint, mixed with the dye, if there is one, by the dye amount.
fn hair_colour(
    material: &Material,
    sub: Option<&SubMaterial>,
    dyed: Option<[f32; 3]>,
    tint: Option<[f32; 3]>,
    read: &mut ChosenFields,
) -> [f32; 3] {
    let melanin = read.number(sub, material, "BaseMelanin");
    let redness = read.number(sub, material, "BaseMelaninRedness");
    let tint = tint.unwrap_or([1.0; 3]);
    let natural = natural_hair(melanin, redness);
    let natural = [0, 1, 2].map(|channel| {
        natural.get(channel).copied().unwrap_or_default()
            * tint.get(channel).copied().unwrap_or(1.0)
    });
    dyed.map_or(natural, |dye| {
        lerp(natural, dye, read.number(sub, material, "DyeAmount"))
    })
}

fn lerp(from: [f32; 3], to: [f32; 3], at: f32) -> [f32; 3] {
    [0, 1, 2].map(|channel| {
        let start = from.get(channel).copied().unwrap_or_default();
        let end = to.get(channel).copied().unwrap_or_default();
        (end - start).mul_add(at, start)
    })
}

/// Returns the colour of hair with `melanin` from 0 (blond) over 0.5 (brown) to 1 (black), moved
/// towards auburn by `redness`.
fn natural_hair(melanin: f32, redness: f32) -> [f32; 3] {
    let base = if melanin < 0.5 {
        lerp(BLOND, BROWN, melanin / 0.5)
    } else {
        lerp(BROWN, BLACK, (melanin - 0.5) / 0.5)
    };
    let red = lerp(
        base,
        AUBURN.map(|channel| channel * 0.7_f32.mul_add(-melanin, 1.0)),
        0.6,
    );
    lerp(base, red, redness)
}

#[cfg(test)]
#[path = "look_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "look_natural_tests.rs"]
mod natural_tests;

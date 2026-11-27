// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use FieldKind::{Boolean, Integer, Number, Pair, Texture, Triple};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum FieldKind {
    Text,
    Boolean,
    Integer,
    Number,
    Pair,
    Triple,
    Guid,
    Texture,
}

pub(super) type Fields = &'static [(&'static str, FieldKind)];

pub(super) const LIBRARY: Fields = &[
    ("Name", FieldKind::Text),
    ("SandboxVersion", FieldKind::Text),
];

pub(super) const ITEM: Fields = &[
    ("Id", FieldKind::Guid),
    ("Library", FieldKind::Text),
    ("Name", FieldKind::Text),
    ("SubLibrary", FieldKind::Text),
    ("Type", FieldKind::Text),
    ("Enable", Boolean),
];

pub(super) const COMPONENT: Fields = &[
    ("Name", FieldKind::Text),
    ("Type", FieldKind::Text),
    ("Enable", Boolean),
];

const GROUPS: &[(&str, Fields)] = &[
    (
        "AdvancedShading",
        &[
            ("Absorptance", Number),
            ("AmbientDiffuse", Triple),
            ("AmbientDiffuse.alpha", Number),
            ("Enableadvshading", Boolean),
            ("Scattering", Number),
            ("Transparency", Number),
        ],
    ),
    (
        "CameraOrbs",
        &[
            ("Enablelensdetailshading", Boolean),
            ("Enablelenstexture", Boolean),
            ("Enableorbdetailshading", Boolean),
            ("Illumrange", Number),
            ("Lensdetailbumpiness", Number),
            ("Lensdetailshadingstrength", Number),
            ("LensTexture", Texture),
            ("Lenstexturestrength", Number),
            ("OrbTexture", Texture),
        ],
    ),
    (
        "ChromaticRing",
        &[
            ("Completionfading", Number),
            ("Completionrotation", Number),
            ("Completionspanangle", Number),
            ("EnableGradientTexture", Boolean),
            ("GradientTexture", Texture),
            ("Locktolight", Boolean),
            ("Noiseseed", Integer),
            ("Noisestrength", Number),
            ("Polygoncomplexity", Integer),
            ("Thickness", Number),
        ],
    ),
    (
        "Common",
        &[
            ("AutoRotation", Boolean),
            ("Brightness", Number),
            ("CorrectAspectRatio", Boolean),
            ("DistanceFadingfactor", Number),
            ("OcclusionInteraction", Boolean),
            ("Orbitangle", Number),
            ("Perspectivefactor", Number),
            ("Position", Pair),
            ("Size", Number),
            ("Tint", Triple),
            ("Tint.alpha", Number),
        ],
    ),
    (
        "Dynamics",
        &[
            ("Enable", Boolean),
            ("TriggerFalloff", Number),
            ("TriggerInvert", Boolean),
            ("TriggerOffset", Pair),
            ("TriggerRange", Number),
        ],
    ),
    (
        "Generator",
        &[
            ("Colorvariation", Number),
            ("Noiseseed", Integer),
            ("Numberoforbs", Integer),
            ("Rotationvariation", Number),
            ("Sizevariation", Number),
        ],
    ),
    (
        "GlobalSettings",
        &[
            ("Affectedbylightcolor", Boolean),
            ("AffectedbylightFOV", Boolean),
            ("Affectedbylightradius", Boolean),
            ("EnableHardwareOcclusion", Boolean),
            ("EnableOcclusion", Boolean),
            ("Flarefadetime", Number),
            ("MultiplyColor", Boolean),
            ("OcclusionSize", Pair),
            ("Shaftfadetime", Number),
        ],
    ),
    (
        "Glow",
        &[
            ("Focusfactor", Number),
            ("Gamma", Number),
            ("Polygonfactor", Integer),
        ],
    ),
    (
        "ImageSpaceShafts",
        &[("GoboTex", Texture), ("HighQuality", Boolean)],
    ),
    (
        "IrisShafts",
        &[
            ("BaseTex", Texture),
            ("Complexity", Integer),
            ("Enablegradienttex", Boolean),
            ("GradientTex", Texture),
            ("Noiseseed", Integer),
            ("Sizenoise", Number),
            ("Smoothness", Number),
            ("Spacingnoise", Number),
            ("Spread", Number),
            ("Spreadnoise", Number),
            ("Thickness", Number),
            ("Thicknessnoise", Number),
        ],
    ),
    ("LensGhost", &[("Texture", Texture)]),
    (
        "MultiGhost",
        &[
            ("BrightnessVariation", Number),
            ("ColorVariation", Number),
            ("Count", Integer),
            ("positionfactor", Pair),
            ("positionoffset", Pair),
            ("RandomSeed", Integer),
            ("Scatteringrange", Pair),
            ("SizeVariation", Number),
            ("Texture", Texture),
            ("X-axisnoise", Number),
            ("Y-axisnoise", Number),
        ],
    ),
    (
        "Sensor",
        &[
            ("CustomSensorVariationMap", Boolean),
            ("EffectiveSensorSize", Number),
            ("Sensorimagebrightnessvariationfactor", Number),
            ("Sensorimagesizevariationfactor", Number),
        ],
    ),
    (
        "Streaks",
        &[
            ("Enablegradienttex", Boolean),
            ("GradientTex", Texture),
            ("Noiseseed", Integer),
            ("Sizenoise", Number),
            ("Spacingnoise", Number),
            ("Streakcount", Integer),
            ("Thickness", Number),
            ("Thicknessnoise", Number),
        ],
    ),
    (
        "Transformation",
        &[("Rotation", Number), ("Scale", Pair), ("Translation", Pair)],
    ),
];

pub(super) fn group(name: &str) -> Fields {
    GROUPS
        .iter()
        .find_map(|(group, fields)| (*group == name).then_some(*fields))
        .unwrap_or(&[])
}

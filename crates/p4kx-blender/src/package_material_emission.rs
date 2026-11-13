// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigins, SourceUse, SourceUseRole};
use p4kx_model::{
    EmissionAttribute, EmissionAttributeKind, EmissionNumber, EmissionNumeric, EmissionValue,
    Material, MaterialEmission,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::PackageResult;
use crate::material_emission_origins::{self, EmissionEvidence};

/// Original material emission controls and the static preview selected from them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct PackageMaterialEmission {
    /// Original library root row holding the exact XML declarations.
    pub source_library: usize,
    /// Whether the retained material source was complete.
    pub source_complete: bool,
    /// Ordered original emission and coverage inputs.
    pub attributes: Vec<EmissionField>,
    /// Explicit authored emissive texture declarations.
    pub texture_slots: Vec<EmissionSlot>,
    /// Original mask requests an emittance map when a unique mask was present.
    pub emittance_requested: Option<bool>,
    /// Original material flags request forward rendering when a unique value was present.
    pub forward_enabled: Option<bool>,
    /// Optional Blender static preview with explicit valid authored inputs.
    pub preview: Option<StaticEmission>,
    /// Immediate origins of the scalar values and members within this description.
    #[serde(deserialize_with = "material_emission_origins::read")]
    pub field_origins: FieldOrigins,
}

/// Authored location and typed parsing of one material emission field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct EmissionField {
    /// Original element and attribute position in the referenced library.
    pub source: EmissionSource,
    /// Literal field token and non-destructive numeric interpretation.
    pub value: AuthoredEmission,
}

/// An original material input token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct EmissionSource {
    /// Original child ordinals within the retained material library root.
    pub element: Vec<usize>,
    /// Original attribute ordinal of the selected element.
    pub attribute: usize,
    /// Exact original field name including case.
    pub name: String,
    /// Exact original decoded value.
    pub raw: String,
}

/// A typed source token with explicit invalid and nonfinite states.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
#[non_exhaustive]
pub enum AuthoredEmission {
    /// The field is an authored text.
    Text,
    /// One numeric source value.
    Scalar(EmissionSample),
    /// Three authored channels without reinterpretation.
    Colour([EmissionSample; 3]),
    /// Original material flags retained without numeric truncation.
    Flags(p4kx_extraction::DecimalU64),
    /// Invalid grammar for the original field.
    Invalid,
}

/// One token in an authored emission scalar or colour.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
#[non_exhaustive]
pub enum EmissionSample {
    /// The finite stored float.
    Finite(f32),
    /// Nonfinite NaN.
    Nan,
    /// Positive infinity.
    PositiveInfinity,
    /// Negative infinity.
    NegativeInfinity,
}

/// Original explicit texture slot and filename declarations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct EmissionSlot {
    /// Map attribute and its coordinate.
    pub map: EmissionSource,
    /// File attribute and its coordinate when authored.
    pub file: Option<EmissionSource>,
}

/// Blender static emission preview parameters, derived from authored data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct StaticEmission {
    /// Original material texture-slot name.
    pub slot: String,
    /// Original material texture filename token.
    pub file: String,
    /// Finite authored linear emissive tint.
    pub tint: [f32; 3],
    /// Finite positive authored Glow strength.
    pub strength: f32,
}

impl PackageMaterialEmission {
    pub(crate) fn of(
        source: &MaterialEmission<'_>,
        source_library: usize,
        material: &Material,
        evidence: &EmissionEvidence,
    ) -> PackageResult<Self> {
        let preview = StaticEmission::of(source, material);
        let field_origins = emission_origins(source, evidence, preview.is_some())?;
        Ok(Self {
            source_library,
            source_complete: source.source_complete,
            attributes: source
                .attributes
                .iter()
                .map(|attribute| EmissionField {
                    source: EmissionSource::of(&attribute.value),
                    value: AuthoredEmission::of(attribute.value.interpreted),
                })
                .collect(),
            texture_slots: source
                .slots
                .iter()
                .map(|slot| EmissionSlot {
                    map: EmissionSource::of(&slot.map),
                    file: slot.file.as_ref().map(EmissionSource::of),
                })
                .collect(),
            emittance_requested: source.emittance_requested,
            forward_enabled: source.forward_enabled,
            preview,
            field_origins,
        })
    }
}

fn emission_origins(
    source: &MaterialEmission<'_>,
    evidence: &EmissionEvidence,
    has_preview: bool,
) -> PackageResult<FieldOrigins> {
    let mut origins = material_emission_origins::Fields::new(evidence);
    for (index, attribute) in source.attributes.iter().enumerate() {
        origins.attribute(index, attribute)?;
    }
    for (index, slot) in source.slots.iter().enumerate() {
        origins.slot(index, slot)?;
    }
    for (path, kind) in [
        ("/emittance_requested", EmissionAttributeKind::ShaderMask),
        ("/forward_enabled", EmissionAttributeKind::Flags),
    ] {
        origins.derived(
            path,
            source
                .attributes
                .iter()
                .filter(|attribute| attribute.kind == kind),
            &source.element,
            source.source_complete,
        )?;
    }
    let raw_coverage = origins.element_at(
        &source.element,
        "authoredCoverage",
        SourceUseRole::Conversion,
    )?;
    origins.bind(
        "/source_complete",
        raw_coverage.as_slice(),
        source.source_complete,
    )?;
    if has_preview {
        preview_origins(source, &mut origins)?;
    } else {
        let tested =
            origins.element_at(&source.element, "staticPreview", SourceUseRole::Selector)?;
        origins.bind("/preview", tested.as_slice(), false)?;
    }
    Ok(origins.finish())
}

pub(crate) fn relevant(source: &MaterialEmission<'_>) -> bool {
    source.forward_enabled == Some(true)
        || source.emittance_requested == Some(true)
        || !source.slots.is_empty()
        || source.attributes.iter().any(|attribute| {
            matches!(
                attribute.kind,
                EmissionAttributeKind::Glow
                    | EmissionAttributeKind::Emissive
                    | EmissionAttributeKind::AlphaGlowCutoff
                    | EmissionAttributeKind::EmissiveTint
            )
        })
}

impl StaticEmission {
    pub(crate) fn matches(&self, map: &str, file: &str) -> bool {
        self.slot == map && self.file == file
    }

    fn of(source: &MaterialEmission<'_>, material: &Material) -> Option<Self> {
        if !source.source_complete
            || source.emittance_requested != Some(true)
            || !matches!(material.shader.as_str(), "Illum" | "HardSurface")
            || material.flags.no_draw()
            || !material.layers.is_empty()
        {
            return None;
        }
        let [slot] = source.slots.as_slice() else {
            return None;
        };
        let file = slot.file.as_ref()?.raw.value.as_str();
        let map = slot.map.raw.value.as_str();
        if file.is_empty() || !unique_texture(material, map, file) {
            return None;
        }
        let EmissionNumeric::Scalar(EmissionNumber::Finite(strength)) =
            unique(source, EmissionAttributeKind::Glow)?
                .value
                .interpreted
        else {
            return None;
        };
        let EmissionNumeric::Colour(
            [
                EmissionNumber::Finite(red),
                EmissionNumber::Finite(green),
                EmissionNumber::Finite(blue),
            ],
        ) = unique(source, EmissionAttributeKind::Emissive)?
            .value
            .interpreted
        else {
            return None;
        };
        let tint = [red, green, blue];
        if strength <= 0.0 || tint.iter().any(|channel| *channel < 0.0) {
            return None;
        }
        Some(Self {
            slot: map.to_owned(),
            file: file.to_owned(),
            tint,
            strength,
        })
    }
}

fn unique<'value, 'source>(
    source: &'value MaterialEmission<'source>,
    kind: EmissionAttributeKind,
) -> Option<&'value EmissionAttribute<'source>> {
    let mut candidates = source.attributes.iter().filter(|field| field.kind == kind);
    let value = candidates.next()?;
    candidates.next().is_none().then_some(value)
}

fn preview_origins(
    source: &MaterialEmission<'_>,
    origins: &mut material_emission_origins::Fields<'_>,
) -> PackageResult<()> {
    let mut selectors = Vec::<SourceUse>::new();
    selectors.extend(origins.element_at(
        &source.element,
        "staticPreview",
        SourceUseRole::Selector,
    )?);
    for field in &source.attributes {
        if matches!(
            field.kind,
            EmissionAttributeKind::ShaderMask
                | EmissionAttributeKind::Shader
                | EmissionAttributeKind::Flags
                | EmissionAttributeKind::Glow
                | EmissionAttributeKind::Emissive
        ) {
            selectors.extend(origins.at(&field.value, SourceUseRole::Selector)?);
        }
    }
    let [slot] = source.slots.as_slice() else {
        return Err(material_emission_origins::missing_inventory());
    };
    selectors.extend(origins.at(&slot.map, SourceUseRole::Selector)?);
    if let Some(file) = &slot.file {
        selectors.extend(origins.at(file, SourceUseRole::Selector)?);
    }
    origins.bind("/preview", &selectors, true)?;
    for (path, kind) in [
        ("/preview/strength", EmissionAttributeKind::Glow),
        ("/preview/tint", EmissionAttributeKind::Emissive),
        ("/preview/tint/0", EmissionAttributeKind::Emissive),
        ("/preview/tint/1", EmissionAttributeKind::Emissive),
        ("/preview/tint/2", EmissionAttributeKind::Emissive),
    ] {
        let mut used = selectors.clone();
        if let Some(field) = unique(source, kind) {
            used.extend(origins.at(&field.value, SourceUseRole::Conversion)?);
        }
        origins.bind(path, &used, true)?;
    }
    let mut map = selectors.clone();
    map.extend(origins.at(&slot.map, SourceUseRole::Conversion)?);
    origins.bind("/preview/slot", &map, true)?;
    if let Some(file) = &slot.file {
        let mut used = selectors;
        used.extend(origins.at(file, SourceUseRole::Conversion)?);
        origins.bind("/preview/file", &used, true)?;
    }
    Ok(())
}

fn unique_texture(material: &Material, map: &str, file: &str) -> bool {
    let mut slots = material
        .textures
        .iter()
        .filter(|slot| slot.map == map && slot.file == file);
    slots.next().is_some() && slots.next().is_none()
}

impl EmissionSource {
    fn of(value: &EmissionValue<'_>) -> Self {
        Self {
            element: value.element.clone(),
            attribute: value.ordinal,
            name: value.raw.name.clone(),
            raw: value.raw.value.clone(),
        }
    }
}

impl AuthoredEmission {
    fn of(value: EmissionNumeric) -> Self {
        match value {
            EmissionNumeric::Text => Self::Text,
            EmissionNumeric::Scalar(number) => Self::Scalar(EmissionSample::of(number)),
            EmissionNumeric::Colour(numbers) => Self::Colour(numbers.map(EmissionSample::of)),
            EmissionNumeric::Flags(bits) => Self::Flags(p4kx_extraction::DecimalU64::new(bits)),
            _ => Self::Invalid,
        }
    }
}

impl EmissionSample {
    const fn of(number: EmissionNumber) -> Self {
        match number {
            EmissionNumber::Finite(value) => Self::Finite(value),
            EmissionNumber::PositiveInfinity => Self::PositiveInfinity,
            EmissionNumber::NegativeInfinity => Self::NegativeInfinity,
            _ => Self::Nan,
        }
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;

use p4kx_model::{
    MaterialLibrary, MaterialReadField, MaterialReadObservation, MaterialReadUse,
    MaterialSourceLimits,
};

use crate::material_observer_baseline::TestResult;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Observed {
    pub(crate) material: usize,
    pub(crate) field: MaterialReadField,
    pub(crate) indices: Vec<usize>,
    pub(crate) source_field: String,
    pub(crate) use_kind: MaterialReadUse,
}

impl From<MaterialReadObservation<'_>> for Observed {
    fn from(event: MaterialReadObservation<'_>) -> Self {
        Self {
            material: event.material,
            field: event.field,
            indices: event.indices.to_vec(),
            source_field: event.source_field.to_owned(),
            use_kind: event.use_kind,
        }
    }
}

pub(crate) fn capture(
    bytes: &[u8],
    limits: MaterialSourceLimits,
) -> TestResult<(MaterialLibrary, Vec<Observed>)> {
    let mut events = Vec::new();
    let library = MaterialLibrary::parse_with_observer(
        "synthetic/observer",
        bytes,
        limits,
        || Ok::<(), Infallible>(()),
        |event| {
            events.push(event.into());
            Ok(())
        },
    )?;
    Ok((library, events))
}

pub(crate) fn palette(events: &[Observed]) -> impl Iterator<Item = &Observed> {
    events
        .iter()
        .filter(|event| event.field == MaterialReadField::Palette)
}

pub(crate) fn field<'a>(
    events: &'a [Observed],
    output: MaterialReadField,
    source: &str,
) -> TestResult<&'a Observed> {
    let mut matching = events
        .iter()
        .filter(|event| event.field == output && event.source_field == source);
    let event = matching.next().ok_or("required source observation")?;
    if matching.next().is_some() {
        return Err("the source observation must be unique".into());
    }
    Ok(event)
}

pub(crate) fn assert_default_fields(events: &[&Observed]) {
    for field in [
        MaterialReadField::Name,
        MaterialReadField::Flags,
        MaterialReadField::Shader,
        MaterialReadField::SurfaceType,
        MaterialReadField::StringGenMask,
        MaterialReadField::Diffuse,
        MaterialReadField::Specular,
        MaterialReadField::Emissive,
        MaterialReadField::Shininess,
        MaterialReadField::Opacity,
        MaterialReadField::AlphaTest,
        MaterialReadField::Glow,
        MaterialReadField::Palette,
    ] {
        assert!(
            events.iter().any(|event| event.field == field),
            "the engine-default disposition selects {field:?}"
        );
    }
}

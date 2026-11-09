// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::mtl_number;
use crate::mtl_read_site::{FieldElement, ReadSite};
use crate::mtl_xml::XmlElement;
use crate::{MaterialFlags, MaterialReadField, MaterialReadObservation, MaterialReadUse};

pub(crate) struct DirectRead<'observer, Element, Observer> {
    pub(crate) element: Element,
    pub(crate) site: ReadSite,
    pub(crate) observer: &'observer mut Observer,
    pub(crate) shader_use: MaterialReadUse,
    pub(crate) mask_use: MaterialReadUse,
}

pub(crate) struct Identity {
    pub(crate) name: String,
    pub(crate) flags: MaterialFlags,
    pub(crate) shader: String,
    pub(crate) surface_type: String,
    pub(crate) string_gen_mask: String,
}

pub(crate) struct Lighting {
    pub(crate) diffuse: [f32; 3],
    pub(crate) specular: [f32; 3],
    pub(crate) emissive: [f32; 3],
    pub(crate) shininess: f32,
    pub(crate) opacity: f32,
    pub(crate) alpha_test: f32,
    pub(crate) glow: f32,
}

const DEFAULT_DIFFUSE: [f32; 3] = [1.0; 3];
const DEFAULT_SPECULAR: [f32; 3] = [0.04; 3];
const DEFAULT_SHININESS: f32 = 10.0;

impl<'source, Element: XmlElement<'source>, Observer> DirectRead<'_, Element, Observer> {
    pub(crate) fn identity<Interrupted>(&mut self) -> Result<Identity, Interrupted>
    where
        Observer: FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
    {
        Ok(Identity {
            name: self.text(MaterialReadField::Name, "Name")?,
            flags: self.flags()?,
            shader: self.text(MaterialReadField::Shader, "Shader")?,
            surface_type: self.text(MaterialReadField::SurfaceType, "SurfaceType")?,
            string_gen_mask: self.text(MaterialReadField::StringGenMask, "StringGenMask")?,
        })
    }

    pub(crate) fn lighting<Interrupted>(&mut self) -> Result<Lighting, Interrupted>
    where
        Observer: FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
    {
        Ok(Lighting {
            diffuse: self.colour(MaterialReadField::Diffuse, "Diffuse", DEFAULT_DIFFUSE)?,
            specular: self.colour(MaterialReadField::Specular, "Specular", DEFAULT_SPECULAR)?,
            emissive: self.colour(MaterialReadField::Emissive, "Emissive", [0.0; 3])?,
            shininess: self.number(MaterialReadField::Shininess, "Shininess", DEFAULT_SHININESS)?,
            opacity: self.number(MaterialReadField::Opacity, "Opacity", 1.0)?,
            alpha_test: self.number(MaterialReadField::AlphaTest, "AlphaTest", 0.0)?,
            glow: self.number(MaterialReadField::Glow, "Glow", 0.0)?,
        })
    }

    pub(crate) fn text<Interrupted>(
        &mut self,
        field: MaterialReadField,
        name: &str,
    ) -> Result<String, Interrupted>
    where
        Observer: FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
    {
        let value = self.element.attribute(name);
        let use_kind = if value.is_some() {
            MaterialReadUse::Authored
        } else {
            MaterialReadUse::Selected
        };
        self.emit(field, name, use_kind)?;
        match field {
            MaterialReadField::Shader => self.shader_use = use_kind,
            MaterialReadField::StringGenMask => self.mask_use = use_kind,
            _ => {}
        }
        Ok(value.unwrap_or_default().to_owned())
    }

    pub(crate) fn number<Interrupted>(
        &mut self,
        field: MaterialReadField,
        name: &str,
        default: f32,
    ) -> Result<f32, Interrupted>
    where
        Observer: FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
    {
        let value = self.element.attribute(name).and_then(mtl_number::scalar);
        self.emit(field, name, numeric_use(value.is_some()))?;
        Ok(value.unwrap_or(default))
    }

    pub(crate) fn colour<Interrupted>(
        &mut self,
        field: MaterialReadField,
        name: &str,
        default: [f32; 3],
    ) -> Result<[f32; 3], Interrupted>
    where
        Observer: FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
    {
        let value = self.element.attribute(name).and_then(mtl_number::vector);
        self.emit(field, name, numeric_use(value.is_some()))?;
        Ok(value.unwrap_or(default))
    }

    pub(crate) fn flags<Interrupted>(&mut self) -> Result<MaterialFlags, Interrupted>
    where
        Observer: FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
    {
        let value = self
            .element
            .attribute("MtlFlags")
            .and_then(|value| value.trim().parse::<i64>().ok());
        self.emit(
            MaterialReadField::Flags,
            "MtlFlags",
            numeric_use(value.is_some()),
        )?;
        let bits = value.map_or(0, |value| value & 0xFFFF_FFFF);
        Ok(MaterialFlags(u32::try_from(bits).unwrap_or(0)))
    }

    fn emit<Interrupted>(
        &mut self,
        field: MaterialReadField,
        name: &str,
        use_kind: MaterialReadUse,
    ) -> Result<(), Interrupted>
    where
        Observer: FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
    {
        self.site
            .at(FieldElement::Material, field, name, use_kind, self.observer)
    }
}

pub(crate) const fn numeric_use(valid: bool) -> MaterialReadUse {
    if valid {
        MaterialReadUse::Interpreted
    } else {
        MaterialReadUse::Selected
    }
}

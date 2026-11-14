// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::mtl_number;
use crate::mtl_read_site::{FieldElement, FieldToken, ReadSite};
use crate::mtl_xml::XmlElement;
use crate::{MaterialFlags, MaterialReadField, MaterialReadObservation, MaterialReadUse};

pub(crate) struct DirectRead<'observer, 'source, Element, Observer> {
    pub(crate) element: Element,
    pub(crate) site: ReadSite,
    pub(crate) observer: &'observer mut Observer,
    pub(crate) shader: FieldToken<'source>,
    pub(crate) mask: FieldToken<'source>,
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

impl<'source, Element: XmlElement<'source>, Observer> DirectRead<'_, 'source, Element, Observer> {
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
        name: &'source str,
    ) -> Result<String, Interrupted>
    where
        Observer: FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
    {
        let attribute = self.element.positioned_attribute(name);
        let use_kind = if attribute.is_some() {
            MaterialReadUse::Authored
        } else {
            MaterialReadUse::Selected
        };
        let token = FieldToken::attribute(name, attribute.as_ref(), use_kind);
        self.emit(field, token)?;
        match field {
            MaterialReadField::Shader => self.shader = token,
            MaterialReadField::StringGenMask => self.mask = token,
            _ => {}
        }
        Ok(attribute.map_or("", |attribute| attribute.value).to_owned())
    }

    pub(crate) fn number<Interrupted>(
        &mut self,
        field: MaterialReadField,
        name: &'source str,
        default: f32,
    ) -> Result<f32, Interrupted>
    where
        Observer: FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
    {
        let attribute = self.element.positioned_attribute(name);
        let value = attribute
            .as_ref()
            .and_then(|attribute| mtl_number::scalar(attribute.value));
        self.emit(
            field,
            FieldToken::attribute(name, attribute.as_ref(), numeric_use(value.is_some())),
        )?;
        Ok(value.unwrap_or(default))
    }

    pub(crate) fn colour<Interrupted>(
        &mut self,
        field: MaterialReadField,
        name: &'source str,
        default: [f32; 3],
    ) -> Result<[f32; 3], Interrupted>
    where
        Observer: FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
    {
        let attribute = self.element.positioned_attribute(name);
        let value = attribute
            .as_ref()
            .and_then(|attribute| mtl_number::vector(attribute.value));
        self.emit(
            field,
            FieldToken::attribute(name, attribute.as_ref(), numeric_use(value.is_some())),
        )?;
        Ok(value.unwrap_or(default))
    }

    pub(crate) fn flags<Interrupted>(&mut self) -> Result<MaterialFlags, Interrupted>
    where
        Observer: FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
    {
        let attribute = self.element.positioned_attribute("MtlFlags");
        let value = attribute
            .as_ref()
            .and_then(|attribute| attribute.value.trim().parse::<i64>().ok());
        self.emit(
            MaterialReadField::Flags,
            FieldToken::attribute("MtlFlags", attribute.as_ref(), numeric_use(value.is_some())),
        )?;
        let bits = value.map_or(0, |value| value & 0xFFFF_FFFF);
        Ok(MaterialFlags(u32::try_from(bits).unwrap_or(0)))
    }

    fn emit<Interrupted>(
        &mut self,
        field: MaterialReadField,
        token: FieldToken<'_>,
    ) -> Result<(), Interrupted>
    where
        Observer: FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
    {
        self.site
            .token(FieldElement::Material, field, token, self.observer)
    }
}

pub(crate) const fn numeric_use(valid: bool) -> MaterialReadUse {
    if valid {
        MaterialReadUse::Interpreted
    } else {
        MaterialReadUse::Selected
    }
}

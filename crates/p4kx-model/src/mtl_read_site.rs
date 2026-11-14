// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::mtl_xml::PositionedAttribute;
use crate::{MaterialReadField, MaterialReadObservation, MaterialReadUse};

const FIELDS: [MaterialReadField; 13] = [
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
];

#[derive(Clone, Copy)]
pub(crate) enum MaterialElement {
    Root,
    Submaterial { list: usize, ordinal: usize },
}

#[derive(Clone, Copy)]
pub(crate) enum FieldElement {
    Material,
    Child { ordinal: usize },
    Grandchild { parent: usize, ordinal: usize },
}

#[derive(Clone, Copy)]
pub(crate) struct FieldToken<'a> {
    pub(crate) name: &'a str,
    pub(crate) ordinal: Option<usize>,
    pub(crate) use_kind: MaterialReadUse,
}

impl<'source> FieldToken<'source> {
    pub(crate) fn attribute(
        name: &'source str,
        found: Option<&PositionedAttribute<'source>>,
        use_kind: MaterialReadUse,
    ) -> Self {
        Self {
            name,
            ordinal: found.map(|attribute| attribute.ordinal),
            use_kind,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct ReadSite {
    pub(crate) material: usize,
    pub(crate) element: MaterialElement,
}

impl ReadSite {
    pub(crate) const ROOT: Self = Self {
        material: 0,
        element: MaterialElement::Root,
    };

    pub(crate) fn at<Interrupted>(
        self,
        element: FieldElement,
        field: MaterialReadField,
        source_field: &str,
        use_kind: MaterialReadUse,
        observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
    ) -> Result<(), Interrupted> {
        self.token(
            element,
            field,
            FieldToken {
                name: source_field,
                ordinal: None,
                use_kind,
            },
            observer,
        )
    }

    pub(crate) fn token<Interrupted>(
        self,
        element: FieldElement,
        field: MaterialReadField,
        token: FieldToken<'_>,
        observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
    ) -> Result<(), Interrupted> {
        let mut emit = |indices: &[usize]| {
            observer(MaterialReadObservation {
                material: self.material,
                field,
                indices,
                source_field: token.name,
                attribute_ordinal: token.ordinal,
                use_kind: token.use_kind,
            })
        };
        match (self.element, element) {
            (MaterialElement::Root, FieldElement::Material) => emit(&[]),
            (MaterialElement::Root, FieldElement::Child { ordinal }) => emit(&[ordinal]),
            (MaterialElement::Root, FieldElement::Grandchild { parent, ordinal }) => {
                emit(&[parent, ordinal])
            }
            (MaterialElement::Submaterial { list, ordinal }, FieldElement::Material) => {
                emit(&[list, ordinal])
            }
            (
                MaterialElement::Submaterial { list, ordinal },
                FieldElement::Child { ordinal: child },
            ) => emit(&[list, ordinal, child]),
            (
                MaterialElement::Submaterial { list, ordinal },
                FieldElement::Grandchild {
                    parent,
                    ordinal: child,
                },
            ) => emit(&[list, ordinal, parent, child]),
        }
    }

    pub(crate) fn disposition<Interrupted>(
        self,
        name: &str,
        observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
    ) -> Result<(), Interrupted> {
        self.at(
            FieldElement::Material,
            MaterialReadField::Row,
            "elementKind",
            MaterialReadUse::Selected,
            observer,
        )?;
        let root = Self {
            element: MaterialElement::Root,
            ..self
        };
        root.at(
            FieldElement::Material,
            MaterialReadField::Row,
            "SubMaterials",
            MaterialReadUse::Selected,
            observer,
        )?;
        for field in FIELDS {
            observer(MaterialReadObservation {
                material: self.material,
                field,
                indices: &[],
                source_field: "SubMaterials",
                attribute_ordinal: None,
                use_kind: MaterialReadUse::Selected,
            })?;
            self.at(
                FieldElement::Material,
                field,
                name,
                MaterialReadUse::Selected,
                observer,
            )?;
        }
        Ok(())
    }
}

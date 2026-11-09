// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

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
pub(crate) struct ReadSite {
    pub(crate) material: usize,
    pub(crate) element: MaterialElement,
}

impl ReadSite {
    pub(crate) fn at<Interrupted>(
        self,
        element: FieldElement,
        field: MaterialReadField,
        source_field: &str,
        use_kind: MaterialReadUse,
        observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
    ) -> Result<(), Interrupted> {
        let mut emit = |indices: &[usize]| {
            observer(MaterialReadObservation {
                material: self.material,
                field,
                indices,
                source_field,
                use_kind,
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
        for field in FIELDS {
            observer(MaterialReadObservation {
                material: self.material,
                field,
                indices: &[],
                source_field: "SubMaterials",
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

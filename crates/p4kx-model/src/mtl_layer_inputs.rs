// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{collect_site, token_site};
use crate::mtl_raw;
use crate::mtl_read_site::{FieldElement, ReadSite};
use crate::mtl_source_error::MaterialReadError;
use crate::mtl_xml::XmlElement;
use crate::{MaterialLayer, MaterialReadField, MaterialReadObservation};

pub(crate) fn layers<'source, Interrupted>(
    element: impl XmlElement<'source>,
    site: ReadSite,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<Vec<MaterialLayer>, MaterialReadError<Interrupted>> {
    collect_site(
        site,
        FieldElement::Material,
        MaterialReadField::Layers,
        "MatLayers",
        observer,
    )?;
    let mut layers = Vec::new();
    let lists = element
        .children()
        .enumerate()
        .filter(|(_, child)| child.name().eq_ignore_ascii_case("MatLayers"));
    for (parent, list) in lists {
        collect_site(
            site,
            FieldElement::Child { ordinal: parent },
            MaterialReadField::Layers,
            "layerElements",
            observer,
        )?;
        for (ordinal, layer) in list
            .children()
            .enumerate()
            .filter(|(_, child)| child.name().eq_ignore_ascii_case("Layer"))
        {
            let index = layers.len();
            let input = FieldElement::Grandchild { parent, ordinal };
            collect_site(
                site,
                input,
                MaterialReadField::Layer { index },
                "Layer",
                observer,
            )?;
            observe_layer_attributes(layer, site, input, index, observer)?;
            let layer = MaterialLayer {
                attributes: mtl_raw::attributes(layer),
                surface: None,
                palette_entry: None,
            };
            for (name, field) in [
                ("Name", MaterialReadField::LayerName { index }),
                ("Path", MaterialReadField::LayerPath { index }),
                ("PaletteTint", MaterialReadField::LayerPaletteTint { index }),
            ] {
                if let Some((attribute, name, _)) = layer.positioned_attribute(name) {
                    token_site(site, input, field, name, attribute, observer)?;
                }
            }
            layers.push(layer);
        }
    }
    Ok(layers)
}

fn observe_layer_attributes<'source, Interrupted>(
    layer: impl XmlElement<'source>,
    site: ReadSite,
    input: FieldElement,
    index: usize,
    observer: &mut impl FnMut(MaterialReadObservation<'_>) -> Result<(), Interrupted>,
) -> Result<(), MaterialReadError<Interrupted>> {
    for (attribute, (name, _)) in layer.attributes().enumerate() {
        token_site(
            site,
            input,
            MaterialReadField::LayerAttribute { index, attribute },
            name,
            attribute,
            observer,
        )?;
    }
    Ok(())
}

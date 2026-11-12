// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::mtl_source::ElementAuthored;
use crate::mtl_xml::XmlElement;
use crate::{LayerSurface, TintEntry};
use crate::{MaterialError, PaletteSlot};

/// The deepest nesting of elements the reader keeps in a [`RawElement`].
pub const MAX_RAW_DEPTH: usize = 8;

/// The most elements the reader keeps in the raw elements of one material.
pub const MAX_RAW_ELEMENTS: usize = 4096;

/// An attribute list as the file gives it: names and values in file order.
pub type Attributes = Vec<(String, String)>;

/// An XML element of a material as the file has it, for parts of the format the reader does not
/// interpret.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RawElement {
    /// The element's name.
    pub name: String,
    /// The element's attributes in file order.
    pub attributes: Attributes,
    /// The element's child elements in file order.
    pub children: Vec<Self>,
    /// Ordered source syntax supplement, independent of the existing interpreted fields.
    pub authored: ElementAuthored,
}

/// One layer of a material's `MatLayers` list: another material blended into the surface.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct MaterialLayer {
    /// All attributes of the `Layer` element in file order, e.g. `Name`, `Path`, `TintColor`,
    /// `PaletteTint`, `UVTiling`, `GlossMult`.
    pub attributes: Attributes,
    /// The surface values of the layer's own `.mtl` file; `None` until
    /// [`MaterialStatus::load`](crate::MaterialStatus::load) has read it, and when it is missing.
    pub surface: Option<LayerSurface>,
    /// The palette entry a [`TintPalette`](crate::TintPalette) gave the layer, for a layer with a
    /// `PaletteTint` from 1 to 3.
    pub palette_entry: Option<TintEntry>,
}

impl MaterialLayer {
    /// Returns the value of the attribute `name`, ignoring case.
    #[must_use]
    pub fn attribute(&self, name: &str) -> Option<&str> {
        self.positioned_attribute(name).map(|(_, _, value)| value)
    }

    pub(crate) fn positioned_attribute(&self, name: &str) -> Option<(usize, &str, &str)> {
        self.attributes
            .iter()
            .enumerate()
            .find(|(_, (known, _))| known.eq_ignore_ascii_case(name))
            .map(|(index, (name, value))| (index, name.as_str(), value.as_str()))
    }

    /// Returns the palette entry the layer takes: that of its `PaletteTint` from 1 to 3.
    #[must_use]
    pub fn palette_slot(&self) -> Option<PaletteSlot> {
        let number = self.attribute("PaletteTint")?.trim().parse::<u32>().ok()?;
        PaletteSlot::of_layer(number)
    }
}

pub(crate) struct Budget {
    left: usize,
    elements_cut: bool,
    depth_cut: bool,
}

impl Budget {
    pub(crate) const fn new() -> Self {
        Self {
            left: MAX_RAW_ELEMENTS,
            elements_cut: false,
            depth_cut: false,
        }
    }

    const fn take(&mut self) -> bool {
        if let Some(left) = self.left.checked_sub(1) {
            self.left = left;
            true
        } else {
            self.elements_cut = true;
            false
        }
    }

    /// Returns what the limits cut off, for [`Material::truncated`](crate::Material::truncated).
    pub(crate) fn cut(&self) -> Vec<String> {
        let mut cut = Vec::new();
        if self.depth_cut {
            cut.push(format!("raw nesting beyond {MAX_RAW_DEPTH}"));
        }
        if self.elements_cut {
            cut.push(format!("elements beyond {MAX_RAW_ELEMENTS}"));
        }
        cut
    }
}

pub(crate) fn attributes<'a>(element: impl XmlElement<'a>) -> Attributes {
    element
        .attributes()
        .map(|(name, value)| (name.to_owned(), value.to_owned()))
        .collect()
}

pub(crate) fn raw<'a>(
    element: impl XmlElement<'a>,
    depth: usize,
    budget: &mut Budget,
    ordinal: Option<usize>,
) -> Result<Option<RawElement>, MaterialError> {
    if !budget.take() {
        return Ok(None);
    }
    let mut children = Vec::new();
    let children_in_order = element.children().enumerate();
    for (position, child) in children_in_order {
        if depth >= MAX_RAW_DEPTH {
            budget.depth_cut = true;
            break;
        }
        if let Some(child) = raw(child, depth + 1, budget, Some(position))? {
            children.push(child);
        }
    }
    Ok(Some(RawElement {
        name: element.name().to_owned(),
        attributes: attributes(element),
        children,
        authored: element.syntax()?.into_owned(ordinal),
    }))
}
pub(crate) fn other_elements<'a>(
    element: impl XmlElement<'a>,
    budget: &mut Budget,
) -> Result<Vec<RawElement>, MaterialError> {
    const INTERPRETED: [&str; 4] = ["SubMaterials", "Textures", "MatLayers", "PublicParams"];
    let mut elements = Vec::new();
    let children_in_order = element.children().enumerate();
    for (ordinal, child) in children_in_order {
        if INTERPRETED
            .iter()
            .any(|known| child.name().eq_ignore_ascii_case(known))
        {
            continue;
        }
        if let Some(child) = raw(child, 1, budget, Some(ordinal))? {
            elements.push(child);
        }
    }
    Ok(elements)
}

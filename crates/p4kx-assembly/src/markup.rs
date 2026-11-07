// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::Xml;

use crate::ValueOrigin;

/// The deepest element nesting that is read.
pub(crate) const MAX_ELEMENT_DEPTH: usize = p4kx_cryxml::MAX_TEXT_DEPTH;

/// The most elements one implementation and its patch may hold.
pub(crate) const MAX_ELEMENTS: usize = 1 << 21;

/// An owned XML element of a text or `CryXml` document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Element {
    pub(crate) name: String,
    pub(crate) attributes: Vec<(String, String)>,
    pub(crate) origins: Vec<(String, ValueOrigin)>,
    pub(crate) children: Vec<Self>,
}

impl Element {
    /// Creates an element read from the base file: every attribute is [`ValueOrigin::Base`].
    pub(crate) const fn read(
        name: String,
        attributes: Vec<(String, String)>,
        children: Vec<Self>,
    ) -> Self {
        Self {
            name,
            attributes,
            origins: Vec::new(),
            children,
        }
    }

    /// Returns where the attribute `name` got its value, ignoring ASCII case.
    pub(crate) fn origin_of(&self, name: &str) -> ValueOrigin {
        self.origins
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map_or(ValueOrigin::Base, |(_, origin)| *origin)
    }

    /// Records that the attribute `name` got its value from `origin`.
    pub(crate) fn mark(&mut self, name: &str, origin: ValueOrigin) {
        if let Some((_, old)) = self
            .origins
            .iter_mut()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
        {
            *old = origin;
        } else {
            self.origins.push((name.to_owned(), origin));
        }
    }

    /// Records `origin` for every attribute of the element and of everything below it.
    pub(crate) fn mark_all(&mut self, origin: ValueOrigin) {
        self.origins = self
            .attributes
            .iter()
            .map(|(name, _)| (name.clone(), origin))
            .collect();
        for child in &mut self.children {
            child.mark_all(origin);
        }
    }

    /// Returns the value of the attribute `name`, ignoring ASCII case.
    pub(crate) fn attribute(&self, name: &str) -> Option<&str> {
        self.attributes
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// Sets the attribute `name`, ignoring ASCII case, or appends it.
    pub(crate) fn set_attribute(&mut self, name: &str, value: &str) {
        if let Some((_, old)) = self
            .attributes
            .iter_mut()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
        {
            value.clone_into(old);
        } else {
            self.attributes.push((name.to_owned(), value.to_owned()));
        }
    }

    /// Returns the first child element named `name`, ignoring ASCII case.
    pub(crate) fn child(&self, name: &str) -> Option<&Self> {
        self.children
            .iter()
            .find(|child| child.name.eq_ignore_ascii_case(name))
    }

    /// Returns the child elements named `name`, ignoring ASCII case.
    pub(crate) fn children_named<'s, 'n>(
        &'s self,
        name: &'n str,
    ) -> impl Iterator<Item = &'s Self> + use<'s, 'n> {
        self.children
            .iter()
            .filter(move |child| child.name.eq_ignore_ascii_case(name))
    }
}

/// The most attributes the elements of one implementation and its patch may hold.
const MAX_ATTRIBUTES: usize = 1 << 22;

/// The least bytes of names and values a document may be read into.
const MIN_BYTES: usize = 1 << 16;

/// How many bytes of records, names and values the tree may hold per byte of input: an element
/// record alone is several times the size of the shortest markup that makes it.
const INPUT_FACTOR: usize = 16;

const ELEMENT_BYTES: usize = size_of::<Element>();

const ATTRIBUTE_BYTES: usize = size_of::<(String, String)>();

/// What reading and patching one implementation may spend: elements, attributes, and bytes of
/// element and attribute records, names and values held, the last at [`INPUT_FACTOR`] times the
/// input read.
#[derive(Debug, Clone)]
pub(crate) struct Budget {
    elements: usize,
    exhausted: bool,
    attributes: usize,
    bytes: usize,
}

#[derive(Clone, Copy)]
enum Unit {
    Elements,
    Attributes,
    Bytes,
}

impl Budget {
    /// Creates a budget for reading `input` bytes.
    pub(crate) fn new(input: usize) -> Self {
        Self {
            elements: MAX_ELEMENTS,
            exhausted: false,
            attributes: MAX_ATTRIBUTES,
            bytes: input.saturating_mul(INPUT_FACTOR).max(MIN_BYTES),
        }
    }

    /// Adds the allowance for reading `input` more bytes.
    pub(crate) const fn grow(&mut self, input: usize) {
        self.bytes = self
            .bytes
            .saturating_add(input.saturating_mul(INPUT_FACTOR));
    }

    #[cfg(test)]
    pub(crate) fn without_elements() -> Self {
        Self {
            elements: 0,
            ..Self::new(0)
        }
    }

    pub(crate) const fn exhausted(&self) -> bool {
        self.exhausted
    }

    fn take(&mut self, unit: Unit, count: usize) -> Result<(), String> {
        let (left, what) = match unit {
            Unit::Elements => (&mut self.elements, "elements"),
            Unit::Attributes => (&mut self.attributes, "attributes"),
            Unit::Bytes => (&mut self.bytes, "bytes of names and values"),
        };
        let Some(remaining) = left.checked_sub(count) else {
            self.exhausted = true;
            return Err(format!("it needs more {what} than are allowed"));
        };
        *left = remaining;
        Ok(())
    }
    fn element(&mut self, name: &str) -> Result<(), String> {
        self.take(Unit::Elements, 1)?;
        self.take(Unit::Bytes, name.len().saturating_add(ELEMENT_BYTES))
    }

    fn attribute(&mut self, name: &str, value: &str) -> Result<(), String> {
        self.take(Unit::Attributes, 1)?;
        self.take(
            Unit::Bytes,
            name.len()
                .saturating_add(value.len())
                .saturating_add(ATTRIBUTE_BYTES),
        )
    }

    /// Returns why `root` cannot be copied once more within the budget, without spending it.
    pub(crate) fn check_copy(&self, root: &Element) -> Result<(), String> {
        self.clone().charge_copy(root)
    }

    /// Charges the cost of copying `element` without its children.
    pub(crate) fn charge_shallow(&mut self, element: &Element) -> Result<(), String> {
        self.element(&element.name)?;
        element
            .attributes
            .iter()
            .try_for_each(|(name, value)| self.attribute(name, value))
    }

    /// Charges the cost of copying `element` with everything below it.
    pub(crate) fn charge_copy(&mut self, element: &Element) -> Result<(), String> {
        self.element(&element.name)?;
        for (name, value) in &element.attributes {
            self.attribute(name, value)?;
        }
        element
            .children
            .iter()
            .try_for_each(|child| self.charge_copy(child))
    }

    /// Charges the cost of setting an attribute on an element with `attributes` attributes.
    pub(crate) fn charge_set(
        &mut self,
        attributes: usize,
        name: &str,
        value: &str,
    ) -> Result<(), String> {
        self.take(Unit::Attributes, attributes.max(1))?;
        self.take(
            Unit::Bytes,
            name.len()
                .saturating_add(value.len())
                .saturating_add(ATTRIBUTE_BYTES),
        )
    }
}

/// Returns the depth of `element`: 1 for an element without children.
pub(crate) fn depth(element: &Element) -> usize {
    element
        .children
        .iter()
        .map(depth)
        .max()
        .unwrap_or(0)
        .saturating_add(1)
}

/// Returns the error for a document nested deeper than [`MAX_ELEMENT_DEPTH`].
pub(crate) fn too_deep() -> String {
    format!("its elements are nested deeper than {MAX_ELEMENT_DEPTH}")
}

/// Reads the root element of a text XML or `CryXml` document, spending `budget`.
pub(crate) fn parse(bytes: &[u8], budget: &mut Budget) -> Result<Element, String> {
    match p4kx_cryxml::read(bytes).map_err(|error| error.to_string())? {
        Xml::CryXml(document) => from_cry(document.root(), 0, budget),
        Xml::Text(text) => {
            let text = std::str::from_utf8(text).map_err(|error| error.to_string())?;
            let text = text.trim_start_matches('\u{feff}');
            p4kx_cryxml::check_text_depth(text).map_err(|error| error.to_string())?;
            let document = roxmltree::Document::parse(text).map_err(|error| error.to_string())?;
            from_text(document.root_element(), 0, budget)
        }
    }
}

fn from_cry(
    element: p4kx_cryxml::Element<'_>,
    depth: usize,
    budget: &mut Budget,
) -> Result<Element, String> {
    if depth >= MAX_ELEMENT_DEPTH {
        return Err(too_deep());
    }
    budget.element(element.name())?;
    let mut attributes = Vec::new();
    for attribute in element.attributes() {
        budget.attribute(attribute.name(), attribute.value())?;
        attributes.push((attribute.name().to_owned(), attribute.value().to_owned()));
    }
    let children = element
        .children()
        .map(|child| from_cry(child, depth.saturating_add(1), budget))
        .collect::<Result<_, _>>()?;
    Ok(Element::read(
        element.name().to_owned(),
        attributes,
        children,
    ))
}

fn from_text(
    element: roxmltree::Node<'_, '_>,
    depth: usize,
    budget: &mut Budget,
) -> Result<Element, String> {
    if depth >= MAX_ELEMENT_DEPTH {
        return Err(too_deep());
    }
    budget.element(element.tag_name().name())?;
    let mut attributes = Vec::new();
    for attribute in element.attributes() {
        budget.attribute(attribute.name(), attribute.value())?;
        attributes.push((attribute.name().to_owned(), attribute.value().to_owned()));
    }
    let children = element
        .children()
        .filter(roxmltree::Node::is_element)
        .map(|child| from_text(child, depth.saturating_add(1), budget))
        .collect::<Result<_, _>>()?;
    Ok(Element::read(
        element.tag_name().name().to_owned(),
        attributes,
        children,
    ))
}

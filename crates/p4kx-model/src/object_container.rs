// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{CryXmlError, Xml};
use thiserror::Error;

use crate::Transform;
use crate::mtl_xml::XmlElement;

/// The most nested containers one object container lists.
pub const MAX_CHILD_CONTAINERS: usize = 1 << 14;

/// The most `.soc` files one object container streams.
pub const MAX_STREAMED_SOCS: usize = 1 << 12;

/// The deepest `StreamingOCs` nesting that is read.
pub const MAX_STREAMING_DEPTH: usize = 16;

/// A failure to read an object container description.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ObjectContainerError {
    /// The file is no readable `CryXml`.
    #[error("the file is not valid CryXml")]
    CryXml(#[from] CryXmlError),
    /// The file is no well-formed text XML.
    #[error("the file is not well-formed XML")]
    Xml(#[from] roxmltree::Error),
    /// The text XML nests its elements too deeply to be read.
    #[error("the file is not readable XML: {0}")]
    TooDeep(#[from] p4kx_cryxml::TextTooDeep),
    /// The root element is no `ObjectContainer`.
    #[error("the root element is {found}, not ObjectContainer")]
    NotContainer {
        /// The root element's name.
        found: String,
    },
    /// The file lists more of something than is read.
    #[error("the file lists more than {limit} {what}")]
    TooMany {
        /// What there is too much of.
        what: &'static str,
        /// The most read.
        limit: usize,
    },
}

/// A container nested in an [`ObjectContainer`] with its placement: a `Child` of its
/// `ChildObjectContainers`.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct ChildContainer {
    /// The name of the nested container, from which its file is found.
    pub name: String,
    /// The class of the Entity that holds the container, empty when not given.
    pub class: String,
    /// The name of the Entity that holds the container, empty when not given.
    pub entity_name: String,
    /// The position in the parent container's space, in metres; zero when the `pos` attribute is
    /// missing or not three numbers.
    pub position: [f64; 3],
    /// The rotation in the parent's space as a quaternion `[w, x, y, z]`; the identity when the
    /// `rot` attribute is missing or not four numbers.
    pub rotation: [f64; 4],
    /// Whether the container is stored in a file of its own.
    pub external: bool,
    /// Whether the `pos` or `rot` attribute is present but not made of numbers, in which case
    /// the placement is not the container's own.
    pub malformed: bool,
}

impl ChildContainer {
    /// Returns the transform from the nested container's space to its parent's, or `None` when
    /// the position or rotation is not finite or the rotation has no length.
    #[must_use]
    pub fn transform(&self) -> Option<Transform> {
        Transform::from_quaternion(self.rotation, self.position)
    }
}

/// The box that an object container declares around its entities: its `minBounds` and
/// `maxBounds`, in metres in the container's space.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct ContainerBounds {
    /// The corner with the smallest coordinates.
    pub min: [f64; 3],
    /// The corner with the largest coordinates.
    pub max: [f64; 3],
}

impl ContainerBounds {
    /// Returns whether `point` lies in the box grown by `margin` metres on every side.
    #[must_use]
    pub fn contains(&self, point: [f32; 3], margin: f64) -> bool {
        point
            .iter()
            .zip(self.min.iter().zip(&self.max))
            .all(|(value, (low, high))| {
                let value = f64::from(*value);
                value >= low - margin && value <= high + margin
            })
    }
}

/// The description of an object container: the `<name>.xml` file of a socpak.
///
/// It names the containers nested in it with their placement (`ChildObjectContainers`) and the
/// `.soc` files that hold its entities and placed models (`StreamingOCs`, which nest); the
/// `.soc` files share the container's space.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct ObjectContainer {
    /// The container's name.
    pub name: String,
    /// The declared bounds, `None` when the description gives none or they are not three finite
    /// numbers each.
    pub bounds: Option<ContainerBounds>,
    /// The nested containers in file order.
    pub children: Vec<ChildContainer>,
    /// The `.soc` files of the container in file order, parents before the files nested in them,
    /// as the description spells them.
    pub streams: Vec<String>,
}

impl ObjectContainer {
    /// Reads the description in `bytes`, text XML or `CryXml`.
    ///
    /// # Errors
    ///
    /// Returns [`ObjectContainerError`] when the file is neither, its root is no
    /// `ObjectContainer` or it lists more containers or `.soc` files than are read.
    pub fn parse(bytes: &[u8]) -> Result<Self, ObjectContainerError> {
        match p4kx_cryxml::read(bytes)? {
            Xml::CryXml(document) => container(document.root()),
            Xml::Text(text) => {
                let text = String::from_utf8_lossy(text);
                p4kx_cryxml::check_text_depth(&text)?;
                let document = roxmltree::Document::parse(&text)?;
                container(document.root_element())
            }
        }
    }
}

fn container<'a>(root: impl XmlElement<'a>) -> Result<ObjectContainer, ObjectContainerError> {
    if !root.name().eq_ignore_ascii_case("ObjectContainer") {
        return Err(ObjectContainerError::NotContainer {
            found: root.name().to_owned(),
        });
    }
    let mut result = ObjectContainer {
        name: root.attribute("name").unwrap_or_default().to_owned(),
        bounds: declared_bounds(root.attribute("minBounds"), root.attribute("maxBounds")),
        ..ObjectContainer::default()
    };
    for section in root.children() {
        if section.name().eq_ignore_ascii_case("ChildObjectContainers") {
            for child in section.children() {
                if !child.name().eq_ignore_ascii_case("Child") {
                    continue;
                }
                if result.children.len() == MAX_CHILD_CONTAINERS {
                    return Err(ObjectContainerError::TooMany {
                        what: "nested containers",
                        limit: MAX_CHILD_CONTAINERS,
                    });
                }
                result.children.push(child_container(child));
            }
        } else if section.name().eq_ignore_ascii_case("StreamingOCs") {
            streams(section, 0, &mut result.streams)?;
        }
    }
    Ok(result)
}

fn streams<'a>(
    section: impl XmlElement<'a>,
    depth: usize,
    out: &mut Vec<String>,
) -> Result<(), ObjectContainerError> {
    if depth >= MAX_STREAMING_DEPTH {
        return Err(ObjectContainerError::TooMany {
            what: "levels of streamed containers",
            limit: MAX_STREAMING_DEPTH,
        });
    }
    for oc in section.children() {
        if !oc.name().eq_ignore_ascii_case("OC") {
            continue;
        }
        if out.len() == MAX_STREAMED_SOCS {
            return Err(ObjectContainerError::TooMany {
                what: ".soc files",
                limit: MAX_STREAMED_SOCS,
            });
        }
        if let Some(name) = oc.attribute("name").filter(|name| !name.is_empty()) {
            out.push(name.to_owned());
        }
        for nested in oc.children() {
            if nested.name().eq_ignore_ascii_case("StreamingOCs") {
                streams(nested, depth + 1, out)?;
            }
        }
    }
    Ok(())
}

/// Returns whether an authored Child value declares a separately stored container.
#[must_use]
pub fn is_external_child_value(value: &str) -> bool {
    value == "1" || value.eq_ignore_ascii_case("true")
}

fn child_container<'a>(child: impl XmlElement<'a>) -> ChildContainer {
    let (pos, rot) = (child.attribute("pos"), child.attribute("rot"));
    let position = numbers::<3>(pos);
    let rotation = numbers::<4>(rot);
    ChildContainer {
        malformed: (pos.is_some() && position.is_none()) || (rot.is_some() && rotation.is_none()),
        name: child.attribute("name").unwrap_or_default().to_owned(),
        class: child.attribute("class").unwrap_or_default().to_owned(),
        entity_name: child.attribute("entityName").unwrap_or_default().to_owned(),
        position: position.unwrap_or([0.0; 3]),
        rotation: rotation.unwrap_or([1.0, 0.0, 0.0, 0.0]),
        external: child
            .attribute("external")
            .is_some_and(is_external_child_value),
    }
}

fn declared_bounds(min: Option<&str>, max: Option<&str>) -> Option<ContainerBounds> {
    let (min, max) = (numbers::<3>(min)?, numbers::<3>(max)?);
    min.iter()
        .chain(&max)
        .all(|value| value.is_finite())
        .then_some(ContainerBounds { min, max })
}

fn numbers<const N: usize>(text: Option<&str>) -> Option<[f64; N]> {
    let mut values = [0.0; N];
    let mut parts = text?.split(',');
    for value in &mut values {
        *value = parts.next()?.trim().parse().ok()?;
    }
    parts.next().is_none().then_some(values)
}

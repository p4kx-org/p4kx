// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml;

use crate::starmap_transform::{Rotation, Transform};

/// An element of text XML or `CryXml`, read alike.
pub(crate) trait Node<'d>: Copy {
    fn original(self) -> Root<'d>;
    fn name(self) -> &'d str;
    fn attributes(self) -> impl Iterator<Item = (&'d str, &'d str)>;
    fn children(self) -> impl Iterator<Item = Self>;
}

impl<'d> Node<'d> for cryxml::Element<'d> {
    fn original(self) -> Root<'d> {
        Root::Cry(self)
    }

    fn name(self) -> &'d str {
        cryxml::Element::name(self)
    }

    fn attributes(self) -> impl Iterator<Item = (&'d str, &'d str)> {
        cryxml::Element::attributes(self).map(|attribute| (attribute.name(), attribute.value()))
    }

    fn children(self) -> impl Iterator<Item = Self> {
        cryxml::Element::children(self)
    }
}

impl<'d> Node<'d> for roxmltree::Node<'d, '_> {
    fn original(self) -> Root<'d> {
        Root::Text(self)
    }

    fn name(self) -> &'d str {
        self.tag_name().name()
    }

    fn attributes(self) -> impl Iterator<Item = (&'d str, &'d str)> {
        roxmltree::Node::attributes(&self).map(|attribute| (attribute.name(), attribute.value()))
    }

    fn children(self) -> impl Iterator<Item = Self> {
        roxmltree::Node::children(&self).filter(roxmltree::Node::is_element)
    }
}

/// Why bytes are no XML document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub(crate) enum XmlError {
    /// The bytes are neither `CryXml` nor well-formed text XML.
    #[error("is no XML")]
    Malformed,
    /// Text XML that is not UTF-8.
    #[error("is no UTF-8")]
    NotUtf8,
    /// Text XML that nests deeper than [`cryxml::check_text_depth`] allows.
    #[error("nests too deep")]
    TooDeep,
}

/// A parsed XML document, `CryXml` or text, read alike through [`Root`].
pub(crate) enum Document<'b> {
    /// A parsed `CryXml`.
    Cry(cryxml::Document<'b>),
    /// Parsed text XML.
    Text(roxmltree::Document<'b>),
}

impl<'b> Document<'b> {
    /// Parses `bytes` as `CryXml` or as text XML, ignoring a byte order mark.
    ///
    /// # Errors
    ///
    /// Returns why `bytes` are no document: not XML, not UTF-8, or nested too deep.
    pub(crate) fn parse(bytes: &'b [u8]) -> Result<Self, XmlError> {
        match cryxml::read(bytes).map_err(|_| XmlError::Malformed)? {
            cryxml::Xml::CryXml(document) => Ok(Self::Cry(document)),
            cryxml::Xml::Text(text) => {
                let text = std::str::from_utf8(text)
                    .map_err(|_| XmlError::NotUtf8)?
                    .trim_start_matches('\u{feff}');
                cryxml::check_text_depth(text).map_err(|_| XmlError::TooDeep)?;
                roxmltree::Document::parse(text)
                    .map(Self::Text)
                    .map_err(|_| XmlError::Malformed)
            }
        }
    }

    /// Returns the original node position only for a node of this actual parse.
    pub(crate) fn original_index(&self, node: Root<'_>) -> Option<u32> {
        match (self, node) {
            (Self::Cry(document), Root::Cry(element)) if element.is_from(document) => {
                Some(element.index())
            }
            (Self::Text(document), Root::Text(node)) if std::ptr::eq(document, node.document()) => {
                Some(node.id().get())
            }
            _ => None,
        }
    }

    /// Returns the root element.
    pub(crate) fn root(&self) -> Root<'_> {
        match self {
            Self::Cry(document) => Root::Cry(document.root()),
            Self::Text(document) => Root::Text(document.root_element()),
        }
    }
}

/// An element of a [`Document`] of either format.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Root<'d> {
    /// An element of a `CryXml` document.
    Cry(cryxml::Element<'d>),
    /// An element of a text XML document.
    Text(roxmltree::Node<'d, 'd>),
}

impl<'d> Node<'d> for Root<'d> {
    fn original(self) -> Self {
        self
    }

    fn name(self) -> &'d str {
        match self {
            Self::Cry(element) => Node::name(element),
            Self::Text(node) => Node::name(node),
        }
    }

    fn attributes(self) -> impl Iterator<Item = (&'d str, &'d str)> {
        match self {
            Self::Cry(element) => Either::Left(Node::attributes(element)),
            Self::Text(node) => Either::Right(Node::attributes(node)),
        }
    }

    fn children(self) -> impl Iterator<Item = Self> {
        match self {
            Self::Cry(element) => Either::Left(Node::children(element).map(Self::Cry)),
            Self::Text(node) => Either::Right(Node::children(node).map(Self::Text)),
        }
    }
}

/// An iterator that is one of two kinds.
enum Either<L, R> {
    Left(L),
    Right(R),
}

impl<T, L: Iterator<Item = T>, R: Iterator<Item = T>> Iterator for Either<L, R> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        match self {
            Self::Left(left) => left.next(),
            Self::Right(right) => right.next(),
        }
    }
}

/// Returns the value of the attribute `name` of `node`, ignoring ASCII case.
pub(crate) fn attribute<'d>(node: impl Node<'d>, name: &str) -> Option<&'d str> {
    node.attributes()
        .find(|(have, _)| have.eq_ignore_ascii_case(name))
        .map(|(_, value)| value)
}

/// Returns the element named `name` that is a child of `node`, or else the first one below it in
/// document order, ignoring ASCII case.
pub(crate) fn first_below<'d, N: Node<'d>>(node: N, name: &str) -> Option<N> {
    if let Some(direct) = node
        .children()
        .find(|child| child.name().eq_ignore_ascii_case(name))
    {
        return Some(direct);
    }
    let mut pending: Vec<N> = node.children().collect();
    pending.reverse();
    while let Some(next) = pending.pop() {
        if next.name().eq_ignore_ascii_case(name) {
            return Some(next);
        }
        let mark = pending.len();
        pending.extend(next.children());
        if let Some(added) = pending.get_mut(mark..) {
            added.reverse();
        }
    }
    None
}

/// The original Child coordinates and the transform used by the walk.
#[derive(Clone, Copy)]
pub(crate) struct AuthoredPose {
    pub(crate) position: Option<[f64; 3]>,
    pub(crate) rotation: Option<[f64; 4]>,
    unit: Rotation,
}

impl AuthoredPose {
    pub(crate) fn transform(self) -> Transform {
        Transform {
            position: self.position.unwrap_or([0.0; 3]),
            rotation: self.unit,
        }
    }
}

/// Reads Child positions as x, y, z metres and quaternion rotations as w, x, y, z.
pub(crate) fn pose<'d>(child: impl Node<'d>) -> Option<AuthoredPose> {
    let position = match attribute(child, "pos") {
        Some(text) => Some(numbers::<3>(text)?),
        None => None,
    };
    let (rotation, unit) = match attribute(child, "rot") {
        Some(text) => {
            let parts = numbers::<4>(text)?;
            (Some(parts), Rotation::from_parts(parts)?)
        }
        None => (None, Rotation::IDENTITY),
    };
    Some(AuthoredPose {
        position,
        rotation,
        unit,
    })
}

fn numbers<const N: usize>(text: &str) -> Option<[f64; N]> {
    let mut parts = text.split(',');
    let mut values = [0.0; N];
    for slot in &mut values {
        let value: f64 = parts.next()?.trim().parse().ok()?;
        if !value.is_finite() {
            return None;
        }
        *slot = value;
    }
    parts.next().is_none().then_some(values)
}

#[cfg(test)]
mod tests {
    use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};

    use super::*;

    fn names(root: Root<'_>) -> Vec<&str> {
        root.children().map(Node::name).collect()
    }

    fn assert_reads_alike(document: &Document<'_>) {
        let root = document.root();
        assert_eq!(root.name(), "Root");
        assert_eq!(attribute(root, "KEY"), Some("v"), "ignoring case");
        assert_eq!(names(root), ["A", "B"]);
    }

    #[test]
    fn text_and_cryxml_read_alike() {
        let text = br#"<Root key="v"><A/><B/></Root>"#;
        assert_reads_alike(&Document::parse(text).expect("text"));
        let cry = TestElement::new("Root")
            .attribute("key", "v")
            .child(TestElement::new("A"))
            .child(TestElement::new("B"))
            .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
        assert_reads_alike(&Document::parse(&cry).expect("cryxml"));
    }

    #[test]
    fn a_byte_order_mark_is_ignored() {
        let text = "\u{feff}<Root key=\"v\"><A/><B/></Root>";
        assert_reads_alike(&Document::parse(text.as_bytes()).expect("text"));
    }

    #[test]
    fn bytes_that_are_no_xml_say_why() {
        assert_eq!(Document::parse(b"").err(), Some(XmlError::Malformed));
        assert_eq!(
            Document::parse(b"\x00\x01").err(),
            Some(XmlError::Malformed)
        );
        assert_eq!(
            Document::parse(b"<a><b></a>").err(),
            Some(XmlError::Malformed)
        );
        assert_eq!(
            Document::parse(b"<a b=\"\xff\"/>").err(),
            Some(XmlError::NotUtf8)
        );
        let deep = "<a>".repeat(100_000);
        assert_eq!(
            Document::parse(deep.as_bytes()).err(),
            Some(XmlError::TooDeep)
        );
    }
}

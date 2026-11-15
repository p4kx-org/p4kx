// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::sync::Arc;

use p4kx_cryxml::{CryXmlError, Xml};
use thiserror::Error;

use crate::Transform;
use crate::mtl_xml::XmlElement;
use crate::prefab_source::{PositionedPrefabLibrary, PrefabLibrarySource, PrefabSource};
use crate::soc_entities::entity_transform;

/// The most prefabs one library lists.
pub const MAX_LIBRARY_PREFABS: usize = 1 << 16;

/// The most objects one library lists, nested groups counted.
pub const MAX_LIBRARY_OBJECTS: usize = 1 << 21;

/// The deepest nesting of groups in a prefab that is read; deeper objects are left out.
pub const MAX_GROUP_DEPTH: usize = 64;

/// A failure to read a prefab library.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PrefabError {
    /// The file is no readable `CryXml`.
    #[error("the file is not valid CryXml")]
    CryXml(#[from] CryXmlError),
    /// The file is no well-formed text XML.
    #[error("the file is not well-formed XML")]
    Xml(#[from] roxmltree::Error),
    /// The text XML nests its elements too deeply to be read.
    #[error("the file is not readable XML: {0}")]
    TooDeep(#[from] p4kx_cryxml::TextTooDeep),
    /// The root element is no `PrefabsLibrary` and holds no `Prefab`.
    #[error("the root element is {found}, which is no prefab library")]
    NotLibrary {
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

/// The prefab an object of type `Prefab` instances.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PrefabRef {
    /// The `PrefabGUID` as the file spells it, empty when it names none.
    pub guid: String,
    /// The `PrefabName`: the library name and the prefab name joined by `.`, e.g.
    /// `pu/derelict/components.visual.flare`; empty when it names none.
    pub name: String,
}

/// An object of a prefab, placed in the prefab's space.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct PrefabObject {
    /// The object's `Name`.
    pub name: String,
    /// The object's `Type`, e.g. `Entity`, `Brush`, `GeomEntity` or `Prefab`.
    pub kind: String,
    /// The object's `Id`, an editor GUID as the file spells it.
    pub id: String,
    /// The `EntityClass`, empty when the object has none.
    pub class: String,
    /// The geometry file the object names as the file spells it, below `Data`: the `Prefab` of a
    /// brush, the `Geometry` of a geometry entity or the geometry resource of an entity.
    pub model: Option<String>,
    /// The transform from the object's space to the prefab's, `CryEngine` coordinates, Z up and
    /// in metres, with the transforms of the groups around the object already applied.
    pub transform: Transform,
    /// The prefab the object instances, for an object of type `Prefab`.
    pub nested: Option<PrefabRef>,
    /// Whether the object is attached to another object (`Parent` other than its group, or an
    /// `AttachmentTarget`), in which case its transform is relative to that attachment and not
    /// to the prefab.
    pub attached: bool,
}

/// A prefab: the objects of a reusable piece of a level.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Prefab {
    /// The prefab's `Name` within its library.
    pub name: String,
    /// The prefab's `Id`, an editor GUID as the file spells it.
    pub id: String,
    /// The `Library` the prefab names.
    pub library: String,
    /// The objects in file order, groups flattened into them and left out themselves.
    pub objects: Vec<PrefabObject>,
    /// The number of objects left out because their transform is not usable or they sit too
    /// deep in groups.
    pub invalid: usize,
}

/// A prefab library: an XML file below `DataPrefabs` whose root, a `PrefabsLibrary` or an element
/// named after the library, holds the prefabs.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct PrefabLibrary {
    /// The root's `Name`, or the root element's own name when it has none.
    pub name: String,
    /// The prefabs in file order.
    pub prefabs: Vec<Prefab>,
}

impl PrefabLibrary {
    /// Reads the library in `bytes`, text XML or `CryXml`.
    ///
    /// # Errors
    ///
    /// Returns [`PrefabError`] when the file is neither, its root is no `PrefabsLibrary` or it
    /// lists more prefabs or objects than are read.
    pub fn parse(bytes: &[u8]) -> Result<Self, PrefabError> {
        Ok(parse_library(bytes, SourceCapture::ValuesOnly)?.library)
    }

    /// Reads the library and the positions of its original fields.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::parse`].
    pub fn parse_positioned(bytes: &[u8]) -> Result<PositionedPrefabLibrary, PrefabError> {
        parse_library(bytes, SourceCapture::Positions)
    }

    /// Returns the first prefab named `name`, ignoring case.
    #[must_use]
    pub fn find(&self, name: &str) -> Option<&Prefab> {
        self.prefabs
            .iter()
            .find(|prefab| prefab.name.eq_ignore_ascii_case(name))
    }
}

#[derive(Clone, Copy)]
enum SourceCapture {
    ValuesOnly,
    Positions,
}

impl SourceCapture {
    const fn omit_if_repaired(self, repaired: bool) -> Self {
        if repaired { Self::ValuesOnly } else { self }
    }
}

fn parse_library(
    bytes: &[u8],
    capture: SourceCapture,
) -> Result<PositionedPrefabLibrary, PrefabError> {
    match p4kx_cryxml::read(bytes)? {
        Xml::CryXml(document) => library(
            document.root(),
            capture.omit_if_repaired(document.is_lossy()),
        ),
        Xml::Text(text) => {
            let text = String::from_utf8_lossy(text);
            p4kx_cryxml::check_text_depth(&text)?;
            let document = roxmltree::Document::parse(&text)?;
            library(
                document.root_element(),
                capture.omit_if_repaired(matches!(text, Cow::Owned(_))),
            )
        }
    }
}

fn library<'a>(
    root: impl XmlElement<'a>,
    capture: SourceCapture,
) -> Result<PositionedPrefabLibrary, PrefabError> {
    let is_library = root.name().eq_ignore_ascii_case("PrefabsLibrary")
        || root
            .children()
            .any(|element| element.name().eq_ignore_ascii_case("Prefab"));
    if !is_library {
        return Err(PrefabError::NotLibrary {
            found: root.name().to_owned(),
        });
    }
    let authored_name = root
        .attribute("Name")
        .map(str::trim)
        .filter(|name| !name.is_empty());
    let mut library = PrefabLibrary {
        name: authored_name.unwrap_or_else(|| root.name()).to_owned(),
        prefabs: Vec::new(),
    };
    let mut source = match capture {
        SourceCapture::Positions => PrefabLibrarySource::new(root, authored_name),
        SourceCapture::ValuesOnly => None,
    };
    let mut total_objects = 0_usize;
    for element in root.children() {
        if !element.name().eq_ignore_ascii_case("Prefab") {
            continue;
        }
        if library.prefabs.len() >= MAX_LIBRARY_PREFABS {
            return Err(PrefabError::TooMany {
                what: "prefabs",
                limit: MAX_LIBRARY_PREFABS,
            });
        }
        let objects = child(element, "Objects");
        library
            .prefabs
            .push(read_prefab(element, objects, &mut total_objects)?);
        if let Some(source) = &mut source {
            source.prefabs.push(PrefabSource::new(element, objects));
        }
    }
    Ok(PositionedPrefabLibrary { library, source })
}

fn attribute<'a>(element: impl XmlElement<'a>, name: &str) -> String {
    element
        .attribute(name)
        .unwrap_or_default()
        .trim()
        .to_owned()
}

fn child<'a, E: XmlElement<'a>>(parent: E, name: &str) -> Option<E> {
    parent
        .children()
        .find(|element| element.name().eq_ignore_ascii_case(name))
}

fn is_object<'a>(element: impl XmlElement<'a>) -> bool {
    element.name().eq_ignore_ascii_case("Object")
}

struct Pending<E> {
    element: E,
    group: Transform,
    group_id: Arc<str>,
    depth: usize,
}

fn push_members<'a, E: XmlElement<'a>>(
    stack: &mut Vec<Pending<E>>,
    parent: E,
    group: Transform,
    group_id: &Arc<str>,
    depth: usize,
) {
    let at = stack.len();
    stack.extend(
        parent
            .children()
            .filter(|member| is_object(*member))
            .map(|element| Pending {
                element,
                group,
                group_id: Arc::clone(group_id),
                depth,
            }),
    );
    if let Some(added) = stack.get_mut(at..) {
        added.reverse();
    }
}

fn read_prefab<'a, E: XmlElement<'a>>(
    element: E,
    objects: Option<E>,
    total: &mut usize,
) -> Result<Prefab, PrefabError> {
    let mut prefab = Prefab {
        name: attribute(element, "Name"),
        id: attribute(element, "Id"),
        library: attribute(element, "Library"),
        objects: Vec::new(),
        invalid: 0,
    };
    let Some(objects) = objects else {
        return Ok(prefab);
    };
    let mut stack = Vec::new();
    push_members(&mut stack, objects, Transform::IDENTITY, &Arc::from(""), 0);
    while let Some(pending) = stack.pop() {
        *total += 1;
        if *total > MAX_LIBRARY_OBJECTS {
            return Err(PrefabError::TooMany {
                what: "objects",
                limit: MAX_LIBRARY_OBJECTS,
            });
        }
        let Some(local) = entity_transform(pending.element) else {
            prefab.invalid += 1 + count_below(pending.element);
            continue;
        };
        let world = pending.group.then(&local);
        let kind = attribute(pending.element, "Type");
        let id: Arc<str> = Arc::from(attribute(pending.element, "Id"));
        if let Some(members) = child(pending.element, "Objects") {
            if pending.depth + 1 >= MAX_GROUP_DEPTH {
                prefab.invalid += count_below(pending.element);
            } else {
                push_members(&mut stack, members, world, &id, pending.depth + 1);
            }
        }
        if !kind.eq_ignore_ascii_case("Group") {
            prefab.objects.push(read_object(
                pending.element,
                kind,
                &id,
                world,
                &pending.group_id,
            ));
        }
    }
    Ok(prefab)
}

fn count_below<'a>(element: impl XmlElement<'a>) -> usize {
    let mut count = 0;
    let mut stack = vec![element];
    while let Some(parent) = stack.pop() {
        for members in parent
            .children()
            .filter(|node| node.name().eq_ignore_ascii_case("Objects"))
        {
            for member in members.children().filter(|node| is_object(*node)) {
                count += 1;
                stack.push(member);
            }
        }
    }
    count
}

fn read_object<'a>(
    element: impl XmlElement<'a>,
    kind: String,
    id: &str,
    transform: Transform,
    group_id: &str,
) -> PrefabObject {
    let nested = kind.eq_ignore_ascii_case("Prefab").then(|| PrefabRef {
        guid: attribute(element, "PrefabGUID"),
        name: attribute(element, "PrefabName"),
    });
    let parent = attribute(element, "Parent");
    let attached = (!parent.is_empty() && !parent.eq_ignore_ascii_case(group_id))
        || !attribute(element, "AttachmentTarget").is_empty();
    PrefabObject {
        name: attribute(element, "Name"),
        model: model(element, &kind),
        kind,
        id: id.to_owned(),
        class: attribute(element, "EntityClass"),
        transform,
        nested,
        attached,
    }
}

fn model<'a>(element: impl XmlElement<'a>, kind: &str) -> Option<String> {
    let named = |name: &str| {
        element
            .attribute(name)
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .map(str::to_owned)
    };
    if kind.eq_ignore_ascii_case("Brush") {
        return named("Prefab");
    }
    named("Geometry").or_else(|| resource_geometry(element))
}

fn resource_geometry<'a, E: XmlElement<'a>>(element: E) -> Option<String> {
    let mut node = child(
        child(element, "PropertiesDataCore")?,
        "EntityGeometryResource",
    )?;
    for _ in 0..3 {
        node = child(node, "Geometry")?;
    }
    node.attribute("path")
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
}

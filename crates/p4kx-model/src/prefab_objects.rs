// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use crate::mtl_xml::XmlElement;
use crate::soc_entities::entity_transform;
use crate::{
    MAX_GROUP_DEPTH, MAX_LIBRARY_OBJECTS, Prefab, PrefabAttachmentSource,
    PrefabAttributeObservation, PrefabError, PrefabModelSource, PrefabObject, PrefabObjectSource,
    PrefabRef, PrefabReferenceSource, PrefabResourceChild, PrefabResourceEnd,
    PrefabTransformAncestor, PrefabTransformInputs, Transform,
};

fn attribute<'a>(element: impl XmlElement<'a>, name: &str) -> String {
    attribute_text(element, name).to_owned()
}

fn attribute_text<'a>(element: impl XmlElement<'a>, name: &str) -> &'a str {
    element.attribute(name).unwrap_or_default().trim()
}

pub(crate) fn child<'a, E: XmlElement<'a>>(parent: E, name: &str) -> Option<E> {
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
    groups: Option<Arc<PrefabTransformAncestor>>,
}

fn push_members<'a, E: XmlElement<'a>>(
    stack: &mut Vec<Pending<E>>,
    parent: E,
    group: Transform,
    group_id: &Arc<str>,
    depth: usize,
    groups: Option<&Arc<PrefabTransformAncestor>>,
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
                groups: groups.cloned(),
            }),
    );
    if let Some(added) = stack.get_mut(at..) {
        added.reverse();
    }
}

pub(crate) fn read<'a, E: XmlElement<'a>>(
    element: E,
    objects: Option<E>,
    total: &mut usize,
    mut sources: Option<&mut Vec<PrefabObjectSource>>,
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
    push_members(
        &mut stack,
        objects,
        Transform::IDENTITY,
        &Arc::from(""),
        0,
        None,
    );
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
                let groups = sources.as_ref().map(|_| group_source(&pending));
                push_members(
                    &mut stack,
                    members,
                    world,
                    &id,
                    pending.depth + 1,
                    groups.as_ref(),
                );
            }
        }
        if !kind.eq_ignore_ascii_case("Group") {
            prefab.objects.push(read_object(
                pending,
                kind,
                &id,
                world,
                sources.as_deref_mut(),
            ));
        }
    }
    Ok(prefab)
}

fn group_source<'a, E: XmlElement<'a>>(pending: &Pending<E>) -> Arc<PrefabTransformAncestor> {
    Arc::new(PrefabTransformAncestor {
        transform: PrefabTransformInputs::new(pending.element),
        id: pending.element.attribute_position("Id"),
        parent: pending.groups.clone(),
    })
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

fn read_object<'a, E: XmlElement<'a>>(
    pending: Pending<E>,
    kind: String,
    id: &str,
    transform: Transform,
    sources: Option<&mut Vec<PrefabObjectSource>>,
) -> PrefabObject {
    let element = pending.element;
    let nested = kind.eq_ignore_ascii_case("Prefab").then(|| PrefabRef {
        guid: attribute(element, "PrefabGUID"),
        name: attribute(element, "PrefabName"),
    });
    let parent = attribute_text(element, "Parent");
    let outside_group = !parent.is_empty() && !parent.eq_ignore_ascii_case(&pending.group_id);
    let attached = outside_group || !attribute_text(element, "AttachmentTarget").is_empty();
    let geometry = Geometry::read(element, &kind);
    if let Some(sources) = sources {
        sources.push(PrefabObjectSource {
            name: element.attribute_position("Name"),
            kind: element.attribute_position("Type"),
            id: element.attribute_position("Id"),
            class: element.attribute_position("EntityClass"),
            model: geometry.source(element),
            reference: nested.as_ref().map(|_| PrefabReferenceSource {
                name: element.attribute_position("PrefabName"),
                guid: element.attribute_position("PrefabGUID"),
            }),
            attachment: PrefabAttachmentSource {
                parent: element.attribute_position("Parent"),
                compared_group_id: !parent.is_empty() && pending.groups.is_some(),
                attachment_target: (!outside_group).then(|| {
                    PrefabAttributeObservation::observed(
                        element.attribute_position("AttachmentTarget"),
                    )
                }),
            },
            transform: PrefabTransformInputs::new(element),
            groups: pending.groups,
        });
    }
    PrefabObject {
        name: attribute(element, "Name"),
        model: geometry.path.map(str::to_owned),
        kind,
        id: id.to_owned(),
        class: attribute(element, "EntityClass"),
        transform,
        nested,
        attached,
    }
}

enum GeometrySelection<E> {
    Brush,
    Direct,
    Resource {
        element: E,
        missing: Option<PrefabResourceChild>,
    },
}

struct Geometry<'a, E> {
    path: Option<&'a str>,
    selection: GeometrySelection<E>,
}

impl<'a, E: XmlElement<'a>> Geometry<'a, E> {
    fn read(element: E, kind: &str) -> Self {
        use PrefabResourceChild::{EntityGeometryResource, Geometry, PropertiesDataCore};

        if kind.eq_ignore_ascii_case("Brush") {
            return Self {
                path: nonempty(attribute_text(element, "Prefab")),
                selection: GeometrySelection::Brush,
            };
        }
        if let Some(path) = nonempty(attribute_text(element, "Geometry")) {
            return Self {
                path: Some(path),
                selection: GeometrySelection::Direct,
            };
        }
        let mut node = element;
        for key in [
            PropertiesDataCore,
            EntityGeometryResource,
            Geometry,
            Geometry,
            Geometry,
        ] {
            let Some(child) = child(node, key.tag()) else {
                return Self {
                    path: None,
                    selection: GeometrySelection::Resource {
                        element: node,
                        missing: Some(key),
                    },
                };
            };
            node = child;
        }
        Self {
            path: nonempty(attribute_text(node, "path")),
            selection: GeometrySelection::Resource {
                element: node,
                missing: None,
            },
        }
    }

    fn source(&self, object: E) -> PrefabModelSource {
        match self.selection {
            GeometrySelection::Brush => {
                PrefabModelSource::Brush(object.attribute_position("Prefab"))
            }
            GeometrySelection::Direct => {
                PrefabModelSource::Geometry(object.attribute_position("Geometry"))
            }
            GeometrySelection::Resource { element, missing } => PrefabModelSource::Resource {
                geometry: object.attribute_position("Geometry"),
                end: missing.map_or_else(
                    || PrefabResourceEnd::Path {
                        element: element.position(),
                        position: element.attribute_position("path"),
                    },
                    |child| PrefabResourceEnd::MissingChild {
                        parent: element.position(),
                        child,
                    },
                ),
            },
        }
    }
}

fn nonempty(text: &str) -> Option<&str> {
    (!text.is_empty()).then_some(text)
}

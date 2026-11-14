// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::VecDeque;
use std::io;

use p4kx_extraction::{DataForgeEntry, DataForgeOrigin, SourceLocation, StructValue, Value};
use serde_json::{Value as Json, json};

use crate::player_source_classes::Classes;

use super::{Physical, Sources};

pub(super) struct Actor {
    physical: Physical,
    record: u32,
    geometry: Vec<Geometry>,
}

struct Geometry {
    component: usize,
    slots: Vec<usize>,
    path: Json,
    tags: Json,
}

pub(super) fn observe(
    data: Option<&DataForgeEntry>,
    classes: Classes<'_, '_>,
) -> io::Result<Actor> {
    let Some(DataForgeOrigin::Archive(original)) = data.map(DataForgeEntry::origin) else {
        return Err(io::Error::other("selected DataForge origin unavailable"));
    };
    let Some((entities, _)) = classes else {
        return Err(io::Error::other("selected Entity index unavailable"));
    };
    let [actor] = entities.named_class_candidates("Player") else {
        return Err(io::Error::other("expected one selected actor"));
    };
    let tree = actor
        .tree()
        .map_err(|_| io::Error::other("cannot read actor tree"))?;
    let Some(Value::Array(components)) = tree.field("Components") else {
        return Err(io::Error::other("actor component array unavailable"));
    };
    let mut geometry = Vec::new();
    for (component, raw) in components.iter().enumerate() {
        if let Value::Struct(value) = raw
            && value.type_name() == "SGeometryResourceParams"
            && let Some(root) = structure(value, "Geometry")
        {
            collect(root, component, &mut geometry);
        }
    }
    Ok(Actor {
        physical: Physical::of(original),
        record: actor.index(),
        geometry,
    })
}

fn structure<'tree, 'source>(
    node: &'tree StructValue<'source>,
    name: &str,
) -> Option<&'tree StructValue<'source>> {
    match node.field(name)? {
        Value::Struct(child) => Some(child),
        _ => None,
    }
}

fn collect(root: &StructValue<'_>, component: usize, out: &mut Vec<Geometry>) {
    let mut pending = VecDeque::from([(root, Vec::new())]);
    while let Some((node, slots)) = pending.pop_front() {
        if let Some(Value::Array(children)) = node.field("SubGeometry") {
            for (slot, child) in children.iter().enumerate() {
                if let Value::Struct(value) = child {
                    let mut positions = slots.clone();
                    positions.push(slot);
                    pending.push_back((value, positions));
                }
            }
        }
        let path = structure(node, "Geometry")
            .and_then(|value| structure(value, "Geometry"))
            .and_then(|value| value.field("path"));
        out.push(Geometry {
            component,
            slots,
            path: text(path),
            tags: text(node.field("Tags")),
        });
    }
}

fn text(value: Option<&Value<'_>>) -> Json {
    match value {
        None => json!(["absent", null]),
        Some(Value::String(text)) => json!(["present", text]),
        Some(Value::Null) => json!(["null", null]),
        Some(Value::Error(_)) => json!(["unreadable", null]),
        _ => json!(["invalid", null]),
    }
}

impl Actor {
    pub(super) const fn len(&self) -> usize {
        self.geometry.len()
    }

    pub(super) fn verify(
        &self,
        index: usize,
        wire: &Json,
        sources: &Sources<'_>,
    ) -> io::Result<()> {
        let original = self
            .geometry
            .get(index)
            .ok_or_else(|| io::Error::other("extra actor geometry row"))?;
        if wire["recordIndex"] != self.record
            || wire["component"] != original.component
            || wire["subGeometry"] != json!(original.slots)
        {
            return Err(io::Error::other("actor geometry owner position disagrees"));
        }
        let mut root = format!("Components[{}].Geometry", original.component);
        for slot in &original.slots {
            use std::fmt::Write as _;
            write!(&mut root, ".SubGeometry[{slot}]").map_err(io::Error::other)?;
        }
        for (field, suffix, expected) in [
            (
                &wire["modelPath"],
                ".Geometry.Geometry.path",
                &original.path,
            ),
            (&wire["tags"], ".Tags", &original.tags),
        ] {
            if &json!([field["state"], field["value"]]) != expected {
                return Err(io::Error::other("actor geometry original value differs"));
            }
            sources.field(
                field,
                &self.physical,
                &SourceLocation::DataForge {
                    record_index: self.record,
                    field: format!("{root}{suffix}"),
                },
            )?;
        }
        Ok(())
    }
}

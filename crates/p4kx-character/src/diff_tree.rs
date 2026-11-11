// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::diff::{Changes, hex, text};
use crate::diff_pair::{self, Paired};
use crate::diff_source::{MatchSite, Trail};
use crate::{
    ChfNameKind, ColorParam, FloatParam, ItemPort, Material, NameTable, SubMaterial, Texture,
};

/// How the items of a list are paired, named and shown when only one file has them.
struct Kind<T> {
    key: fn(&T) -> u32,
    name: Option<ChfNameKind>,
    hash_field: &'static str,
    value: fn(&T) -> String,
    numbered: bool,
}

impl<T> Kind<T> {
    fn label(&self, names: &NameTable, hash: u32) -> String {
        if self.numbered {
            hash.to_string()
        } else {
            self.name
                .and_then(|kind| names.get(kind, hash))
                .map_or_else(|| hex(&hash), str::to_owned)
        }
    }
    fn site(&self, names: &NameTable, pair: &Paired<'_, T>) -> MatchSite {
        MatchSite {
            before: pair.old_at,
            after: pair.new_at,
            hash: pair.hash,
            ordinal: pair.ordinal,
            hash_field: self.hash_field,
            name_kind: self
                .name
                .filter(|kind| names.get(*kind, pair.hash).is_some()),
        }
    }
}

fn each<T: PartialEq>(
    changes: &mut Changes<'_>,
    trail: &Trail,
    lists: (&[T], &[T]),
    kind: &Kind<T>,
    both: impl Fn(&mut Changes<'_>, &Trail, &T, &T),
) {
    let pairs = diff_pair::pair(lists.0, lists.1, kind.key);
    for pair in &pairs {
        let label = kind.label(changes.names, pair.hash);
        let at = trail.segment(&label, &hex(&pair.hash), &kind.site(changes.names, pair));
        match (pair.old, pair.new) {
            (Some(old), Some(new)) if old != new => both(changes, &at, old, new),
            (Some(old), None) => changes.removed(&at, (kind.value)(old)),
            (None, Some(new)) => changes.added(&at, (kind.value)(new)),
            _ => {}
        }
    }
    let mut matched: Vec<(usize, &Paired<'_, T>)> = pairs
        .iter()
        .filter(|pair| pair.old_at.is_some())
        .filter_map(|pair| pair.new_at.map(|at| (at, pair)))
        .collect();
    if matched
        .iter()
        .zip(matched.iter().skip(1))
        .any(|(a, b)| a.0 > b.0)
    {
        let names = changes.names;
        let list = |matched: &[(usize, &Paired<'_, T>)]| {
            matched
                .iter()
                .map(|(_, pair)| {
                    let label = kind.label(names, pair.hash);
                    if pair.ordinal == 0 {
                        label
                    } else {
                        format!("{label}#{}", pair.ordinal)
                    }
                })
                .collect::<Vec<_>>()
                .join(", ")
        };
        let old = list(&matched);
        matched.sort_by_key(|(at, _)| *at);
        let lookups = matched
            .iter()
            .filter_map(|(_, pair)| trail.lookup(&kind.site(names, pair)))
            .collect();
        changes.order(&trail.display_key("order"), old, list(&matched), lookups);
    }
}

fn plural(count: u64, one: &str, many: &str) -> String {
    format!("{count} {}", if count == 1 { one } else { many })
}

fn port_text(port: &ItemPort) -> String {
    let children = port.count().saturating_sub(1);
    if children == 0 {
        format!("item {}", port.item)
    } else {
        format!(
            "item {}, {}",
            port.item,
            plural(children, "child port", "child ports")
        )
    }
}

pub(crate) fn diff_ports(
    changes: &mut Changes<'_>,
    trail: &Trail,
    old: Option<&ItemPort>,
    new: Option<&ItemPort>,
) {
    match (old, new) {
        (Some(old), Some(new)) => {
            changes.field(trail, "port", &old.port, &new.port, hex);
            port_fields(changes, trail, old, new);
        }
        (Some(old), None) => changes.removed(trail, port_text(old)),
        (None, Some(new)) => changes.added(trail, port_text(new)),
        (None, None) => {}
    }
}

fn port_fields(changes: &mut Changes<'_>, trail: &Trail, old: &ItemPort, new: &ItemPort) {
    changes.field(trail, "item", &old.item, &new.item, text);
    changes.field(trail, "tail_count", &old.tail_count, &new.tail_count, text);
    let kind = Kind {
        key: |port: &ItemPort| port.port,
        name: Some(ChfNameKind::Port),
        hash_field: "port",
        value: port_text,
        numbered: false,
    };
    each(
        changes,
        &trail.clone().source_key("children"),
        (&old.children, &new.children),
        &kind,
        port_fields,
    );
}

fn material_text(material: &Material) -> String {
    format!(
        "base material {}, {}",
        material.base_material,
        plural(
            material.sub_materials.len() as u64,
            "sub-material",
            "sub-materials"
        )
    )
}

pub(crate) fn diff_materials(
    changes: &mut Changes<'_>,
    trail: &Trail,
    old: &[Material],
    new: &[Material],
) {
    let kind = Kind {
        key: |material: &Material| material.attachment,
        name: Some(ChfNameKind::Attachment),
        hash_field: "attachment",
        value: material_text,
        numbered: false,
    };
    each(changes, trail, (old, new), &kind, material);
}

fn material(changes: &mut Changes<'_>, at: &Trail, old: &Material, new: &Material) {
    changes.field(
        at,
        "base_material",
        &old.base_material,
        &new.base_material,
        text,
    );
    changes.field(at, "flags", &old.flags, &new.flags, |flags| {
        format!("{flags:#010x}")
    });
    let kind = Kind {
        key: |sub: &SubMaterial| sub.name,
        name: Some(ChfNameKind::SubMaterial),
        hash_field: "name",
        value: |sub| {
            format!(
                "{}, {}, {}",
                plural(sub.textures.len() as u64, "texture", "textures"),
                plural(sub.floats.len() as u64, "float", "floats"),
                plural(sub.colors.len() as u64, "colour", "colours")
            )
        },
        numbered: false,
    };
    each(
        changes,
        &at.key("sub_materials"),
        (&old.sub_materials, &new.sub_materials),
        &kind,
        sub_material,
    );
}

fn sub_material(changes: &mut Changes<'_>, at: &Trail, old: &SubMaterial, new: &SubMaterial) {
    let textures = Kind {
        key: |texture: &Texture| u32::from(texture.slot),
        name: None,
        hash_field: "slot",
        value: |texture| texture.texture.to_string(),
        numbered: true,
    };
    each(
        changes,
        &at.key("textures"),
        (&old.textures, &new.textures),
        &textures,
        |changes, at, old, new| {
            changes.field(at, "texture", &old.texture, &new.texture, text);
        },
    );
    let floats = Kind {
        key: |float: &FloatParam| float.name,
        name: Some(ChfNameKind::Parameter),
        hash_field: "name",
        value: |float| float_text(float.value),
        numbered: false,
    };
    each(
        changes,
        &at.key("floats"),
        (&old.floats, &new.floats),
        &floats,
        |changes, at, old, new| {
            changes.field(
                at,
                "value",
                &old.value.to_bits(),
                &new.value.to_bits(),
                |bits| float_text(f32::from_bits(*bits)),
            );
        },
    );
    let colors = Kind {
        key: |color: &ColorParam| color.name,
        name: Some(ChfNameKind::Parameter),
        hash_field: "name",
        value: |color| colour_text(color.rgba),
        numbered: false,
    };
    each(
        changes,
        &at.key("colors"),
        (&old.colors, &new.colors),
        &colors,
        |changes, at, old, new| {
            changes.field(at, "rgba", &old.rgba, &new.rgba, |rgba| colour_text(*rgba));
        },
    );
}

fn float_text(value: f32) -> String {
    if value.is_finite() {
        value.to_string()
    } else {
        format!("{value} ({:#010x})", value.to_bits())
    }
}

fn colour_text([red, green, blue, alpha]: [u8; 4]) -> String {
    format!("#{red:02x}{green:02x}{blue:02x}{alpha:02x}")
}

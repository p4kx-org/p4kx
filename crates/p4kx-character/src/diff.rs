// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::diff_source::{ChfDiffLookup, ChfObservedChange, Trail};
use crate::{Chf, Dna, DnaBlend, NameTable, face_part_name};

/// One difference between two CHF files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChfChange {
    /// The field, named like the keys of the JSON form and with every hash that a name matches
    /// replaced by the name, e.g. `materials.head_itemport.sub_materials.shader_eyeinner`.
    pub path: String,
    /// The same field with every hash as `0x` and eight hexadecimal digits, which does not
    /// depend on the names known.
    pub raw_path: String,
    /// The value in the first file, `None` when the field is absent there.
    pub old: Option<String>,
    /// The value in the second file, `None` when the field is absent there.
    pub new: Option<String>,
}

/// Collects the changes between two files.
pub(crate) struct Changes<'n> {
    pub(crate) names: &'n NameTable,
    emit: &'n mut dyn FnMut(ChfChange, &Trail, Vec<ChfDiffLookup>),
}

impl Changes<'_> {
    pub(crate) fn field<T: PartialEq>(
        &mut self,
        trail: &Trail,
        key: &str,
        old: &T,
        new: &T,
        show: impl Fn(&T) -> String,
    ) {
        if old != new {
            self.push(&trail.key(key), Some(show(old)), Some(show(new)));
        }
    }

    pub(crate) fn removed(&mut self, trail: &Trail, value: String) {
        self.push(trail, Some(value), None);
    }

    pub(crate) fn added(&mut self, trail: &Trail, value: String) {
        self.push(trail, None, Some(value));
    }

    pub(crate) fn order(
        &mut self,
        trail: &Trail,
        old: String,
        new: String,
        lookups: Vec<ChfDiffLookup>,
    ) {
        self.emit(trail, Some(old), Some(new), lookups);
    }

    fn push(&mut self, trail: &Trail, old: Option<String>, new: Option<String>) {
        self.emit(trail, old, new, Vec::new());
    }

    fn emit(
        &mut self,
        trail: &Trail,
        old: Option<String>,
        new: Option<String>,
        order: Vec<ChfDiffLookup>,
    ) {
        (self.emit)(
            ChfChange {
                path: trail.text.clone(),
                raw_path: trail.raw.clone(),
                old,
                new,
            },
            trail,
            order,
        );
    }
}

pub(crate) fn text<T: ToString>(value: &T) -> String {
    value.to_string()
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "has the signature of a field formatter"
)]
pub(crate) fn hex(value: &u32) -> String {
    format!("{value:#010x}")
}

/// Returns what differs between `old` and `new`, in the order of the JSON form, with the hashes
/// that `names` knows spelled out in [`ChfChange::path`].
///
/// Item ports, materials, sub-materials, parameters and textures are paired by their hash (the
/// slot for a texture): equal items first, then the rest in order, a repeat of a hash getting
/// `#n`. Items that exist in both files in another order give one `order` change at their list
/// with the labels in either order. The bytes of the container are not compared except the header
/// flags: the compressed frame is a coding of the body, not part of the character. Both files
/// must come from [`Chf::read`], which keeps the item port depth within [`crate::MAX_PORT_DEPTH`].
#[must_use]
pub fn diff_chf(old: &Chf, new: &Chf, names: &NameTable) -> Vec<ChfChange> {
    let mut values = Vec::new();
    compare(old, new, names, false, &mut |change, _, _| {
        values.push(change);
    });
    values
}

/// Returns each comparison row with the actual field and hash-selector coordinates.
#[must_use]
pub fn diff_chf_observed(old: &Chf, new: &Chf, names: &NameTable) -> Vec<ChfObservedChange> {
    let mut values = Vec::new();
    compare(old, new, names, true, &mut |change, trail, order| {
        values.push(trail.observe(change, order));
    });
    values
}

fn compare(
    old: &Chf,
    new: &Chf,
    names: &NameTable,
    capture: bool,
    emit: &mut dyn FnMut(ChfChange, &Trail, Vec<ChfDiffLookup>),
) {
    let mut changes = Changes { names, emit };
    let root = Trail::root(capture);
    let container = root.key("container");
    changes.field(
        &container,
        "flags",
        &old.container.flags,
        &new.container.flags,
        |flags| format!("{flags:#06x}"),
    );
    let (old, new) = (&old.character, &new.character);
    let root = root.source_key("character");
    let versions = root.key("versions");
    changes.field(
        &versions,
        "female",
        &old.versions.female,
        &new.versions.female,
        text,
    );
    changes.field(
        &versions,
        "male",
        &old.versions.male,
        &new.versions.male,
        text,
    );
    changes.field(&root, "body_type", &old.body_type, &new.body_type, text);
    changes.field(&root, "voice", &old.voice, &new.voice, text);
    let dna = root.key("dna");
    match (&old.dna, &new.dna) {
        (Some(old), Some(new)) => diff_dna(&mut changes, &dna, old, new),
        (Some(_), None) => changes.removed(&dna, "present".to_owned()),
        (None, Some(_)) => changes.added(&dna, "present".to_owned()),
        (None, None) => {}
    }
    changes.field(
        &root,
        "item_port_tail",
        &old.item_port_tail,
        &new.item_port_tail,
        text,
    );
    crate::diff_tree::diff_ports(
        &mut changes,
        &root.key("item_ports"),
        old.item_ports.as_ref(),
        new.item_ports.as_ref(),
    );
    crate::diff_tree::diff_materials(
        &mut changes,
        &root.key("materials"),
        &old.materials,
        &new.materials,
    );
}

fn diff_dna(changes: &mut Changes<'_>, trail: &Trail, old: &Dna, new: &Dna) {
    let names = changes.names;
    changes.field(
        trail,
        "gender_hash",
        &old.gender_hash,
        &new.gender_hash,
        |hash| {
            names
                .rig(*hash)
                .map_or_else(|| hex(hash), |name| format!("{hash:#010x} ({name})"))
        },
    );
    changes.field(
        trail,
        "variant_hash",
        &old.variant_hash,
        &new.variant_hash,
        hex,
    );
    changes.field(
        trail,
        "blends_per_part",
        &old.blends_per_part,
        &new.blends_per_part,
        text,
    );
    changes.field(
        trail,
        "header_unknown",
        &old.header_unknown,
        &new.header_unknown,
        text,
    );
    changes.field(
        trail,
        "max_head_id",
        &old.max_head_id,
        &new.max_head_id,
        text,
    );
    let parts = trail.key("face_parts");
    for index in 0..old.face_parts.len().max(new.face_parts.len()) {
        let part = parts.display_key(&face_part_name(index)).source_indices(
            index,
            index < old.face_parts.len(),
            index < new.face_parts.len(),
        );
        match (old.face_parts.get(index), new.face_parts.get(index)) {
            (Some(old), Some(new)) => diff_blends(changes, &part, old, new),
            (Some(_), None) => changes.removed(&part, "present".to_owned()),
            (None, Some(_)) => changes.added(&part, "present".to_owned()),
            (None, None) => {}
        }
    }
}

fn diff_blends(changes: &mut Changes<'_>, part: &Trail, old: &[DnaBlend], new: &[DnaBlend]) {
    for blend in 0..old.len().max(new.len()) {
        let at = part
            .display_key(&format!("blends[{blend}]"))
            .source_key("blends")
            .source_indices(blend, blend < old.len(), blend < new.len());
        match (old.get(blend), new.get(blend)) {
            (Some(old), Some(new)) => {
                changes.field(&at, "value", &old.value, &new.value, text);
                changes.field(&at, "head_id", &old.head_id, &new.head_id, text);
            }
            (Some(old), None) => changes.removed(&at, blend_text(*old)),
            (None, Some(new)) => changes.added(&at, blend_text(*new)),
            (None, None) => {}
        }
    }
}

fn blend_text(blend: DnaBlend) -> String {
    format!("value {}, head {}", blend.value, blend.head_id)
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;

use anyhow::Result;
use p4kx_assembly::{
    Assembly, EntityInterior, EntityNode, Interior, InteriorForge, InteriorOptions, InteriorWatch,
};
use p4kx_extraction::{Archive, DataForge, DataForgeEntry};

use crate::query_table::one_line;

/// Adds the Interior of the Entity `tree` to `assembly`, with the class Records of the
/// containers' entities looked up in `forge`.
pub(crate) fn with_interior(
    assembly: Assembly,
    (archive, entry, forge): (&Archive, &DataForgeEntry, &DataForge<'_>),
    tree: &EntityNode,
    lod: u32,
) -> Result<Assembly> {
    let options = InteriorOptions::default().with_lod(lod);
    let classes = InteriorForge::new(forge).with_entry(entry);
    let interior = EntityInterior::build_with(
        archive,
        &assembly,
        tree,
        &options,
        &InteriorWatch::none(),
        Some(&classes),
    )?;
    Ok(assembly.with_interior(interior))
}

fn or_warn<T, E: std::fmt::Display>(read: Result<T, E>) -> Option<T> {
    match read {
        Ok(value) => Some(value),
        Err(error) => {
            let _ = writeln!(
                std::io::stderr().lock(),
                "warning: the DataForge cannot be read, so entities that name only a class Record are left out: {}",
                one_line(&error.to_string())
            );
            None
        }
    }
}

/// Builds the Interior of the container chain `layers`, the Archive first, with the class Records
/// of the entities looked up in the Archive's DataForge; an Archive without one builds the
/// Interior without.
pub(crate) fn build(
    layers: &[Archive],
    entry: &str,
    options: &InteriorOptions,
) -> Result<Interior> {
    let refs: Vec<&Archive> = layers.iter().collect();
    let forge_entry = layers
        .first()
        .filter(|outer| DataForgeEntry::locate(outer).is_some())
        .and_then(|outer| or_warn(DataForgeEntry::read(outer)));
    let forge = forge_entry.as_ref().and_then(|entry| or_warn(entry.open()));
    let classes = forge
        .as_ref()
        .zip(forge_entry.as_ref())
        .map(|(forge, entry)| InteriorForge::new(forge).with_entry(entry));
    Ok(Interior::build_with(
        &refs,
        entry,
        options,
        &InteriorWatch::none(),
        classes.as_ref(),
    )?)
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    ArchiveFiles, Attachment, LoadoutChanges, LoadoutPicks, LoadoutResolver, PortNode,
};
use p4kx_extraction::DataForge;

use crate::archives::Opened;
use crate::budget::{fill_within, json_len};
use crate::error::{ToolError, capped};
use crate::loadout_shape;
use crate::loadout_shapes::Counts;
pub(crate) use crate::loadout_shapes::{LoadoutParams, LoadoutResult};
use crate::page::Page;

/// The deepest port nesting a call returns when it gives no depth.
pub(crate) const DEFAULT_DEPTH: u32 = 16;
/// The deepest port nesting a call may ask for.
pub(crate) const MAX_DEPTH: u32 = 32;
/// The most bytes accepted in one Loadout pick argument.
const MAX_PICK_BYTES: usize = 256;
/// The most picks a call may list.
const MAX_PICKS: usize = 64;

/// Resolves the Loadout of the Entity `params` names and returns one page of its ports.
pub(crate) fn resolve(
    opened: &Opened,
    forge: &DataForge<'_>,
    params: &LoadoutParams,
    page: Page,
) -> Result<LoadoutResult, ToolError> {
    let stop_at_docked = params.stop_at_docked_vehicles == Some(true);
    let picks = picks_of(params.loadout_pick.as_deref().unwrap_or_default())?;
    let key = picks.normalised();
    let tree = opened.loadouts().get_or_resolve(
        opened.archive(),
        &params.entity,
        (stop_at_docked, &key),
        || {
            let files = ArchiveFiles::new(opened.archive());
            let mut resolver = LoadoutResolver::new(forge).with_files(&files);
            if stop_at_docked {
                resolver = resolver.stopping_at_docked_vehicles();
            }
            let record = resolver.entities().find(&params.entity)?;
            let changes = LoadoutChanges::new().with_picks(picks.clone());
            Ok(resolver.resolve_with(&record, &changes)?)
        },
    )?;
    let max_depth = usize::try_from(
        params
            .max_depth
            .unwrap_or(DEFAULT_DEPTH)
            .clamp(1, MAX_DEPTH),
    )
    .unwrap_or(1);
    let all = tree.walk();
    let counts = counts(&all);
    let near: Vec<(&String, &PortNode)> = all
        .iter()
        .filter(|(path, _)| depth(path) <= max_depth)
        .map(|(path, port)| (path, *port))
        .collect();
    let mut result = loadout_shape::envelope(
        opened.archive().path(),
        &tree,
        counts,
        near.len(),
        all.len() - near.len(),
    )?;
    let items = page
        .slice(&near)
        .iter()
        .map(|&(path, port)| loadout_shape::port(path, depth(path), port));
    let (ports, next_cursor) = fill_within(page, near.len(), json_len(&result), items, |shape| {
        format!("loadout port at depth {}", shape.depth)
    })?;
    result.ports = ports;
    result.next_cursor = next_cursor;
    Ok(result)
}

/// Reads the picks of a call: each text `<group>=<index>` or a bare `<index>`, several to a text
/// separated by commas.
pub(crate) fn picks_of(texts: &[String]) -> Result<LoadoutPicks, ToolError> {
    if texts.len() > MAX_PICKS {
        return Err(ToolError::Refused(format!(
            "loadout_pick lists {} picks; at most {MAX_PICKS} are accepted",
            texts.len()
        )));
    }
    for text in texts {
        capped("loadout_pick", text, MAX_PICK_BYTES)?;
    }
    Ok(LoadoutPicks::from_list(&texts.join(","))?)
}

fn depth(path: &str) -> usize {
    path.matches('/').count() + 1
}

fn counts(all: &[(String, &PortNode)]) -> Counts {
    let mut counts = Counts {
        ports: all.len(),
        ..Counts::default()
    };
    for (_, port) in all {
        match port.attachment() {
            Attachment::Empty => counts.empty += 1,
            Attachment::Entity(_) => counts.attached += 1,
            Attachment::Unresolved(_) => counts.unresolved += 1,
        }
    }
    counts
}

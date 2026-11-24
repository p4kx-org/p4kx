// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{
    ArchiveFiles, Attachment, Entities, EntityNode, LoadoutChanges, LoadoutPicks, LoadoutResolver,
    PortNode,
};
use p4kx_extraction::{DataForge, DataForgeOrigin};

use crate::archives::Opened;
use crate::budget::{fill_within, fit_prefix, json_len};
use crate::error::{ToolError, capped};
use crate::loadout_shape;
use crate::loadout_shapes::{Counts, LoadoutHeader};
pub(crate) use crate::loadout_shapes::{LoadoutParams, LoadoutResult};
use crate::page::Page;

#[path = "loadout_name_origins.rs"]
mod name_origins;
#[path = "loadout_origins.rs"]
mod origins;

pub(crate) use name_origins::ResolvedLoadout;

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
    origin: &DataForgeOrigin,
    params: &LoadoutParams,
    page: Page,
) -> Result<LoadoutResult, ToolError> {
    let resolved = resolve_tree(opened, forge, params)?;
    let max_depth = usize::try_from(
        params
            .max_depth
            .unwrap_or(DEFAULT_DEPTH)
            .clamp(1, MAX_DEPTH),
    )
    .unwrap_or(1);
    let Listing {
        counts,
        total,
        window,
    } = Listing::collect(&resolved.tree, max_depth, page);
    page.check(total)?;
    let deeper = counts.ports - total;
    let header = loadout_shape::header(
        opened.archive().path(),
        &resolved.tree,
        counts,
        total,
        deeper,
    )?;
    Projection {
        forge,
        origin,
        resolved: &resolved,
        page,
    }
    .publish(header, &window)
}

fn resolve_tree(
    opened: &Opened,
    forge: &DataForge<'_>,
    params: &LoadoutParams,
) -> Result<Arc<ResolvedLoadout>, ToolError> {
    let stop_at_docked = params.stop_at_docked_vehicles == Some(true);
    let picks = picks_of(params.loadout_pick.as_deref().unwrap_or_default())?;
    let key = picks.normalised();
    opened.loadouts().get_or_resolve(
        opened.archive(),
        &params.entity,
        (stop_at_docked, &key),
        || {
            let files = ArchiveFiles::new(opened.archive());
            let capture = Arc::new(name_origins::Capture::new(forge));
            let mut resolver = LoadoutResolver::with_observed_entities(
                Arc::new(Entities::new(forge)),
                capture.clone(),
            )
            .with_files(&files);
            if stop_at_docked {
                resolver = resolver.stopping_at_docked_vehicles();
            }
            let record = resolver.entities().find(&params.entity)?;
            let changes = LoadoutChanges::new().with_picks(picks.clone());
            let tree = resolver.resolve_with(&record, &changes)?;
            capture.finish(tree).map_err(ToolError::from)
        },
    )
}

struct Projection<'input, 'forge> {
    forge: &'input DataForge<'forge>,
    origin: &'input DataForgeOrigin,
    resolved: &'input ResolvedLoadout,
    page: Page,
}

impl Projection<'_, '_> {
    fn publish(
        self,
        header: LoadoutHeader,
        window: &[PagePort<'_>],
    ) -> Result<LoadoutResult, ToolError> {
        let page = self.page;
        let total = header.total;
        let items = window
            .iter()
            .map(|row| loadout_shape::port(&row.path, row.depth, row.port));
        let (mut ports, _) = fill_within(page, total, json_len(&header), items, |shape| {
            format!("loadout port at depth {}", shape.depth)
        })?;
        let (keep, fitted) = fit_prefix(
            ports.len(),
            |len| {
                let origins = origins::capture(
                    self.forge,
                    self.origin,
                    self.resolved,
                    window.split_at(len).0,
                )?;
                Ok(LoadoutResult {
                    header: &header,
                    ports: ports.split_at(len).0,
                    next_cursor: page.next_cursor_after(len, total)?,
                    origins,
                })
            },
            || format!("loadout port at position {}", page.offset()),
        )?;
        let LoadoutResult {
            next_cursor,
            origins,
            ..
        } = fitted;
        ports.truncate(keep);
        Ok(LoadoutResult {
            header,
            ports,
            next_cursor,
            origins,
        })
    }
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

struct PagePort<'tree> {
    path: String,
    depth: usize,
    port: &'tree PortNode,
    ancestors: Vec<&'tree PortNode>,
}

#[derive(Default)]
struct Listing<'tree> {
    counts: Counts,
    total: usize,
    window: Vec<PagePort<'tree>>,
}

impl<'tree> Listing<'tree> {
    fn collect(tree: &'tree EntityNode, max_depth: usize, page: Page) -> Self {
        let mut listing = Self::default();
        tree.visit_ports(|path, route| {
            let Some((&port, ancestors)) = route.split_last() else {
                return;
            };
            listing.counts.ports += 1;
            match port.attachment() {
                Attachment::Empty => listing.counts.empty += 1,
                Attachment::Entity(_) => listing.counts.attached += 1,
                Attachment::Unresolved(_) => listing.counts.unresolved += 1,
            }
            let depth = path.matches('/').count() + 1;
            if depth <= max_depth {
                if page.contains(listing.total) {
                    listing.window.push(PagePort {
                        path: path.to_owned(),
                        depth,
                        port,
                        ancestors: ancestors.to_vec(),
                    });
                }
                listing.total += 1;
            }
        });
        listing
    }
}

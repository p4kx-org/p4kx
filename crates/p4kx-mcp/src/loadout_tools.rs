// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};

use crate::error::capped;
use crate::loadout::{self, LoadoutParams, LoadoutResult};
use crate::noted::Noted;
use crate::page::Page;
use crate::params::MAX_PATH_LEN;
use crate::server::Server;

#[tool_router(router = loadout_router, vis = "pub(crate)")]
impl Server {
    #[tool(
        description = "Resolve the default Loadout of an Entity (class name, record name, record path or GUID) into its tree \
        of ports, listed depth first with parents before children: per port its path, name, where the attachment comes from \
        (loadout, override, port_default, unset, empty for a port only the vehicle implementation declares), the item types and sizes it takes, the Entity attached to it with its class \
        name and GUID, and for every port that cannot be resolved the reason (dangling_reference, unknown_class, cycle, ...) \
        and the reference or class name the Loadout names. Counts cover the whole tree; max_depth (16 by default, at most 32) \
        leaves deeper ports out and deeper says how many. Paged with a cursor. The content may be forwarded by the client to \
        third parties; game texts and media must not be redistributed.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<LoadoutResult>(),
        annotations(
            title = "Entity Loadout",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn entity_loadout(
        &self,
        Parameters(params): Parameters<LoadoutParams>,
    ) -> Result<Noted<LoadoutResult>, String> {
        self.preload(params.archive.clone()).await?;
        self.run(move |state| {
            capped("entity", &params.entity, MAX_PATH_LEN)?;
            let opened = state.archive(params.archive.as_deref())?;
            let picks = loadout::picks_of(params.loadout_pick.as_deref().unwrap_or_default())?;
            let listing = (
                "entity_loadout",
                opened.version(),
                &params.entity,
                params.max_depth,
                params.stop_at_docked_vehicles,
                picks.normalised(),
            );
            let page = Page::new(&listing, params.cursor.as_deref(), params.limit)?;
            let held = Arc::clone(&opened);
            opened.with_dataforge(move |forge, _| loadout::resolve(&held, forge, &params, page))
        })
        .await
    }
}

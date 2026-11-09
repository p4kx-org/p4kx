// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::CancelToken;
use p4kx_extraction::query::MAX_QUERY_LEN;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};

use crate::error::capped;
use crate::forge_params::QueryParams;
use crate::forge_shapes::QueryResult;
use crate::ordered::Ordered;
use crate::page::Page;
use crate::query::{self, Target};
use crate::server::Server;

struct CancelOnDrop(CancelToken);

impl Drop for CancelOnDrop {
    fn drop(&mut self) {
        self.0.cancel();
    }
}

#[tool_router(router = query_router, vis = "pub(crate)")]
impl Server {
    #[tool(
        description = "Run a Query over the Records of the Archive's DataForge, e.g. \
        `from EntityClassDefinition at \"*/spaceships/*\" select Components[VehicleComponentParams].crewSize where Components[VehicleComponentParams].crewSize >= 4`. \
        from selects a type and the types deriving from it, at a record path glob, select one column per dot path \
        ([n], [*], [TypeName] for array elements, -> to follow a Reference), where a condition. \
        Returns the columns and one page of rows in Record order; pass next_cursor back as cursor. \
        An invalid Query is an error naming its byte span; an unreadable value needed to decide selection is a tool error, not an empty result. \
        list_structs gives the type and field names. \
        The content may be forwarded by the client to third parties; game texts and media must not be redistributed.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<QueryResult>(),
        annotations(
            title = "Run Query",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn run_query(
        &self,
        Parameters(params): Parameters<QueryParams>,
    ) -> Result<Ordered<QueryResult>, String> {
        self.preload(params.archive.clone()).await?;
        let cancel = CancelToken::new();
        let _stops_the_selection = CancelOnDrop(cancel.clone());
        self.run_ordered(move |state| {
            capped("query", &params.query, MAX_QUERY_LEN)?;
            let opened = state.forge(params.archive.as_deref())?;
            let listing = ("run_query", opened.version(), &params.query);
            let page = Page::new(&listing, params.cursor.as_deref(), params.limit)?;
            let held = opened.clone();
            opened.with_dataforge(move |forge, archive| {
                let target = Target {
                    forge,
                    archive,
                    opened: &held,
                };
                query::run(&target, &params.query, page, &cancel)
            })
        })
        .await
    }
}

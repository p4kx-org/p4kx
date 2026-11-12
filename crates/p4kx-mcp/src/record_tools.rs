// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};

use crate::definitions;
use crate::error::capped;
use crate::forge_params::{
    DefinitionParams, MAX_GUID_LEN, MAX_NAME_LEN, RecordParams, RecordSearchParams,
};
use crate::forge_shapes::{EnumsResult, RecordResult, RecordSearchResult, StructsResult};
use crate::noted::Noted;
use crate::ordered::Ordered;
use crate::page::Page;
use crate::params::MAX_PATH_LEN;
use crate::record_search;
use crate::records::{self, Selector};
use crate::server::Server;

#[tool_router(router = record_router, vis = "pub(crate)")]
impl Server {
    #[tool(
        description = "Get a Record of the Archive's DataForge by GUID, or every Record with a record path, as Record JSON: \
        $type, $guid, $path and the fields; References are links with guid, path and type, never the linked Record. \
        Large results page: pass next_cursor back as cursor. The content may be forwarded by the client to \
        third parties; game texts and media must not be redistributed.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<RecordResult>(),
        annotations(
            title = "Get Record",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_record(
        &self,
        Parameters(params): Parameters<RecordParams>,
    ) -> Result<Ordered<RecordResult>, String> {
        self.preload(params.archive.clone()).await?;
        self.run_ordered(move |state| {
            if let Some(guid) = &params.guid {
                capped("guid", guid, MAX_GUID_LEN)?;
            }
            if let Some(path) = &params.path {
                capped("path", path, MAX_PATH_LEN)?;
            }
            let selector = Selector::new(params.guid, params.path)?;
            let opened = state.forge(params.archive.as_deref())?;
            let listing = ("get_record", opened.version(), &selector);
            let page = Page::new(&listing, params.cursor.as_deref(), None)?;
            opened
                .with_dataforge(move |forge, archive| records::get(forge, archive, &selector, page))
        })
        .await
    }

    #[tool(
        description = "Search the Records of the Archive's DataForge by record path and type name, both Filters: \
        with * or ? a glob over the whole text, otherwise a substring; case and separator kind do not matter; \
        each Filter is at most 1024 bytes with at most 32 * and ?, otherwise the call is an error. \
        Results are in Record order and page with a cursor. Field origins identify the original DataForge values and selectors. The content may be forwarded by the client to \
        third parties; game texts and media must not be redistributed.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<RecordSearchResult>(),
        annotations(
            title = "Search Records",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn search_records(
        &self,
        Parameters(params): Parameters<RecordSearchParams>,
    ) -> Result<Noted<RecordSearchResult>, String> {
        self.preload(params.archive.clone()).await?;
        self.run(move |state| {
            if let Some(path) = &params.path {
                capped("path", path, MAX_PATH_LEN)?;
            }
            if let Some(type_name) = &params.type_name {
                capped("type", type_name, MAX_NAME_LEN)?;
            }
            let opened = state.forge(params.archive.as_deref())?;
            let listing = (
                "search_records",
                opened.version(),
                &params.path,
                &params.type_name,
            );
            let page = Page::new(&listing, params.cursor.as_deref(), params.limit)?;
            opened.with_dataforge_origin(move |forge, archive, origin| {
                let filters = (params.path.as_deref(), params.type_name.as_deref());
                record_search::search(forge, &archive, origin, filters, page)
            })
        })
        .await
    }

    #[tool(
        description = "List the struct definitions of the Archive's DataForge, the types of Records and nested values, \
        with their parent and the fields each declares (name, data type, array, target struct or enum). \
        name is a Filter over the struct name. Paged with a cursor; use it to write correct Queries. Field origins identify the declarations actually used. The content may be forwarded by the client to \
        third parties; game texts and media must not be redistributed.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<StructsResult>(),
        annotations(
            title = "List structs",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_structs(
        &self,
        Parameters(params): Parameters<DefinitionParams>,
    ) -> Result<Noted<StructsResult>, String> {
        self.preload(params.archive.clone()).await?;
        self.run(move |state| {
            if let Some(name) = &params.name {
                capped("name", name, MAX_NAME_LEN)?;
            }
            let opened = state.forge(params.archive.as_deref())?;
            let listing = ("list_structs", opened.version(), &params.name);
            let page = Page::new(&listing, params.cursor.as_deref(), params.limit)?;
            opened.with_dataforge_origin(move |forge, archive, origin| {
                definitions::structs(forge, archive, origin, params.name.as_deref(), page)
            })
        })
        .await
    }

    #[tool(
        description = "List the enum definitions of the Archive's DataForge with their values in order. \
        name is a Filter over the enum name. Paged with a cursor; field origins identify the original option positions. The content may be forwarded by the \
        client to third parties; game texts and media must not be redistributed.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<EnumsResult>(),
        annotations(
            title = "List enums",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_enums(
        &self,
        Parameters(params): Parameters<DefinitionParams>,
    ) -> Result<Noted<EnumsResult>, String> {
        self.preload(params.archive.clone()).await?;
        self.run(move |state| {
            if let Some(name) = &params.name {
                capped("name", name, MAX_NAME_LEN)?;
            }
            let opened = state.forge(params.archive.as_deref())?;
            let listing = ("list_enums", opened.version(), &params.name);
            let page = Page::new(&listing, params.cursor.as_deref(), params.limit)?;
            opened.with_dataforge_origin(move |forge, archive, origin| {
                definitions::enums(forge, archive, origin, params.name.as_deref(), page)
            })
        })
        .await
    }
}

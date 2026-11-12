// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};

use crate::chunk_pages;
use crate::error::capped;
use crate::model_params::{ChunksParams, MAX_TYPE_LEN, ModelItemsParams, ModelParams};
use crate::model_shapes::{ChunksResult, ModelItemsResult, ModelOverview};
use crate::models;
use crate::noted::Noted;
use crate::page::Page;
use crate::params::MAX_PATH_LEN;
use crate::server::Server;

#[tool_router(router = model_router, vis = "pub(crate)")]
impl Server {
    #[tool(
        description = "List the chunks of a CryEngine chunk file Entry (.cgf, .cga, .skin, .chr, .cgfm, .caf, .dba, ...): \
        container form and version, then per chunk its table index, type name and ID, version, chunk ID, byte order, \
        data offset and size. Chunk types not in the table of known types are listed with known false and no name. \
        type narrows the list to one chunk type. Paged with a cursor; the fieldOrigins and provenance describe the original chunk table and path. \
        No chunk data is returned. The content may be forwarded by the client to \
        third parties; game texts and media must not be redistributed.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<ChunksResult>(),
        annotations(
            title = "List chunks",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_chunks(
        &self,
        Parameters(params): Parameters<ChunksParams>,
    ) -> Result<Noted<ChunksResult>, String> {
        self.run(move |state| {
            let _permit = state.model().hold()?;
            let path = capped("path", &params.path, MAX_PATH_LEN)?;
            if let Some(chunk_type) = &params.chunk_type {
                capped("type", chunk_type, MAX_TYPE_LEN)?;
            }
            let opened = state.archive(params.archive.as_deref())?;
            let listing = ("list_chunks", opened.version(), path, &params.chunk_type);
            let page = Page::new(&listing, params.cursor.as_deref(), params.limit)?;
            chunk_pages::list(&opened, path, params.chunk_type.as_deref(), page)
        })
        .await
    }

    #[tool(
        description = "Summarise a model Entry (.cgf, .cga, .skin, .chr with its geometry companion): the geometry and \
        material library it names, material IDs, how the material library read, the levels of detail with vertices, \
        triangles and mesh counts, and the number of materials, nodes, meshes, bones, animations, unread animations \
        and chunks. It is `p4kx model info --json` with the long lists left out; list_model_items returns them, \
        page by page. Reading the animations of a large model takes time. The content may be forwarded by the client \
        to third parties; game texts and media must not be redistributed.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<ModelOverview>(),
        annotations(
            title = "Get model info",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn get_model_info(
        &self,
        Parameters(params): Parameters<ModelParams>,
    ) -> Result<Noted<ModelOverview>, String> {
        self.run(move |state| {
            let _permit = state.model().hold()?;
            let path = capped("path", &params.path, MAX_PATH_LEN)?;
            let opened = state.archive(params.archive.as_deref())?;
            models::overview(&opened, path)
        })
        .await
    }

    #[tool(
        description = "List one section of a model Entry, page by page: materials, nodes, meshes (of all levels of \
        detail, each with its lod), bones, animations, unread_animations or chunks. Each item is shaped as in \
        `p4kx model info --json`. Use get_model_info first for the counts. The content may be forwarded by the \
        client to third parties; game texts and media must not be redistributed.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<ModelItemsResult>(),
        annotations(
            title = "List model items",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_model_items(
        &self,
        Parameters(params): Parameters<ModelItemsParams>,
    ) -> Result<Noted<ModelItemsResult>, String> {
        self.run(move |state| {
            let _permit = state.model().hold()?;
            let path = capped("path", &params.path, MAX_PATH_LEN)?;
            let opened = state.archive(params.archive.as_deref())?;
            let listing = ("list_model_items", opened.version(), path, params.section);
            let page = Page::new(&listing, params.cursor.as_deref(), params.limit)?;
            models::items(&opened, path, params.section, page)
        })
        .await
    }
}

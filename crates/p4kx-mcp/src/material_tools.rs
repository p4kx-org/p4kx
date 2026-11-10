// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};

use crate::error::capped;
use crate::material::{self, MaterialParams, MaterialsResult};
use crate::noted::Noted;
use crate::page::Page;
use crate::params::MAX_PATH_LEN;
use crate::server::Server;

#[tool_router(router = material_router, vis = "pub(crate)")]
impl Server {
    #[tool(
        description = "Summarise a .mtl material library Entry (text XML or CryXml): whether it is one material or a list of \
        sub-materials, its default tint palette, and per sub-material, page by page, the material ID, name, shader, surface type, \
        generation mask, flags, colours, shininess, opacity, alpha test, glow, texture slots with their texture paths (at most 64), shader \
        parameters (at most 64) and material layers (at most 64). Text values are complete; a material that cannot fit the 256 KiB \
        response budget is refused when requested as the first item. It is the file as written; layer \
        files are not read. Files over 8 MiB are refused. The content may be forwarded by the client to third parties; game texts \
        and media must not be redistributed.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<MaterialsResult>(),
        annotations(
            title = "Summarise materials",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn mtl_summary(
        &self,
        Parameters(params): Parameters<MaterialParams>,
    ) -> Result<Noted<MaterialsResult>, String> {
        self.run(move |state| {
            let path = capped("path", &params.path, MAX_PATH_LEN)?;
            let opened = state.archive(params.archive.as_deref())?;
            let listing = ("mtl_summary", opened.version(), path);
            let page = Page::new(&listing, params.cursor.as_deref(), params.limit)?;
            let (place, inner) = opened.place(path)?;
            material::summary(&place, inner, page)
        })
        .await
    }
}

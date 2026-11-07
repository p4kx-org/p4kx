// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use rmcp::handler::server::wrapper::Parameters;
use rmcp::{tool, tool_router};

use crate::error::capped;
use crate::image::{self, ImageParams, ImageResult};
use crate::noted::Noted;
use crate::params::MAX_PATH_LEN;
use crate::picture::Pictured;
use crate::server::Server;

#[tool_router(router = image_router, vis = "pub(crate)")]
impl Server {
    #[tool(
        description = "Show a DDS texture of the Archive as a PNG image content block, with its size, format and mip count \
        as structured content. The path is the texture header Entry; its parts and alpha tail are merged. Without mip the \
        largest level that fits max_side (512 by default, at most 2048) is shown; a cubemap is shown as a cross of its six \
        faces, or one face with face; an array or volume as a sheet. The PNG is at most 1 MiB; a larger mip is an error. \
        The image may be forwarded by the client to third parties; game texts and media must not be redistributed.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<ImageResult>(),
        annotations(
            title = "Preview image",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn image_preview(
        &self,
        Parameters(params): Parameters<ImageParams>,
    ) -> Result<Pictured<ImageResult>, String> {
        let Noted {
            value: (value, png),
            notes,
        } = self
            .run(move |state| {
                let path = capped("path", &params.path, MAX_PATH_LEN)?;
                let opened = state.archive(params.archive.as_deref())?;
                let (place, inner) = opened.place(path)?;
                let _permit = state.model().hold()?;
                image::preview(&place, &params, inner)
            })
            .await?;
        Ok(Pictured { value, png, notes })
    }
}

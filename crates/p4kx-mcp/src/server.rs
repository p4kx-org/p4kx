// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{ServerHandler, tool, tool_handler, tool_router};
use tokio::sync::Semaphore;

use crate::archives::{Archives, Opened};
use crate::entries;
use crate::error::{ToolError, capped};
use crate::forge_file::ForgeSource;
use crate::gate::Gate;
use crate::key_report;
use crate::keys::KeyChoice;
use crate::locate::{Accepts, Locations};
use crate::noted::{self, Noted, Notes};
use crate::ordered::Ordered;
use crate::page::Page;
use crate::params::{ArchiveParams, ChannelsParams, ListParams, MAX_PATH_LEN, SearchParams};
use crate::shapes::{ArchiveInfo, ChannelShape, ChannelsResult, FolderResult, SearchResult};

const WHOLE_TEXT_DECODERS: usize = 4;

const INSTRUCTIONS: &str = "Read-only access to the Data.p4k Archives of Star Citizen. \
An Archive is named by a Channel name such as LIVE (see list_channels) or by the absolute path \
of a .p4k file; without one the tools use the Archive the server was started with. Listings \
page: pass next_cursor back as cursor for the next page. Entry content is read in ranges: pass \
next_offset back as offset for the next range. The DataForge tools (get_record, search_records, \
list_structs, list_enums, run_query) read the Archive's DataForge, its typed Records, and also take \
the absolute path of a DataForge file such as Game2.dcb in place of an Archive; list_structs \
gives the type and field names a Query uses. The model tools (list_chunks, get_model_info, \
list_model_items) read the chunk files and models of the Archive. A path \
`outer.socpak::inner.socpak::file` reads inside socpak containers in list_entries, \
search_entries (container), read_entry_text and read_entry_bytes. image_preview returns a \
texture as a PNG image, mtl_summary the materials of a .mtl and entity_loadout the Loadout \
tree of an Entity. The audio tools \
(search_audio_triggers, get_entity_sounds, resolve_audio_trigger, list_soundbank_wems, list_soundbank_objects, \
get_wem_info, get_audio_excerpt) follow the Wwise chain from an Entity's trigger through events and soundbanks to \
WEMs; only get_audio_excerpt returns audio, a short WAV. P4KX is an unofficial Star Citizen fan \
project, not affiliated with the Cloud Imperium group of companies. Content these tools return \
may be forwarded by the MCP client to third parties such as a cloud model provider; game texts \
and media must not be redistributed.";

/// The MCP server: read-only tools over the detected Channels and their Archives.
#[derive(Debug, Clone)]
pub(crate) struct Server {
    state: Arc<State>,
    workers: Arc<Semaphore>,
    whole_text: Arc<Semaphore>,
    tool_router: ToolRouter<Self>,
}

/// What every tool call reads: where Channels are found, the default Archive and the open
/// Archives.
#[derive(Debug)]
pub(crate) struct State {
    locations: Locations,
    default_archive: Option<PathBuf>,
    archives: Archives,
    model: Gate,
}

impl Server {
    /// Creates the server; `default_archive` serves calls that name no Archive.
    pub(crate) fn new(
        locations: Locations,
        default_archive: Option<PathBuf>,
        keys: KeyChoice,
    ) -> Self {
        Self {
            state: Arc::new(State {
                locations,
                default_archive,
                archives: Archives::new(keys, Gate::container()),
                model: Gate::model(),
            }),
            workers: Arc::new(Semaphore::new(
                std::thread::available_parallelism().map_or(1, NonZeroUsize::get),
            )),
            whole_text: Arc::new(Semaphore::new(WHOLE_TEXT_DECODERS)),
            tool_router: Self::tool_router()
                + Self::content_router()
                + Self::model_router()
                + Self::audio_router()
                + Self::record_router()
                + Self::query_router()
                + Self::image_router()
                + Self::material_router()
                + Self::loadout_router(),
        }
    }

    /// Returns the permits for decoding an Entry as a whole, at most four at a time.
    pub(crate) fn whole_text(&self) -> Arc<Semaphore> {
        Arc::clone(&self.whole_text)
    }

    /// Reads the DataForge of the Archive `archive` names if that has not happened yet. It waits
    /// on the blocking pool without a worker permit, so calls waiting for the read do not keep
    /// the workers from unrelated tools.
    pub(crate) async fn preload(&self, archive: Option<String>) -> Result<(), String> {
        let state = Arc::clone(&self.state);
        let outcome = tokio::task::spawn_blocking(move || {
            let loaded = state.forge(archive.as_deref())?.load_dataforge();
            noted::clear();
            loaded
        })
        .await;
        match outcome {
            Ok(Ok(())) => Ok(()),
            Ok(Err(error)) => Err(error.message()),
            Err(_) => Err(ToolError::Stopped.message()),
        }
    }
    /// Runs `work` on the blocking pool, at most one call per CPU at a time.
    pub(crate) async fn run<T, F>(&self, work: F) -> Result<Noted<T>, String>
    where
        T: Send + 'static,
        F: FnOnce(&State) -> Result<T, ToolError> + Send + 'static,
    {
        self.run_plain(work)
            .await
            .map(|(value, notes)| Noted { value, notes })
    }

    /// Like [`Server::run`], with the key order of the result kept in its text content.
    pub(crate) async fn run_ordered<T, F>(&self, work: F) -> Result<Ordered<T>, String>
    where
        T: Send + 'static,
        F: FnOnce(&State) -> Result<T, ToolError> + Send + 'static,
    {
        self.run_plain(work)
            .await
            .map(|(value, notes)| Ordered { value, notes })
    }

    async fn run_plain<T, F>(&self, work: F) -> Result<(T, Notes), String>
    where
        T: Send + 'static,
        F: FnOnce(&State) -> Result<T, ToolError> + Send + 'static,
    {
        let Ok(permit) = Arc::clone(&self.workers).acquire_owned().await else {
            return Err(ToolError::Stopped.message());
        };
        let state = Arc::clone(&self.state);
        let outcome = tokio::task::spawn_blocking(move || {
            noted::clear();
            let result = work(&state);
            let notes = noted::take();
            drop(permit);
            result.map(|value| (value, notes))
        })
        .await;
        match outcome {
            Ok(Ok(done)) => Ok(done),
            Ok(Err(error)) => Err(error.message()),
            Err(_) => Err(ToolError::Stopped.message()),
        }
    }
}

impl State {
    /// Returns the permits for reading chunk files, models and textures, which may take more than
    /// 1 GiB. Take one with [`Gate::hold`] inside the call, which already holds its worker permit,
    /// and open no container while holding it: resolve every path first.
    pub(crate) const fn model(&self) -> &Gate {
        &self.model
    }

    /// Returns the Archive `archive` names, or the default Archive when it is `None`.
    pub(crate) fn archive(&self, archive: Option<&str>) -> Result<Arc<Opened>, ToolError> {
        let path = self.path(archive, Accepts::Archive)?;
        let opened = self.archives.get(&path)?;
        noted::remember(opened.notice());
        key_report::watch(&opened);
        Ok(opened)
    }

    /// Returns the DataForge input `archive` names, an Archive or a DataForge file, or the
    /// default Archive when it is `None`.
    pub(crate) fn forge(&self, archive: Option<&str>) -> Result<ForgeSource, ToolError> {
        let path = self.path(archive, Accepts::Forge)?;
        let source = self.archives.forge(&path)?;
        if let ForgeSource::Archive(opened) = &source {
            noted::remember(opened.notice());
            key_report::watch(opened);
        }
        Ok(source)
    }

    fn path(&self, archive: Option<&str>, accepts: Accepts) -> Result<PathBuf, ToolError> {
        archive.map_or_else(
            || self.default_archive.clone().ok_or(ToolError::NoArchive),
            |text| self.locations.resolve(text, accepts),
        )
    }
}

#[tool_router(vis = "pub(crate)")]
impl Server {
    #[tool(
        description = "List the installed Channels of Star Citizen (LIVE, PTU, ...) with their Archive path and Build.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<ChannelsResult>(),
        annotations(
            title = "List Channels",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_channels(
        &self,
        Parameters(params): Parameters<ChannelsParams>,
    ) -> Result<Noted<ChannelsResult>, String> {
        self.run(move |state| {
            let page = Page::new(&"list_channels", params.cursor.as_deref(), params.limit)?;
            let channels = state.locations.channels()?;
            Ok(ChannelsResult {
                total: channels.len(),
                channels: page
                    .slice(&channels)
                    .iter()
                    .map(ChannelShape::from)
                    .collect(),
                next_cursor: page.next_cursor(channels.len())?,
            })
        })
        .await
    }

    #[tool(
        description = "Summarise an Archive: its Channel, Build, number of Entries, encrypted Entries and sizes.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<ArchiveInfo>(),
        annotations(
            title = "Archive info",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn archive_info(
        &self,
        Parameters(params): Parameters<ArchiveParams>,
    ) -> Result<Noted<ArchiveInfo>, String> {
        self.run(move |state| {
            let opened = state.archive(params.archive.as_deref())?;
            Ok(entries::info(
                opened.archive(),
                &state.locations.public_patches(),
            ))
        })
        .await
    }

    #[tool(
        description = "List one folder of an Archive: its subfolders, then its Entries. Folder paths ignore case and separator kind; omit folder for the top level. A folder `outer.socpak::` or `outer.socpak::folder` lists inside that container; the paths returned work as they are in the read tools. The content may be forwarded by the client to third parties; game texts and media must not be redistributed.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<FolderResult>(),
        annotations(
            title = "List Entries",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn list_entries(
        &self,
        Parameters(params): Parameters<ListParams>,
    ) -> Result<Noted<FolderResult>, String> {
        self.run(move |state| {
            let folder = capped(
                "folder",
                params.folder.as_deref().unwrap_or_default(),
                MAX_PATH_LEN,
            )?;
            let opened = state.archive(params.archive.as_deref())?;
            let listing = ("list_entries", opened.version(), folder);
            let page = Page::new(&listing, params.cursor.as_deref(), params.limit)?;
            let (place, inner) = opened.place(folder)?;
            entries::list(&place, inner, page)
        })
        .await
    }

    #[tool(
        description = "Search the Entries of an Archive by path. A filter with * or ? is a glob over the whole path (* also matches separators); any other filter matches anywhere in the path. Case and separator kind do not matter. A filter is at most 1024 bytes with at most 32 * and ?, otherwise the call is an error. container searches inside a socpak container instead. The content may be forwarded by the client to third parties; game texts and media must not be redistributed.",
        output_schema = rmcp::handler::server::tool::schema_for_output::<SearchResult>(),
        annotations(
            title = "Search Entries",
            read_only_hint = true,
            destructive_hint = false,
            idempotent_hint = true,
            open_world_hint = false
        )
    )]
    async fn search_entries(
        &self,
        Parameters(params): Parameters<SearchParams>,
    ) -> Result<Noted<SearchResult>, String> {
        self.run(move |state| {
            let filter = capped("filter", &params.filter, MAX_PATH_LEN)?;
            let container = capped(
                "container",
                params.container.as_deref().unwrap_or_default(),
                MAX_PATH_LEN,
            )?
            .trim_end_matches(crate::container::SEPARATOR);
            let opened = state.archive(params.archive.as_deref())?;
            let listing = ("search_entries", opened.version(), filter, container);
            let page = Page::new(&listing, params.cursor.as_deref(), params.limit)?;
            let place = if container.is_empty() {
                crate::container::Place::Main(&opened)
            } else {
                let rooted = format!("{container}{}", crate::container::SEPARATOR);
                opened.place(&rooted)?.0
            };
            entries::search(&place, filter, page)
        })
        .await
    }
}

#[expect(
    clippy::unused_async_trait_impl,
    reason = "the SDK macro generates async handlers that need no await"
)]
#[tool_handler(router = self.tool_router)]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
                    .with_title("P4KX"),
            )
            .with_instructions(INSTRUCTIONS)
    }
}

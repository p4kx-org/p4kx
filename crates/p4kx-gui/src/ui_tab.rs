// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod contents;
mod export;
mod files;
mod flow;
mod job;
mod output_picture;
mod pick;
mod render;
mod report;
mod settings;
mod ships;
mod still;
mod variables;
mod view;

use std::sync::Arc;

use p4kx_assembly::EntityCatalogue;
use p4kx_extraction::ScreenTexts;

pub use contents::{ContentRow, Fact, MAX_CONTENT_ROWS, Note, RowDetail, UiContents};
pub use export::{UiExport, UiExportStatus, UiExportSummary};
pub use files::FileList;
pub(crate) use output_picture::OutputPicture;
pub use pick::{ShipPick, UiScreensStatus};
pub use render::{UiRendered, UiSource};
pub use report::{ReportLine, UiReport};
pub use settings::{UiBackground, UiFormat, UiPower, UiSettings, UiSize};
pub use ships::ShipList;
pub use view::{UiDetail, UiFilesStatus, UiMode, UiRenderFailure};

use crate::{EntryIndex, TextLanguage};
use job::Job;
use render::{Built, Fail, RenderKey};

type Drawing = Result<Built, Fail>;

#[derive(Debug, Default)]
struct Rendering {
    wanted: Option<RenderKey>,
    requested: Option<RenderKey>,
    job: Option<Job<Drawing>>,
    shown: Option<UiRendered>,
    failure: Option<UiRenderFailure>,
}

#[derive(Debug, Default)]
enum Files {
    #[default]
    Idle,
    Loading(Job<files::Outcome>),
    Failed(Option<String>),
    ArchiveChanged,
    Ready(Box<FileList>),
}

/// The UI tab: the GFx movies of the open Archive, and the screens of its ships, each drawn as
/// `p4kx ui render` draws them, with the choices of size, background and format, and the export
/// of the picture.
#[derive(Debug, Default)]
pub struct UiTab {
    mode: UiMode,
    detail: UiDetail,
    settings: UiSettings,
    variables: String,
    files_query: String,
    ships_query: String,
    files: Files,
    file: Option<String>,
    ships: Option<ShipList>,
    ship: Option<ShipPick>,
    rendering: Rendering,
    export: Option<UiExport>,
    texts: Option<(Option<TextLanguage>, Arc<ScreenTexts>)>,
    serial: u64,
}

impl UiTab {
    /// Returns what the tab lists: the files or the ships.
    #[must_use]
    pub const fn mode(&self) -> UiMode {
        self.mode
    }

    /// Lists the files or the ships.
    pub const fn set_mode(&mut self, mode: UiMode) {
        self.mode = mode;
    }

    /// Returns the part of the details that is shown.
    #[must_use]
    pub const fn detail(&self) -> UiDetail {
        self.detail
    }

    /// Shows the contents or the report below the picture.
    pub const fn set_detail(&mut self, detail: UiDetail) {
        self.detail = detail;
    }

    /// Returns the choices of how the tab draws and exports.
    #[must_use]
    pub const fn settings(&self) -> &UiSettings {
        &self.settings
    }

    /// Returns the choices and the typed game variables as the settings file stores them.
    #[must_use]
    pub fn stored_settings(&self) -> p4kx_settings::UiRenderSettings {
        self.settings.stored(&self.variables)
    }

    /// Replaces the choices of how the tab draws and exports.
    pub const fn set_settings(&mut self, settings: UiSettings) {
        self.settings = settings;
    }

    /// Draws in `size`.
    pub const fn set_size(&mut self, size: UiSize) {
        self.settings.size = size;
    }

    /// Draws on `background`.
    pub const fn set_background(&mut self, background: UiBackground) {
        self.settings.background = background;
    }

    /// Exports in `format`.
    pub const fn set_format(&mut self, format: UiFormat) {
        self.settings.format = format;
    }

    /// Draws screens in the game state `power`.
    pub const fn set_power(&mut self, power: UiPower) {
        self.settings.power = power;
    }

    /// Returns the game variables typed beside the state, as `NAME=VALUE` settings.
    #[must_use]
    pub fn variables_text(&self) -> &str {
        &self.variables
    }

    /// Draws screens with the game variables in `text`, `NAME=VALUE` settings separated by blanks
    /// or commas, after those of the state.
    ///
    /// The text is held to [`p4kx_settings::MAX_VARIABLES_BYTES`] bytes: a longer one is cut at
    /// the last separator before the limit, so that no setting is cut in two, and is empty when
    /// it has none.
    pub fn set_variables_text(&mut self, text: &str) {
        p4kx_settings::bound_variables(text).clone_into(&mut self.variables);
    }

    /// Returns the typed settings that are no `NAME=VALUE`.
    #[must_use]
    pub fn invalid_variables(&self) -> Vec<String> {
        variables::chosen(self.settings.power, &self.variables).invalid
    }

    /// Returns the search text of the list the mode shows.
    #[must_use]
    pub fn query(&self) -> &str {
        match self.mode {
            UiMode::Files => &self.files_query,
            UiMode::Ships => &self.ships_query,
        }
    }

    /// Searches the list the mode shows for `text`, ignoring case; every word has to occur.
    pub fn set_query(&mut self, text: &str) {
        match self.mode {
            UiMode::Files => {
                text.clone_into(&mut self.files_query);
                if let Files::Ready(list) = &mut self.files {
                    list.refilter(text);
                }
            }
            UiMode::Ships => {
                text.clone_into(&mut self.ships_query);
                if let Some(ships) = &mut self.ships {
                    ships.refilter(text);
                }
            }
        }
    }

    /// Returns where the list of files stands.
    #[must_use]
    pub fn files_status(&self) -> UiFilesStatus<'_> {
        match &self.files {
            Files::Idle => UiFilesStatus::Idle,
            Files::Loading(_) => UiFilesStatus::Loading,
            Files::Failed(reason) => UiFilesStatus::Failed(reason.as_deref()),
            Files::ArchiveChanged => UiFilesStatus::ArchiveChanged,
            Files::Ready(list) if list.total() == 0 => UiFilesStatus::NoFiles,
            Files::Ready(list) => UiFilesStatus::Ready(list),
        }
    }

    /// Returns whether the files are listed or being listed.
    #[must_use]
    pub const fn is_files_started(&self) -> bool {
        !matches!(self.files, Files::Idle)
    }

    /// Starts listing the GFx and SWF Entries of `index` off the UI thread, calling `wake` from
    /// it once it ended; returns whether it started. Nothing starts when they are listed or
    /// being listed.
    pub(crate) fn load_files(
        &mut self,
        index: &Arc<EntryIndex>,
        wake: impl Fn() + Send + 'static,
    ) -> bool {
        if self.is_files_started() {
            return false;
        }
        self.files = match files::start(index, wake) {
            Ok(job) => Files::Loading(job),
            Err(reason) => Files::Failed(Some(reason)),
        };
        true
    }

    /// Returns the ships and vehicles the picker lists, once the Entities are listed.
    #[must_use]
    pub const fn ships(&self) -> Option<&ShipList> {
        self.ships.as_ref()
    }

    /// Takes the catalogue of the Entities tab as the list of ships, or forgets the list when
    /// there is none.
    pub(crate) fn sync_ships(&mut self, catalogue: Option<&Arc<EntityCatalogue>>) {
        match catalogue {
            Some(catalogue) if !self.ships.as_ref().is_some_and(|s| s.is_of(catalogue)) => {
                let mut list = ShipList::new(Arc::clone(catalogue));
                list.refilter(&self.ships_query);
                self.ships = Some(list);
            }
            Some(_) => {}
            None => self.ships = None,
        }
    }

    /// Returns the path of the picked file.
    #[must_use]
    pub fn selected_file(&self) -> Option<&str> {
        self.file.as_deref()
    }

    /// Picks the file at `position` of the shown ones.
    pub fn select_file(&mut self, position: usize) {
        if let Files::Ready(list) = &self.files
            && let Some(path) = list.get(position)
        {
            self.file = Some(path.to_owned());
        }
    }

    /// Returns the picked ship or vehicle with its screens.
    #[must_use]
    pub const fn ship(&self) -> Option<&ShipPick> {
        self.ship.as_ref()
    }

    /// Picks the ship or vehicle at `position` of the shown ones and lists its screens off the
    /// UI thread, calling `wake` from it as it ends. Picking the one that is picked keeps it.
    pub(crate) fn select_ship(
        &mut self,
        position: usize,
        index: &Arc<EntryIndex>,
        wake: impl Fn() + Send + 'static,
    ) {
        let Some(entity) = self.ships.as_ref().and_then(|ships| ships.get(position)) else {
            return;
        };
        let (guid, class_name) = (entity.guid(), entity.class_name().to_owned());
        if self.ship.as_ref().is_some_and(|ship| ship.guid() == guid) {
            return;
        }
        let job = ships::list_screens(index, guid, wake);
        self.ship = Some(ShipPick::new(guid, &class_name, job));
    }

    /// Picks the screen at `position` of the listing of the picked ship.
    pub fn pick_screen(&mut self, position: usize) {
        if let Some(ship) = &mut self.ship {
            ship.pick(position);
        }
    }

    /// Returns the picture that was drawn last for what is picked.
    #[must_use]
    pub const fn rendered(&self) -> Option<&UiRendered> {
        self.rendering.shown.as_ref()
    }

    /// Returns whether a picture is being drawn.
    #[must_use]
    pub const fn is_rendering(&self) -> bool {
        self.rendering.job.is_some()
    }

    /// Returns why the last picture could not be drawn.
    #[must_use]
    pub const fn render_failure(&self) -> Option<&UiRenderFailure> {
        self.rendering.failure.as_ref()
    }

    /// Returns what the picked file or screen is made of.
    #[must_use]
    pub fn contents(&self) -> Option<&UiContents> {
        match self.mode {
            UiMode::Files => self
                .rendering
                .shown
                .as_ref()
                .filter(|shown| self.file.as_deref().is_some_and(|f| shown_is(shown, f)))
                .and_then(UiRendered::contents),
            UiMode::Ships => self.ship.as_ref().and_then(ShipPick::contents),
        }
    }

    /// Returns the running or finished export.
    #[must_use]
    pub const fn export(&self) -> Option<&UiExport> {
        self.export.as_ref()
    }

    /// Dismisses the outcome of an export that ended.
    pub fn dismiss_export(&mut self) {
        if self
            .export
            .as_ref()
            .is_some_and(|export| !export.is_running())
        {
            self.export = None;
        }
    }

    /// Forgets the lists, the picks and the picture, because another Archive was opened.
    pub(crate) fn archive_opened(&mut self) {
        let (mode, detail, settings) = (self.mode, self.detail, self.settings);
        let variables = std::mem::take(&mut self.variables);
        *self = Self {
            mode,
            detail,
            settings,
            variables,
            ..Self::default()
        };
    }

    /// Stops what the tab shows of the Archive, because it changed on disk.
    pub(crate) fn archive_changed(&mut self) {
        if self.is_files_started() {
            self.files = Files::ArchiveChanged;
        }
        self.rendering = Rendering::default();
        self.export = None;
        self.texts = None;
    }
}

fn shown_is(shown: &UiRendered, path: &str) -> bool {
    matches!(&shown.source, UiSource::Movie(movie) if movie == path)
}

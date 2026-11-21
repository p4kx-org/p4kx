// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use p4kx_extraction::ArchiveError;
use p4kx_settings::{ExtractionAudio, ExtractionTextures, RecentArchives};

use self::folder_conversion_control::FolderConversionState;

mod archive_change;
mod archive_key;
mod assembly_export_control;
mod audio_control;
mod catalog_control;
mod channel_discovery;
mod characters_control;
mod content_navigation;
mod describe;
mod extraction_control;
mod folder_conversion_control;
mod game_watch;
mod model_export_control;
mod opening;
#[cfg(test)]
mod opening_tests;
mod record_export_control;
mod record_text;
mod sounds_control;
mod ui_control;
mod update;
mod viewer_control;
mod work_tab;

use archive_key::KeyState;
pub use archive_key::{KeyDialog, KeyProblem, KeyReason};
use assembly_export_control::AssemblyState;
use catalog_control::CatalogState;
use content_navigation::PendingHit;
pub(crate) use describe::describe;
pub use game_watch::GAME_CHECK_INTERVAL;
use game_watch::GameWatch;
use model_export_control::ModelExportState;
use opening::open_index;
use record_export_control::RecordExportState;
use update::UpdateState;
pub use work_tab::WorkTab;

use crate::{
    AudioOutput, Browser, ChannelList, CharactersTab, ComparisonTab, ContentSearchTab,
    DeviceOutput, EntryIndex, Extraction, ExtractionSetup, Language, LocalizationTab, PreviewTab,
    QueryTab, RecordTextChoice, SoundsTab, TagsTab, Texts, UiTab, ViewerSupport,
};

/// Why an Archive could not be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    /// The Archive that was opened.
    pub path: PathBuf,
    /// The error and its causes, joined by colons, or `None` when the opening stopped without
    /// a result.
    pub reason: Option<String>,
}

struct Opening {
    path: PathBuf,
    result: Receiver<crate::protected_open::ProtectedResult<EntryIndex, ArchiveError>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HelpWindow {
    FontLicence,
    About,
}

/// The whole state of the window, independent of how it is drawn.
pub struct Model {
    texts: Texts,
    channels: ChannelList,
    channel_refresh: crate::channel_refresh::ChannelRefresh,
    recent: RecentArchives,
    browser: Option<Browser>,
    opening: Option<Opening>,
    failure: Option<Failure>,
    help_window: Option<HelpWindow>,
    preview_tab: PreviewTab,
    extraction_folder: Option<PathBuf>,
    extraction_conversion: bool,
    extraction_textures: ExtractionTextures,
    extraction_audio: ExtractionAudio,
    extraction_setup: Option<ExtractionSetup>,
    extraction: Option<Extraction>,
    folder_conversion: FolderConversionState,
    update: UpdateState,
    work_tab: WorkTab,
    comparison: ComparisonTab,
    stopped_by_change: bool,
    audio_output: Box<dyn AudioOutput>,
    audio_volume: f32,
    query: QueryTab,
    localization: LocalizationTab,
    tags: TagsTab,
    ui: UiTab,
    sounds: SoundsTab,
    content_search: ContentSearchTab,
    pending_hit: Option<PendingHit>,
    record_export: RecordExportState,
    glb_export: ModelExportState,
    assembly: AssemblyState,
    characters: CharactersTab,
    catalog: CatalogState,
    record_text_choice: Option<RecordTextChoice>,
    viewer_support: ViewerSupport,
    key: KeyState,
    game: GameWatch,
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Model")
            .field("texts", &self.texts)
            .field("channels", &self.channels)
            .field("recent", &self.recent)
            .field("browser", &self.browser)
            .field("opening", &self.opening_path())
            .field("failure", &self.failure)
            .field("help_window", &self.help_window)
            .field("preview_tab", &self.preview_tab)
            .field("extraction_folder", &self.extraction_folder)
            .field("extraction_conversion", &self.extraction_conversion)
            .field("extraction_textures", &self.extraction_textures)
            .field("extraction_audio", &self.extraction_audio)
            .field("extraction_setup", &self.extraction_setup)
            .field("extraction", &self.extraction)
            .field("update", &self.update)
            .field("work_tab", &self.work_tab)
            .field("comparison", &self.comparison)
            .field("stopped_by_change", &self.stopped_by_change)
            .field("query", &self.query)
            .field("localization", &self.localization)
            .field("tags", &self.tags)
            .field("ui", &self.ui)
            .field("sounds", &self.sounds)
            .field("content_search", &self.content_search)
            .field("record_export", &self.record_export)
            .field("glb_export", &self.glb_export)
            .field("characters", &self.characters)
            .field("catalog", &self.catalog)
            .field("record_text_choice", &self.record_text_choice)
            .field("key", &self.key)
            .field("game", &self.game)
            .field("audio_volume", &self.audio_volume)
            .finish_non_exhaustive()
    }
}

impl Model {
    /// Creates the state of a window without an Archive, in `language`.
    #[must_use]
    pub fn new(language: Language) -> Self {
        Self {
            texts: Texts::new(language),
            channels: ChannelList::default(),
            channel_refresh: crate::channel_refresh::ChannelRefresh::default(),
            recent: RecentArchives::default(),
            browser: None,
            opening: None,
            failure: None,
            help_window: None,
            preview_tab: PreviewTab::Text,
            extraction_folder: None,
            extraction_conversion: false,
            extraction_textures: ExtractionTextures::default(),
            extraction_audio: ExtractionAudio::default(),
            extraction_setup: None,
            extraction: None,
            folder_conversion: FolderConversionState::default(),
            update: UpdateState::default(),
            work_tab: WorkTab::default(),
            comparison: ComparisonTab::default(),
            stopped_by_change: false,
            audio_output: Box::new(DeviceOutput),
            audio_volume: 1.0,
            query: QueryTab::default(),
            localization: LocalizationTab::default(),
            tags: TagsTab::default(),
            ui: UiTab::default(),
            sounds: SoundsTab::default(),
            content_search: ContentSearchTab::default(),
            pending_hit: None,
            record_export: RecordExportState::default(),
            glb_export: ModelExportState::default(),
            assembly: AssemblyState::default(),
            characters: CharactersTab::default(),
            catalog: CatalogState::default(),
            record_text_choice: None,
            viewer_support: ViewerSupport::default(),
            key: KeyState::default(),
            game: GameWatch::default(),
        }
    }

    /// Returns the interface texts in the current language.
    #[must_use]
    pub const fn texts(&self) -> &Texts {
        &self.texts
    }

    /// Switches the interface language.
    pub fn set_language(&mut self, language: Language) {
        self.texts.set_language(language);
        self.apply_record_language();
    }

    /// Returns the Channels the picker offers.
    #[must_use]
    pub const fn channels(&self) -> &ChannelList {
        &self.channels
    }

    /// Returns the recently opened Archives.
    #[must_use]
    pub const fn recent(&self) -> &RecentArchives {
        &self.recent
    }

    /// Replaces the recently opened Archives.
    pub fn set_recent(&mut self, recent: RecentArchives) {
        self.recent = recent;
    }

    /// Starts opening the Archive at `path`, or the DataForge file whose content it is, on a
    /// background thread, replacing an opening that is still running; `wake` is called from that
    /// thread once the result is ready.
    pub fn open(&mut self, path: PathBuf, wake: impl Fn() + Send + 'static) {
        let (sender, result) = mpsc::channel();
        let thread_path = path.clone();
        let key = self.key_in_use();
        let spawned = thread::Builder::new()
            .name("open-archive".to_owned())
            .spawn(move || {
                if sender.send(open_index(&thread_path, key)).is_ok() {
                    wake();
                }
            });
        self.opening = None;
        self.recheck_game();
        match spawned {
            Ok(_handle) => {
                self.failure = None;
                self.opening = Some(Opening { path, result });
            }
            Err(error) => self.fail(path, &error),
        }
    }

    /// Takes the result of a finished opening; returns whether one finished.
    pub fn poll(&mut self) -> bool {
        self.poll_channel_refresh();
        let Some(opening) = &self.opening else {
            return false;
        };
        let received = match opening.result.try_recv() {
            Ok(received) => received,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => {
                let path = opening.path.clone();
                self.opening = None;
                self.failure = Some(Failure { path, reason: None });
                return true;
            }
        };
        let Some(opening) = self.opening.take() else {
            return false;
        };
        self.channels.record_protection_issues(received.issues);
        match received.result {
            Ok(index) => {
                let path = opening.path;
                self.recent.push(std::path::absolute(&path).unwrap_or(path));
                self.browser = Some(Browser::new(index));
                if self.is_dataforge_file() {
                    self.preview_tab = PreviewTab::DataForge;
                    if self.work_tab.needs_entries() {
                        self.work_tab = WorkTab::Archive;
                    }
                }
                self.apply_record_language();
                self.stopped_by_change = false;
                self.key_archive_opened();
                self.query.archive_opened();
                self.localization.archive_opened();
                self.tags.archive_opened();
                self.ui.archive_opened();
                self.assembly.tab.archive_opened();
                self.characters.reset(false);
                self.sounds.archive_opened();
                self.content_search.archive_opened();
                self.pending_hit = None;
            }
            Err(error) => self.fail(opening.path, &error),
        }
        true
    }

    /// Returns the Archive being opened, or `None` when nothing is opening.
    #[must_use]
    pub fn opening_path(&self) -> Option<&Path> {
        self.opening.as_ref().map(|opening| opening.path.as_path())
    }

    /// Returns why the last opening failed, or `None` when it did not.
    #[must_use]
    pub const fn failure(&self) -> Option<&Failure> {
        self.failure.as_ref()
    }

    /// Returns the browsing state of the open Archive, or `None` when none is open.
    #[must_use]
    pub const fn browser(&self) -> Option<&Browser> {
        self.browser.as_ref()
    }

    /// Returns the browsing state of the open Archive for changes.
    pub const fn browser_mut(&mut self) -> Option<&mut Browser> {
        self.browser.as_mut()
    }

    /// Returns whether the About window is shown.
    #[must_use]
    pub fn about_shown(&self) -> bool {
        self.help_window == Some(HelpWindow::About)
    }

    /// Shows or hides the About window.
    pub fn set_about_shown(&mut self, shown: bool) {
        self.set_help_window(HelpWindow::About, shown);
    }

    /// Returns whether the font licence is shown.
    #[must_use]
    pub fn licence_shown(&self) -> bool {
        self.help_window == Some(HelpWindow::FontLicence)
    }

    /// Shows or hides the font licence.
    pub fn set_licence_shown(&mut self, shown: bool) {
        self.set_help_window(HelpWindow::FontLicence, shown);
    }

    fn set_help_window(&mut self, window: HelpWindow, shown: bool) {
        if shown {
            self.help_window = Some(window);
        } else if self.help_window == Some(window) {
            self.help_window = None;
        }
    }

    /// Returns the tab the preview is shown in.
    #[must_use]
    pub const fn preview_tab(&self) -> PreviewTab {
        self.preview_tab
    }

    /// Shows the preview in `tab`.
    pub fn set_preview_tab(&mut self, tab: PreviewTab) {
        self.preview_tab = tab;
        if tab != PreviewTab::Audio {
            self.pause_playback();
        }
    }

    fn fail(&mut self, path: PathBuf, error: &dyn Error) {
        self.failure = Some(Failure {
            path,
            reason: Some(describe(error)),
        });
    }
}

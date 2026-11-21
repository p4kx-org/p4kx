// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;
use std::sync::Arc;

use super::export::{self, UiExport};
use super::files::{self, FileList};
use super::job::{Job, Poll};
use super::pick::ShipPick;
use super::render::{self, Fail, RenderKey, UiRendered, UiSource};
use super::variables;
use super::view::{UiMode, UiRenderFailure};
use super::{Files, Rendering, UiTab};
use crate::installation::refusal_of;
use crate::{EntryIndex, TextLanguage};

impl UiTab {
    pub(super) fn wanted(&self, language: Option<TextLanguage>) -> Option<RenderKey> {
        let (source, variables) = match self.mode {
            UiMode::Files => (UiSource::Movie(self.file.clone()?), Vec::new()),
            UiMode::Ships => {
                let ship = self.ship.as_ref()?;
                let source = UiSource::Screen {
                    canvas: ship.screen()?.canvas.clone(),
                    brand: ship.brand().map(str::to_owned),
                };
                (
                    source,
                    variables::chosen(self.settings.power, &self.variables).variables,
                )
            }
        };
        Some(RenderKey {
            source,
            size: self.settings.size,
            background: self.settings.background,
            language,
            variables,
        })
    }

    /// Draws the picture of what is picked off the UI thread, unless it is drawn or being drawn
    /// for these choices and `language`; calls `wake` from the thread once it ended.
    pub(crate) fn sync_render(
        &mut self,
        index: &Arc<EntryIndex>,
        language: Option<TextLanguage>,
        wake: impl Fn() + Send + 'static,
    ) {
        let Some(key) = self.wanted(language) else {
            self.rendering = Rendering::default();
            return;
        };
        self.rendering.wanted = Some(key.clone());
        if self.rendering.requested.as_ref() == Some(&key) {
            return;
        }
        if let Some(job) = &self.rendering.job {
            job.stop();
            return;
        }
        if self
            .rendering
            .shown
            .as_ref()
            .is_some_and(|shown| shown.source != key.source)
        {
            self.rendering.shown = None;
        }
        let cached = self
            .texts
            .as_ref()
            .filter(|(cached, _)| *cached == key.language)
            .map(|(_, texts)| Arc::clone(texts));
        let (task_index, task_key) = (Arc::clone(index), key.clone());
        let spawned = Job::spawn_stoppable("ui-render", wake, move |cancel| {
            render::draw(&task_index, &task_key, cached, cancel)
        });
        match spawned {
            Ok(job) => {
                self.rendering.job = Some(job);
                self.rendering.failure = None;
            }
            Err(reason) => {
                self.rendering.job = None;
                self.rendering.failure = Some(UiRenderFailure::Failed(Some(reason)));
            }
        }
        self.rendering.requested = Some(key);
    }

    /// Takes the screens, the file list and the picture once they are ready, and the outcome of
    /// the export once it ended; returns whether something changed.
    pub fn poll(&mut self) -> bool {
        let mut changed = self.ship.as_mut().is_some_and(ShipPick::poll);
        changed |= self.export.as_mut().is_some_and(UiExport::poll);
        changed |= self.poll_files();
        changed |= self.poll_render();
        changed
    }

    fn poll_files(&mut self) -> bool {
        let Files::Loading(job) = &self.files else {
            return false;
        };
        self.files = match job.poll() {
            Poll::Pending => return false,
            Poll::Ready(files::Outcome::Ready(paths)) => {
                let mut list = FileList::new(paths);
                list.refilter(&self.files_query);
                Files::Ready(Box::new(list))
            }
            Poll::Ready(files::Outcome::ArchiveChanged) => Files::ArchiveChanged,
            Poll::Lost => Files::Failed(None),
        };
        true
    }

    fn poll_render(&mut self) -> bool {
        let Some(job) = &self.rendering.job else {
            return false;
        };
        let outcome = match job.poll() {
            Poll::Pending => return false,
            Poll::Ready(outcome) => outcome,
            Poll::Lost => Err(Fail::Reason(String::new())),
        };
        self.rendering.job = None;
        let stale = self.rendering.requested != self.rendering.wanted;
        let Some(key) = self.rendering.requested.clone() else {
            return true;
        };
        if stale || matches!(outcome, Err(Fail::Stopped)) {
            self.rendering.requested = None;
            return true;
        }
        match outcome {
            Ok(built) => {
                self.serial += 1;
                if let Some(texts) = built.texts {
                    self.texts = Some((key.language, texts));
                }
                self.rendering.shown = Some(UiRendered {
                    serial: self.serial,
                    source: key.source,
                    image: Arc::new(built.image),
                    report: built.report,
                    contents: built.contents,
                });
                self.rendering.failure = None;
            }
            Err(failure) => {
                self.rendering.shown = None;
                self.rendering.failure = Some(match failure {
                    Fail::ArchiveChanged => UiRenderFailure::ArchiveChanged,
                    Fail::Protected(error) => UiRenderFailure::Failed(Some(error.to_string())),
                    Fail::Stopped => UiRenderFailure::Failed(None),
                    Fail::Reason(reason) if reason.is_empty() => UiRenderFailure::Failed(None),
                    Fail::Reason(reason) => UiRenderFailure::Failed(Some(reason)),
                });
            }
        }
        true
    }

    /// Returns whether what is picked can be exported: it is picked and no export runs.
    #[must_use]
    pub fn export_available(&self) -> bool {
        self.wanted(None).is_some() && !self.export.as_ref().is_some_and(UiExport::is_running)
    }

    /// Returns the file name the save dialog proposes for the export of what is picked.
    #[must_use]
    pub fn proposed_file_name(&self) -> String {
        let extension = self.settings.format.extension();
        let stem = match (self.mode, &self.ship) {
            (UiMode::Files, _) => self
                .file
                .as_deref()
                .and_then(|path| path.rsplit(['/', '\\']).next())
                .and_then(|name| name.rsplit_once('.').map(|(stem, _)| stem))
                .unwrap_or("ui")
                .to_owned(),
            (UiMode::Ships, Some(ship)) => {
                let screen = ship
                    .screen()
                    .and_then(|screen| screen.canvas.rsplit('/').next())
                    .unwrap_or("screen");
                format!("{}_{screen}", ship.class_name())
            }
            (UiMode::Ships, None) => "screen".to_owned(),
        };
        format!("{stem}.{extension}")
    }

    /// Draws what is picked again, as the choices say, and writes it to `file` off the UI
    /// thread, calling `wake` from it once it ended; returns whether it started.
    ///
    /// Nothing starts while nothing is picked or an export runs. The file is written whole or
    /// not at all, and never over the open Archive.
    pub(crate) fn start_export(
        &mut self,
        index: &Arc<EntryIndex>,
        (file, language): (PathBuf, Option<TextLanguage>),
        wake: impl Fn() + Send + 'static,
    ) -> bool {
        if !self.export_available() {
            return false;
        }
        let Some(key) = self.wanted(language) else {
            return false;
        };
        let texts = self
            .texts
            .as_ref()
            .filter(|(cached, _)| *cached == key.language)
            .map(|(_, texts)| Arc::clone(texts));
        if let Some(refused) = refusal_of(file.parent()) {
            self.export = Some(export::refused(file, refused));
            return true;
        }
        let request = export::Request {
            key,
            format: self.settings.format,
            file,
            texts,
        };
        self.export = Some(export::start(index, request, wake));
        true
    }
}

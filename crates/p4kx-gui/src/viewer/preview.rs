// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use p4kx_extraction::{CancelToken, ModelSource, PathIndex};
use p4kx_render::{Camera, Scene, SceneError, Skeleton};

use super::model_info_provenance::ModelInfoProvenance;
use super::playback::Playback;
use super::preview_animations::{Animated, can_animate, read as read_animations};
use crate::{AnimationCount, EntryIndex, ModelInfo};

mod archive_read;

/// The largest model file or geometry companion, in bytes, the viewer reads.
pub const MODEL_FILE_LIMIT: u64 = 256 * 1024 * 1024;

/// Returns whether `path` names a model file the Model tab shows: a `.cgf`, `.cga`, `.skin` or
/// `.chr`.
#[must_use]
pub fn is_model_path(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, extension)| {
        ["cgf", "cga", "skin", "chr"]
            .iter()
            .any(|known| extension.eq_ignore_ascii_case(known))
    })
}

/// Where the model preview of a model Entry stands.
#[derive(Debug, Clone, Copy)]
pub enum ModelStatus<'a> {
    /// The model and its geometry companion are being read and flattened.
    Loading,
    /// The model could not be read, for the reason given, or reading stopped without a result.
    Failed(Option<&'a str>),
    /// The Archive changed on disk, so the model was not read or its data was dropped.
    ArchiveChanged,
    /// The model file or its companion is this many bytes, above [`MODEL_FILE_LIMIT`].
    FileTooLarge(u64),
    /// The model was read but has nothing the viewer can show or more than it takes.
    Unshowable(SceneError),
    /// The model was read and flattened.
    Ready(&'a Arc<Scene>),
}

#[derive(Debug)]
enum Event {
    Failed(Option<String>),
    ArchiveChanged,
    FileTooLarge(u64),
    Read(Box<Loaded>),
    Animations(Animated),
}

#[derive(Debug)]
struct Loaded {
    info: ModelInfo,
    scene: Result<Scene, SceneError>,
    info_provenance: io::Result<ModelInfoProvenance>,
}

#[derive(Debug)]
enum State {
    Loading,
    Failed(Option<String>),
    ArchiveChanged,
    FileTooLarge(u64),
    Unshowable(SceneError),
    Ready(Arc<Scene>),
}

/// The model preview of a model Entry: the model read and flattened on a background thread, and
/// the camera that looks at it.
///
/// The animations next to the model are read after the model: the preview is ready without them,
/// and [`Self::is_reading_animations`] tells while they are still read.
#[derive(Debug)]
pub struct ModelPreview {
    stop: CancelToken,
    events: Option<Receiver<Event>>,
    state: State,
    info: Option<Arc<ModelInfo>>,
    info_provenance: Option<io::Result<ModelInfoProvenance>>,
    camera: Option<Camera>,
    playback: Option<Playback>,
}

impl ModelPreview {
    /// Starts reading the model Entry `id` of `index` on a background thread, calling `wake`
    /// from it whenever a result is ready.
    pub(crate) fn start(
        index: &Arc<EntryIndex>,
        id: u32,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        let (sender, events) = mpsc::channel();
        let stop = CancelToken::new();
        let thread_index = Arc::clone(index);
        let thread_stop = stop.clone();
        let spawned = thread::Builder::new()
            .name("model-preview".to_owned())
            .spawn(move || {
                let send = |event| {
                    if !thread_stop.is_cancelled() && sender.send(event).is_ok() {
                        wake();
                    }
                };
                archive_read::load(&thread_index, id, &thread_stop, &send);
            });
        let (events, state) = match spawned {
            Ok(_handle) => (Some(events), State::Loading),
            Err(_ignored) => (None, State::Failed(None)),
        };
        Self {
            stop,
            events,
            state,
            info: None,
            info_provenance: None,
            camera: None,
            playback: None,
        }
    }

    /// Returns the facts of the model once it was read, also when the viewer cannot show it.
    #[must_use]
    pub const fn info(&self) -> Option<&Arc<ModelInfo>> {
        self.info.as_ref()
    }

    /// Returns the original inputs of the visible model facts.
    #[must_use]
    pub fn info_provenance(&self) -> Option<Result<&ModelInfoProvenance, &io::Error>> {
        self.info_provenance.as_ref().map(Result::as_ref)
    }

    /// Returns where the preview stands.
    #[must_use]
    pub fn status(&self) -> ModelStatus<'_> {
        match &self.state {
            State::Loading => ModelStatus::Loading,
            State::Failed(reason) => ModelStatus::Failed(reason.as_deref()),
            State::ArchiveChanged => ModelStatus::ArchiveChanged,
            State::FileTooLarge(size) => ModelStatus::FileTooLarge(*size),
            State::Unshowable(error) => ModelStatus::Unshowable(*error),
            State::Ready(scene) => ModelStatus::Ready(scene),
        }
    }

    /// Returns whether the model is still being read.
    #[must_use]
    pub const fn is_loading(&self) -> bool {
        matches!(self.state, State::Loading)
    }

    /// Returns whether the model was read and the animations next to it are still being read.
    #[must_use]
    pub fn is_reading_animations(&self) -> bool {
        self.info
            .as_ref()
            .is_some_and(|info| info.animations == AnimationCount::Reading)
    }

    /// Returns the flattened model once it was read.
    #[must_use]
    pub const fn scene(&self) -> Option<&Arc<Scene>> {
        match &self.state {
            State::Ready(scene) => Some(scene),
            _ => None,
        }
    }

    /// Returns the camera once the model was read.
    #[must_use]
    pub const fn camera(&self) -> Option<&Camera> {
        self.camera.as_ref()
    }

    /// Returns the camera for moving it once the model was read.
    pub const fn camera_mut(&mut self) -> Option<&mut Camera> {
        self.camera.as_mut()
    }

    /// Returns the skeleton overlay and animation playback of the model once it was read, or
    /// `None` for a model without a skeleton.
    #[must_use]
    pub const fn playback(&self) -> Option<&Playback> {
        self.playback.as_ref()
    }

    /// Returns the playback for controlling it.
    pub const fn playback_mut(&mut self) -> Option<&mut Playback> {
        self.playback.as_mut()
    }

    /// Takes the results the background thread sent; returns whether any arrived.
    pub(crate) fn poll(&mut self) -> bool {
        let mut arrived = false;
        while let Some(events) = &self.events {
            match events.try_recv() {
                Ok(event) => {
                    self.apply(event);
                    arrived = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.events = None;
                    arrived |= self.ended();
                }
            }
        }
        arrived
    }

    fn apply(&mut self, event: Event) {
        match event {
            Event::Failed(reason) => self.drop_model(State::Failed(reason)),
            Event::ArchiveChanged => self.drop_model(State::ArchiveChanged),
            Event::FileTooLarge(size) => self.drop_model(State::FileTooLarge(size)),
            Event::Read(read) => {
                let Loaded {
                    info,
                    scene,
                    info_provenance,
                } = *read;
                self.info = Some(Arc::new(info));
                self.info_provenance = Some(info_provenance);
                self.state = match scene {
                    Ok(scene) => {
                        self.camera = Some(Camera::framing(scene.bounds()));
                        self.playback = scene
                            .skeleton()
                            .map(|skeleton| Playback::new(Arc::clone(skeleton), Vec::new()));
                        State::Ready(Arc::new(scene))
                    }
                    Err(error) => State::Unshowable(error),
                };
            }
            Event::Animations(animated) => {
                self.set_count(animated.count);
                if let Some(playback) = &mut self.playback {
                    playback.set_animations(animated.movers);
                }
            }
        }
    }

    fn drop_model(&mut self, state: State) {
        self.state = state;
        self.events = None;
        self.info = None;
        self.info_provenance = None;
        self.camera = None;
        self.playback = None;
    }

    fn set_count(&mut self, count: AnimationCount) {
        if let Some(info) = self.info.take() {
            self.info = Some(Arc::new((*info).clone().with_animations(count)));
        }
    }

    fn ended(&mut self) -> bool {
        if self.is_loading() {
            self.state = State::Failed(None);
            true
        } else if self.is_reading_animations() {
            self.set_count(AnimationCount::Unread);
            true
        } else {
            false
        }
    }
}

impl Drop for ModelPreview {
    fn drop(&mut self) {
        self.stop.cancel();
    }
}

fn finish_model(
    index: &EntryIndex,
    paths: &PathIndex<'_>,
    path: &str,
    source: &ModelSource,
    stop: &CancelToken,
    send: &impl Fn(Event),
) {
    let scene = Scene::from_model(source.model());
    if stop.is_cancelled() {
        return;
    }
    if index.watch().check().is_err() {
        return send(Event::ArchiveChanged);
    }
    let skeleton: Option<Arc<Skeleton>> = scene
        .as_ref()
        .ok()
        .filter(|scene| scene.skin().is_some())
        .and_then(|scene| scene.skeleton().cloned());
    let animatable = can_animate(source.model());
    let count = if animatable {
        AnimationCount::Reading
    } else {
        AnimationCount::Counted(0)
    };
    let info = ModelInfo::of(source.model()).with_animations(count);
    let info_provenance = ModelInfoProvenance::capture(source);
    if stop.is_cancelled() {
        return;
    }
    if index.watch().check().is_err() {
        return send(Event::ArchiveChanged);
    }
    send(Event::Read(Box::new(Loaded {
        info,
        scene,
        info_provenance,
    })));
    if !animatable {
        return;
    }
    let Some(animated) = read_animations(paths, path, skeleton.as_ref(), stop) else {
        return;
    };
    if index.watch().check().is_err() {
        return send(Event::ArchiveChanged);
    }
    send(Event::Animations(animated));
}

#[cfg(test)]
mod events_tests;

#[cfg(test)]
mod tests {
    use super::is_model_path;

    #[test]
    fn model_files_are_recognised_by_extension_ignoring_case() {
        for path in [
            "Data\\Objects\\ship.cgf",
            "Data\\Objects\\ship.CGA",
            "data/objects/gun.skin",
        ] {
            assert!(is_model_path(path), "{path}");
        }
    }

    #[test]
    fn companions_and_other_files_are_not_model_files() {
        for path in ["ship.cgfm", "ship.skinm", "ship.dds", "cgf", "ship.cgf.xml"] {
            assert!(!is_model_path(path), "{path}");
        }
    }
}

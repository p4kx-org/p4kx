// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::io;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

use p4kx_channels::{DetectedChannels, DiscoveryReport, SearchRecipe};
use p4kx_settings::ManualChannels;

pub(crate) type Measurement = DiscoveryReport<DetectedChannels>;
type Work = Box<dyn FnOnce() + Send>;
type Wake = Box<dyn Fn() + Send + Sync>;

#[derive(Debug)]
pub(crate) struct ChannelSearch {
    pub(crate) recipe: Arc<SearchRecipe>,
    pub(crate) manual: ManualChannels,
}

impl ChannelSearch {
    pub(crate) fn measure(self) -> Measurement {
        self.recipe
            .snapshot_with_manual(&self.manual.folders, &self.manual.archives)
            .detect()
    }
}

#[derive(Debug)]
pub(crate) enum RefreshFailure {
    Spawn(io::Error),
    Stopped,
}

struct Request {
    measure: Box<dyn FnOnce() -> Measurement + Send>,
    wake: Wake,
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Request").finish_non_exhaustive()
    }
}

#[derive(Debug)]
struct Running {
    result: Receiver<Measurement>,
    current: bool,
}

#[derive(Debug, Default)]
pub(crate) struct ChannelRefresh {
    running: Option<Running>,
    pending: Option<Request>,
    failure: Option<RefreshFailure>,
}

impl ChannelRefresh {
    pub(crate) fn request(
        &mut self,
        search: ChannelSearch,
        wake: impl Fn() + Send + Sync + 'static,
    ) {
        self.request_with(search, wake, spawn);
    }

    pub(crate) fn request_with(
        &mut self,
        search: ChannelSearch,
        wake: impl Fn() + Send + Sync + 'static,
        spawn: impl FnOnce(Work) -> io::Result<()>,
    ) {
        self.queue_using(
            Request {
                measure: Box::new(move || search.measure()),
                wake: Box::new(wake),
            },
            spawn,
        );
    }

    pub(crate) fn invalidate(&mut self) {
        if let Some(running) = &mut self.running {
            running.current = false;
        }
        self.pending = None;
        self.failure = None;
    }

    pub(crate) const fn failure(&self) -> Option<&RefreshFailure> {
        self.failure.as_ref()
    }

    pub(crate) fn poll(&mut self) -> Option<Measurement> {
        self.poll_using(spawn)
    }

    fn queue_using(&mut self, request: Request, spawn: impl FnOnce(Work) -> io::Result<()>) {
        self.failure = None;
        if self.running.is_some() {
            self.pending = Some(request);
        } else {
            self.start_using(request, spawn);
        }
    }

    fn start_using(&mut self, request: Request, spawn: impl FnOnce(Work) -> io::Result<()>) {
        match launch(request, spawn) {
            Ok(running) => self.running = Some(running),
            Err(error) => self.failure = Some(RefreshFailure::Spawn(error)),
        }
    }

    fn poll_using(&mut self, spawn: impl FnOnce(Work) -> io::Result<()>) -> Option<Measurement> {
        let running = self.running.as_ref()?;
        let current = running.current;
        let measured = match running.result.try_recv() {
            Ok(measured) => Ok(measured),
            Err(TryRecvError::Empty) => return None,
            Err(TryRecvError::Disconnected) => Err(RefreshFailure::Stopped),
        };
        self.running = None;
        if let Some(request) = self.pending.take() {
            self.start_using(request, spawn);
            return None;
        }
        if !current {
            return None;
        }
        match measured {
            Ok(measured) => {
                self.failure = None;
                Some(measured)
            }
            Err(error) => {
                self.failure = Some(error);
                None
            }
        }
    }
}

struct CompletionSender {
    result: Option<Sender<Measurement>>,
    wake: Wake,
}

impl CompletionSender {
    fn send(&mut self, measured: Measurement) {
        if let Some(sender) = self.result.take() {
            let _obsolete = sender.send(measured);
        }
    }
}

impl Drop for CompletionSender {
    fn drop(&mut self) {
        // Close before waking so the first poll observes a stopped worker.
        drop(self.result.take());
        (self.wake)();
    }
}

fn launch(request: Request, spawn: impl FnOnce(Work) -> io::Result<()>) -> io::Result<Running> {
    let (sender, result) = mpsc::channel();
    let Request { measure, wake } = request;
    let mut completion = CompletionSender {
        result: Some(sender),
        wake,
    };
    spawn(Box::new(move || {
        completion.send(measure());
    }))?;
    Ok(Running {
        result,
        current: true,
    })
}

fn spawn(work: Work) -> io::Result<()> {
    thread::Builder::new()
        .name("channel-discovery".to_owned())
        .spawn(work)
        .map(|_worker| ())
}

#[cfg(test)]
#[path = "channel_refresh_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "channel_refresh_wake_order_tests.rs"]
mod wake_order_tests;

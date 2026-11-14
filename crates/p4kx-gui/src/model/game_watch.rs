// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};

use p4kx_extraction::game_is_running;

use super::Model;

/// How long a look at the process list stays valid.
pub const GAME_CHECK_INTERVAL: Duration = Duration::from_secs(20);

/// Whether the background process check warrants a conservative game warning.
#[derive(Debug)]
pub(super) struct GameWatch {
    enabled: bool,
    running: bool,
    next: Option<Instant>,
    pending: Option<Receiver<bool>>,
    probe: fn() -> bool,
}

impl Default for GameWatch {
    fn default() -> Self {
        Self {
            enabled: false,
            running: false,
            next: None,
            pending: None,
            probe: game_is_running,
        }
    }
}

impl GameWatch {
    fn poll(&mut self, wake: impl Fn() + Send + 'static) {
        if !self.enabled {
            return;
        }
        if let Some(pending) = &self.pending {
            match pending.try_recv() {
                Ok(running) => self.running = running,
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => self.running = true,
            }
            self.pending = None;
            self.next = Some(Instant::now() + GAME_CHECK_INTERVAL);
        }
        if self.next.is_some_and(|due| Instant::now() < due) {
            return;
        }
        let probe = self.probe;
        let (sender, result) = mpsc::channel();
        let spawned = thread::Builder::new()
            .name("game-check".to_owned())
            .spawn(move || {
                if sender.send(probe()).is_ok() {
                    wake();
                }
            });
        self.probe_started(spawned.map(|_handle| result));
    }
}

impl GameWatch {
    fn probe_started(&mut self, result: io::Result<Receiver<bool>>) {
        if let Ok(receiver) = result {
            self.pending = Some(receiver);
        } else {
            self.pending = None;
            self.running = true;
            self.next = Some(Instant::now() + GAME_CHECK_INTERVAL);
        }
    }
}

impl Model {
    /// Switches the look at the process list for a running game on or off; it starts off.
    pub const fn watch_game(&mut self, enabled: bool) {
        self.game.enabled = enabled;
    }

    /// Replaces the look at the process list with `probe`, so that a test does not depend on
    /// the machine.
    pub const fn probe_game_with(&mut self, probe: fn() -> bool) {
        self.game.probe = probe;
    }

    /// Looks at the process list when [`GAME_CHECK_INTERVAL`] passed since the last look, on a
    /// background thread that calls `wake` once it has the answer.
    pub fn check_game_when_due(&mut self, wake: impl Fn() + Send + 'static) {
        self.game.poll(wake);
    }

    /// Returns whether the enabled process check warrants a game warning.
    #[must_use]
    pub const fn game_running(&self) -> bool {
        self.game.enabled && self.game.running
    }

    /// Makes the next [`check_game_when_due`](Self::check_game_when_due) look again, as when an
    /// Archive is opened.
    pub(super) const fn recheck_game(&mut self) {
        self.game.next = None;
    }
}

#[cfg(test)]
#[path = "game_watch_tests.rs"]
mod tests;

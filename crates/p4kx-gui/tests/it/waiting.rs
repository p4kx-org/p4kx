// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The one way the headless window tests wait for asynchronous work: step the window until a
//! condition holds, within a time limit, and fail with the name of what was waited for.

use std::error::Error;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use p4kx_gui::Model;

/// How long a window may take to reach a state; generous because a software renderer shares the
/// machine with every other test.
pub(crate) const PATIENCE: Duration = Duration::from_secs(60);

const PAUSE: Duration = Duration::from_millis(2);

/// Steps `harness` until `done` holds for it, and fails with a message naming `what` once
/// [`PATIENCE`] passed.
pub(crate) fn wait_for<S>(
    harness: &mut Harness<'_, S>,
    what: &str,
    done: impl FnMut(&Harness<'_, S>) -> bool,
) -> Result<(), Box<dyn Error>> {
    wait_for_described(harness, what, done, |_| String::new())
}

/// Like [`wait_for`], and the message carries what `describe` says about the window when
/// [`PATIENCE`] passed.
pub(crate) fn wait_for_described<S>(
    harness: &mut Harness<'_, S>,
    what: &str,
    mut done: impl FnMut(&Harness<'_, S>) -> bool,
    describe: impl FnOnce(&Harness<'_, S>) -> String,
) -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    harness.step();
    while !done(harness) {
        if started.elapsed() > PATIENCE {
            return Err(timeout(what, &describe(harness)).into());
        }
        std::thread::sleep(PAUSE);
        harness.step();
    }
    Ok(())
}

fn timeout(what: &str, state: &str) -> String {
    if state.is_empty() {
        format!("timed out after {PATIENCE:?} waiting for {what}")
    } else {
        format!("timed out after {PATIENCE:?} waiting for {what}; state: {state}")
    }
}

/// Polls `subject` with `poll` until `done` holds for it, and fails with a message naming `what`
/// once [`PATIENCE`] passed. For tests that drive a model without a window.
pub(crate) fn wait_polling<T>(
    subject: &mut T,
    what: &str,
    poll: impl FnMut(&mut T),
    done: impl FnMut(&T) -> bool,
) -> Result<(), Box<dyn Error>> {
    wait_polling_described(subject, what, poll, done, |_| String::new())
}

/// Like [`wait_polling`], and the message carries what `describe` says about `subject` when
/// [`PATIENCE`] passed.
pub(crate) fn wait_polling_described<T>(
    subject: &mut T,
    what: &str,
    mut poll: impl FnMut(&mut T),
    mut done: impl FnMut(&T) -> bool,
    describe: impl FnOnce(&T) -> String,
) -> Result<(), Box<dyn Error>> {
    let started = Instant::now();
    while !done(subject) {
        if started.elapsed() > PATIENCE {
            return Err(timeout(what, &describe(subject)).into());
        }
        std::thread::sleep(PAUSE);
        poll(subject);
    }
    Ok(())
}

const REVEAL_STEPS: u32 = 8;

/// Scrolls the work surface of a window with the wheel until the node labelled `label` shows in
/// it, and panics when it does not show after [`REVEAL_STEPS`] scrolls; a low window keeps its tab
/// taller than the room it has.
#[track_caller]
pub(crate) fn reveal(harness: &mut Harness<'_, Model>, label: &str) {
    let work = p4kx_gui::ui::regions(&harness.ctx).work;
    assert!(work.is_some(), "the window has a work surface");
    let work = work.unwrap_or(egui::Rect::NOTHING);
    for _ in 0..REVEAL_STEPS {
        if work.contains(harness.get_by_label(label).rect().center()) {
            return;
        }
        harness.event(egui::Event::PointerMoved(work.center()));
        harness.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Line,
            delta: egui::vec2(0.0, -3.0),
            phase: egui::TouchPhase::Move,
            source: egui::MouseWheelSource::Wheel,
            modifiers: egui::Modifiers::NONE,
        });
        harness.run_steps(2);
    }
    let at = harness.get_by_label(label).rect();
    assert!(
        work.contains(at.center()),
        "`{label}` at {at:?} is not shown in the work surface {work:?}"
    );
}

/// Steps a window of the [`Model`] until `done` holds for the model.
pub(crate) fn wait_for_model(
    harness: &mut Harness<'_, Model>,
    what: &str,
    done: impl FnMut(&Model) -> bool,
) -> Result<(), Box<dyn Error>> {
    wait_for_model_described(harness, what, done, |_| String::new())
}

/// Like [`wait_for_model`], and the message carries what `describe` says about the model when
/// [`PATIENCE`] passed.
pub(crate) fn wait_for_model_described(
    harness: &mut Harness<'_, Model>,
    what: &str,
    mut done: impl FnMut(&Model) -> bool,
    describe: impl FnOnce(&Model) -> String,
) -> Result<(), Box<dyn Error>> {
    wait_for_described(
        harness,
        what,
        |harness| done(harness.state()),
        |harness| describe(harness.state()),
    )
}

const ATTEMPTS: u32 = 3;

/// Runs `act` on a window of the [`Model`] until `done` holds for the model, at most
/// [`ATTEMPTS`] times, and fails with the attempt count. A click that lands while the layout still
/// moves is lost, so it is repeated; a control that needs a second click never passes unnoticed.
pub(crate) fn act_until(
    harness: &mut Harness<'_, Model>,
    what: &str,
    mut act: impl FnMut(&mut Harness<'_, Model>),
    mut done: impl FnMut(&Model) -> bool,
) -> Result<(), Box<dyn Error>> {
    for _ in 0..ATTEMPTS {
        if done(harness.state()) {
            return Ok(());
        }
        act(harness);
        harness.run_steps(3);
    }
    if done(harness.state()) {
        return Ok(());
    }
    Err(format!("{what} did not happen after {ATTEMPTS} attempts").into())
}

/// Clicks the one node whose label contains `label`, once only one such node is shown: a menu
/// that is still opening shows its entry twice for a frame.
pub(crate) fn click_shown_once<S>(
    harness: &mut Harness<'_, S>,
    label: &str,
) -> Result<(), Box<dyn Error>> {
    wait_for_described(
        harness,
        &format!("`{label}` to be shown once"),
        |harness| harness.query_all_by_label_contains(label).count() == 1,
        |harness| {
            let matches = harness.query_all_by_label_contains(label).count();
            format!("{matches} matching nodes")
        },
    )?;
    harness.get_by_label_contains(label).click();
    Ok(())
}

/// Runs a window to rest.
pub(crate) trait RunQuiet {
    /// Steps the window until it asked for no more frames, and panics once [`PATIENCE`] passed.
    /// Unlike `Harness::run`, a window that keeps asking for frames while work finishes in the
    /// background does not fail on a step count.
    fn run_quiet(&mut self);
}

impl<S> RunQuiet for Harness<'_, S> {
    #[track_caller]
    fn run_quiet(&mut self) {
        let started = Instant::now();
        while self.run_ok().is_none() {
            assert!(
                started.elapsed() <= PATIENCE,
                "the window kept asking for frames for {PATIENCE:?}"
            );
            std::thread::sleep(PAUSE);
        }
    }
}

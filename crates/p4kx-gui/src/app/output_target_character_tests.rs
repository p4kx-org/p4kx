// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use p4kx_assembly::BodyVariant;
use p4kx_extraction::RuntimeEnvironment;
use p4kx_testkit::{ArchiveBuilder, armour_entries};
use tempfile::TempDir;

use super::App;
use super::output_target_route;
use crate::dialog::Dialog;
use crate::output_target::ParentTicket;
use crate::{Language, Model};

type TestResult = Result<(), Box<dyn std::error::Error>>;
const BACKGROUND_TIMEOUT: Duration = Duration::from_secs(20);

struct Fixture {
    app: App,
    events: Receiver<()>,
    directory: TempDir,
}

fn wake(sender: Sender<()>) -> impl Fn() + Send + Sync + 'static {
    move || {
        let _ended = sender.send(());
    }
}

impl Fixture {
    fn opened() -> Result<Self, Box<dyn std::error::Error>> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("fixture.p4k");
        armour_entries()
            .into_iter()
            .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
            .write_to(&path)?;
        let (sender, events) = mpsc::channel();
        let mut model = Model::new(Language::BritishEnglish);
        model.open(path, wake(sender.clone()));
        let app = App::new(model, None, None).with_runtime_environment(RuntimeEnvironment::Flatpak);
        let mut fixture = Self {
            app,
            events,
            directory,
        };
        fixture.wait(|model| model.browser().is_some())?;
        fixture.app.model.start_characters(wake(sender));
        fixture.wait(|model| {
            model.characters().export_available()
                && model.characters().sources().is_some()
                && model.entities().catalogue().is_some()
        })?;
        assert!(fixture.app.model.begin_character_export(), "setup A opens");
        Ok(fixture)
    }

    fn wait(&mut self, ready: impl Fn(&Model) -> bool) -> TestResult {
        let deadline = Instant::now() + BACKGROUND_TIMEOUT;
        loop {
            self.app.model.poll();
            self.app.model.entities_mut().poll();
            self.app.model.characters_mut().poll();
            if ready(&self.app.model) {
                return Ok(());
            }
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or("synthetic Character background work timed out")?;
            self.events.recv_timeout(remaining)?;
        }
    }

    fn select_a(&mut self) -> ParentTicket {
        let scope = output_target_route::scope(&self.app.model, Dialog::CharacterFile)
            .expect("built A has its actual source, setup and subject");
        self.app
            .output_target
            .begin(Dialog::CharacterFile, scope, "A.glb".to_owned(), None);
        self.app
            .output_target
            .pending()
            .expect("selection A open")
            .ticket()
    }

    fn rebuild_b(&mut self) -> TestResult {
        self.app
            .model
            .characters_mut()
            .set_body(BodyVariant::Female);
        self.wait(|model| {
            model.characters().export_available()
                && model.characters().body() == BodyVariant::Female
        })
    }
}

#[test]
fn a_character_rebuild_with_the_same_setup_discards_the_old_parent_reply() -> TestResult {
    let mut fixture = Fixture::opened()?;
    let setup_a = fixture
        .app
        .model
        .characters()
        .export_setup()
        .ok_or("setup A missing")?
        .output_owner();
    let ticket = fixture.select_a();
    fixture.rebuild_b()?;
    let setup_b = fixture
        .app
        .model
        .characters()
        .export_setup()
        .ok_or("same setup missing")?
        .output_owner();
    assert_eq!(setup_a, setup_b, "the actual setup lease stayed unchanged");
    fixture.app.retain_output_target();
    fixture
        .app
        .output_target
        .parent_reply(&ticket, Some(fixture.directory.path().to_path_buf()));
    assert!(
        fixture.app.output_target.pending().is_none(),
        "new Armour invalidates A's choice"
    );
    assert!(
        !fixture.app.output_target.confirm(),
        "late reply cannot confirm B under A's target"
    );
    assert!(
        fixture.app.take_dialog_result().is_none(),
        "no redirected output dispatch"
    );
    Ok(())
}

#[test]
fn a_confirmed_character_choice_is_discarded_if_the_same_setup_now_exports_b() -> TestResult {
    let mut fixture = Fixture::opened()?;
    let ticket = fixture.select_a();
    fixture
        .app
        .output_target
        .parent_reply(&ticket, Some(fixture.directory.path().to_path_buf()));
    assert!(
        fixture.app.output_target.confirm(),
        "A was ready before B finished"
    );
    fixture.rebuild_b()?;
    assert!(
        fixture.app.take_dialog_result().is_none(),
        "confirmed A cannot dispatch to B"
    );
    assert!(
        fixture
            .app
            .model
            .characters()
            .export_setup()
            .ok_or("setup remains")?
            .file()
            .is_none(),
        "B's existing setup target was not changed"
    );
    Ok(())
}

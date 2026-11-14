// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use p4kx_assembly::LoadoutPicks;
use p4kx_export::{ArchiveKeyState, ArchiveKeyUse};

use super::Cache;
use crate::archives::{Archives, Opened};
use crate::error::ToolError;
use crate::{key_fixture, key_report};

const DEADLINE: Duration = Duration::from_secs(5);
const NONE: LoadoutPicks = LoadoutPicks::new();

struct Race {
    cache: Arc<Cache<Vec<u8>>>,
    opened: Arc<Opened>,
    read: Receiver<()>,
    entered: Sender<()>,
    published: Receiver<Arc<Vec<u8>>>,
    worker: JoinHandle<Result<(), String>>,
}

impl Race {
    fn start(opened: Arc<Opened>) -> Self {
        let cache = Arc::new(Cache::new(100, Vec::len));
        let (read_tx, read) = mpsc::channel();
        let (entered, entered_rx) = mpsc::channel();
        let (published_tx, published) = mpsc::channel();
        let kept = Arc::clone(&cache);
        let source = Arc::clone(&opened);
        let worker = thread::spawn(move || {
            let winner = kept
                .get_or_resolve(source.archive(), "Entity", (false, &NONE), || {
                    let entry = source
                        .archive()
                        .entries()
                        .iter()
                        .find(|entry| entry.path() == key_fixture::SECRET)
                        .ok_or(ToolError::Stopped)?;
                    let error = source
                        .archive()
                        .open_entry(entry)
                        .err()
                        .ok_or(ToolError::Stopped)?;
                    assert!(
                        error.is_key_required(),
                        "winner actually requested an encrypted Entry without a key"
                    );
                    assert!(read_tx.send(()).is_ok(), "winner read signal is received");
                    assert!(
                        entered_rx.recv_timeout(DEADLINE).is_ok(),
                        "loser must enter resolution before winner publishes"
                    );
                    Ok(vec![1])
                })
                .map_err(|error| error.message())?;
            published_tx.send(winner).map_err(|error| error.to_string())
        });
        Self {
            cache,
            opened,
            read,
            entered,
            published,
            worker,
        }
    }
}

struct Loser {
    returned: Arc<Vec<u8>>,
    winner: Arc<Vec<u8>>,
    report: Option<ArchiveKeyUse>,
}

fn resolve_loser(race: &Race) -> Result<Loser, Box<dyn Error>> {
    race.read.recv_timeout(DEADLINE)?;
    key_report::clear();
    key_report::watch(&race.opened);
    let mut winner = None;
    let mut protocol_error: Option<Box<dyn Error>> = None;
    let resolved =
        race.cache
            .get_or_resolve(race.opened.archive(), "Entity", (false, &NONE), || {
                race.entered.send(()).map_err(|error| {
                    protocol_error = Some(error.into());
                    ToolError::Stopped
                })?;
                winner = Some(race.published.recv_timeout(DEADLINE).map_err(|error| {
                    protocol_error = Some(error.into());
                    ToolError::Stopped
                })?);
                Ok(vec![2])
            });
    if let Some(error) = protocol_error {
        return Err(error);
    }
    Ok(Loser {
        returned: resolved?,
        winner: winner.ok_or("winner publication was not observed")?,
        report: key_report::take(),
    })
}

#[test]
fn concurrent_resolution_reports_the_reused_winners_prior_key_demand() -> Result<(), Box<dyn Error>>
{
    let folder = tempfile::tempdir()?;
    key_fixture::archive(folder.path())?;
    let opened = Archives::default().get(&folder.path().join("Data.p4k"))?;
    let race = Race::start(opened);
    let loser = resolve_loser(&race);
    race.worker.join().map_err(|_| "winner thread panicked")??;
    let loser = loser?;
    assert!(
        Arc::ptr_eq(&loser.returned, &loser.winner),
        "the actual cache winner is returned"
    );
    assert_eq!(
        loser.returned.as_slice(),
        [1],
        "the losing resolution is discarded"
    );
    let report = loser
        .report
        .ok_or("reused winner's missing-key report was lost")?;
    assert_eq!(report.state(), ArchiveKeyState::Missing);
    assert_eq!(
        report.entries_unread(),
        1,
        "only the winner read this physical Entry"
    );
    Ok(())
}

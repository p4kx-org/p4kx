// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Archive key note of a tool result: which encrypted Entries the call could not read.

use std::cell::RefCell;
use std::sync::{Arc, Mutex, PoisonError};

use p4kx_export::{ArchiveKeyState, ArchiveKeyUse};
use p4kx_extraction::{DemandScope, KeyDemand, UnreadEntries};
use rmcp::model::MetaObject;
use serde_json::json;

use crate::archives::Opened;
use crate::keys;

/// An Archive a call read, with the scope that counts what the call asked of its key.
struct Watched {
    opened: Arc<Opened>,
    scope: DemandScope,
}

/// The unread Entries of the builds of kept results a call reused, per Archive.
#[derive(Debug, Default)]
struct Remembered(Vec<(KeyDemand, UnreadEntries)>);

impl Remembered {
    fn add(&mut self, demand: &KeyDemand, unread: &UnreadEntries) {
        match self
            .0
            .iter_mut()
            .find(|(known, _)| known.is_same_record(demand))
        {
            Some((_, known)) => known.union(unread),
            None => self.0.push((demand.clone(), unread.clone())),
        }
    }

    /// Returns `own` united with what was remembered for the Archive of `demand`.
    fn united(&self, demand: &KeyDemand, mut own: UnreadEntries) -> UnreadEntries {
        for (_, unread) in self
            .0
            .iter()
            .filter(|(known, _)| known.is_same_record(demand))
        {
            own.union(unread);
        }
        own
    }
}

/// The reused unread Entries of a call, shared with the threads that work for it.
#[derive(Debug, Clone, Default)]
pub(crate) struct Carry(Arc<Mutex<Remembered>>);

thread_local! {
    static WATCHED: RefCell<Vec<Watched>> = const { RefCell::new(Vec::new()) };
    static REMEMBERED: RefCell<Carry> = RefCell::new(Carry::default());
}

/// Forgets the Archives and the reused results of an earlier call on this thread.
pub(crate) fn clear() {
    WATCHED.with_borrow_mut(Vec::clear);
    REMEMBERED.with_borrow_mut(|carry| *carry = Carry::default());
}

/// Returns the reused unread Entries of the call running on this thread, for a thread that works
/// for it to [`adopt`].
pub(crate) fn carry() -> Carry {
    REMEMBERED.with_borrow(Carry::clone)
}

/// Runs `work` on this thread as part of the call `carry` came from, so that the kept results it
/// reuses are reported by that call.
pub(crate) fn adopt<T>(carry: Carry, work: impl FnOnce() -> T) -> T {
    let before = REMEMBERED.with_borrow_mut(|current| std::mem::replace(current, carry));
    let done = work();
    REMEMBERED.with_borrow_mut(|current| *current = before);
    done
}

/// Adds the Entries a reused kept result lacked, which the build left unread for the Archive of
/// `demand`, to the call running on this thread. An Entry the call asked for itself counts once.
pub(crate) fn reused(demand: &KeyDemand, unread: &UnreadEntries) {
    carry()
        .0
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .add(demand, unread);
}

/// Starts counting the encrypted Entries `opened` cannot read for the call running on this thread.
pub(crate) fn watch(opened: &Arc<Opened>) {
    WATCHED.with_borrow_mut(|watched| {
        if !watched.iter().any(|item| Arc::ptr_eq(&item.opened, opened)) {
            let scope = opened.archive().key_demand().scope();
            watched.push(Watched {
                opened: Arc::clone(opened),
                scope,
            });
        }
    });
}

/// Takes how the key served the call that ran on this thread, or `None` when it read no Archive
/// that has a key or lacked one for an Entry it asked for.
///
/// Per Archive the count is of the distinct Entries asked for while the call ran united with those
/// the builds of the kept results it reused left unread; several Archives add up. Another call
/// running at the same time on the same Archive counts into it too.
pub(crate) fn take() -> Option<ArchiveKeyUse> {
    let reused = REMEMBERED.with_borrow_mut(std::mem::take);
    let reused = reused.0.lock().unwrap_or_else(PoisonError::into_inner);
    WATCHED
        .with_borrow_mut(std::mem::take)
        .iter()
        .map(|item| {
            let archive = item.opened.archive();
            let unread = reused.united(&archive.key_demand(), item.scope.unread());
            ArchiveKeyUse::of_unread(archive, &unread)
        })
        .filter(|used| used.state() != ArchiveKeyState::Missing || used.entries_unread() > 0)
        .reduce(ArchiveKeyUse::merge)
}

/// Returns the text that tells the reader of the result what is missing and how to give the key,
/// or `None` when no encrypted Entry stayed unread.
pub(crate) fn text(used: ArchiveKeyUse) -> Option<String> {
    let unread = used.entries_unread();
    if unread == 0 {
        return None;
    }
    let hint = keys::state_hint(used.state())?;
    let subject = if unread == 1 {
        "1 encrypted Entry".to_owned()
    } else {
        format!("{unread} encrypted Entries")
    };
    Some(format!(
        "{subject} could not be read, so this result lacks what they hold. {hint}"
    ))
}

/// Returns the `_meta` members of the result: `archive_key` and `encrypted_entries_unread`.
pub(crate) fn meta(used: ArchiveKeyUse) -> MetaObject {
    let mut meta = MetaObject::new();
    meta.insert("archive_key".to_owned(), json!(used.state()));
    meta.insert(
        "encrypted_entries_unread".to_owned(),
        json!(used.entries_unread()),
    );
    meta
}

#[cfg(test)]
mod tests {
    use std::error::Error;

    use super::{clear, take, watch};
    use crate::archives::Archives;
    use crate::key_fixture::{OTHER, SECRET, read};
    use crate::key_stamp::KeyStamp;

    #[test]
    fn an_entry_the_call_and_a_reused_build_both_asked_for_counts_once()
    -> Result<(), Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("Data.p4k");
        crate::key_fixture::archive(dir.path())?;
        let opened = Archives::default().get(&path)?;
        let ((), stamp) = KeyStamp::measure(opened.archive(), || {
            read(opened.archive(), SECRET);
            read(opened.archive(), OTHER);
        });

        clear();
        watch(&opened);
        read(opened.archive(), SECRET);
        stamp.reused();
        stamp.reused();
        let used = take().ok_or("no report")?;

        assert_eq!(used.entries_unread(), 2, "each Entry once");
        Ok(())
    }

    #[test]
    fn the_unread_entries_of_another_archive_are_not_united() -> Result<(), Box<dyn Error>> {
        let first = tempfile::tempdir()?;
        let second = tempfile::tempdir()?;
        crate::key_fixture::archive(first.path())?;
        crate::key_fixture::archive(second.path())?;
        let archives = Archives::default();
        let one = archives.get(&first.path().join("Data.p4k"))?;
        let two = archives.get(&second.path().join("Data.p4k"))?;
        let ((), stamp) = KeyStamp::measure(one.archive(), || read(one.archive(), SECRET));

        clear();
        watch(&two);
        read(two.archive(), SECRET);
        stamp.reused();
        let used = take().ok_or("no report")?;

        assert_eq!(used.entries_unread(), 1, "only the Archive the call read");
        Ok(())
    }
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! How the Archive key served the build of a kept result, and when that result is built again.

use std::sync::{Arc, Mutex, PoisonError};

use p4kx_extraction::{Archive, KeyDemand, UnreadEntries};

use crate::key_report;

/// How the Archive key served the build of one kept result: which encrypted Entries the build left
/// unread and which key it ran with.
///
/// A result built with encrypted Entries unread lacks what they hold; a call that reuses it counts
/// them in its report as if it had built the result itself, and the result is built again once the
/// key is another one.
#[derive(Debug, Clone)]
pub(crate) struct KeyStamp {
    demand: KeyDemand,
    unread: Arc<UnreadEntries>,
    generation: u64,
}

impl KeyStamp {
    /// Runs `build` and returns what it built with the stamp of how `archive`'s key served it.
    pub(crate) fn measure<T>(archive: &Archive, build: impl FnOnce() -> T) -> (T, Self) {
        let generation = archive.key_generation();
        let demand = archive.key_demand();
        let scope = demand.scope();
        let value = build();
        let stamp = Self {
            demand,
            unread: Arc::new(scope.unread()),
            generation,
        };
        (value, stamp)
    }

    /// Returns whether the result may be reused with `archive` as it is now: it is complete, or
    /// the key is still the one it was built with.
    pub(crate) fn is_current(&self, archive: &Archive) -> bool {
        self.unread.unread() == 0 || self.generation == archive.key_generation()
    }

    /// Adds the Entries the build left unread to the report of the call running on this thread,
    /// united with the ones the call asked for itself, however often it reuses the result.
    pub(crate) fn reused(&self) {
        key_report::reused(&self.demand, &self.unread);
    }
}

/// One kept result that is built on first use and built again when its [`KeyStamp`] is no longer
/// current. Calls that need it while it is built wait for that build.
#[derive(Debug)]
pub(crate) struct Replaceable<T> {
    kept: Mutex<Option<(KeyStamp, Arc<T>)>>,
}

impl<T> Default for Replaceable<T> {
    fn default() -> Self {
        Self {
            kept: Mutex::new(None),
        }
    }
}

impl<T> Replaceable<T> {
    /// Returns the kept result, building it with `build` when there is none or it is no longer
    /// current for `archive`.
    pub(crate) fn get_or_build(&self, archive: &Archive, build: impl FnOnce() -> T) -> Arc<T> {
        let mut kept = self.kept.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((stamp, value)) = kept.as_ref()
            && stamp.is_current(archive)
        {
            stamp.reused();
            return Arc::clone(value);
        }
        let (fresh, stamp) = KeyStamp::measure(archive, build);
        let value = Arc::new(fresh);
        *kept = Some((stamp, Arc::clone(&value)));
        value
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::error::Error;
    use std::sync::Arc;

    use p4kx_extraction::Archive;
    use p4kx_testkit::{other_test_key, test_key};

    use super::{KeyStamp, Replaceable};
    use crate::key_fixture::{PLAIN, SECRET, archive, read};

    fn build(archive: &Archive, path: &str, builds: &Cell<usize>) -> usize {
        builds.set(builds.get() + 1);
        read(archive, path);
        builds.get()
    }

    #[test]
    fn an_incomplete_result_is_built_again_once_a_key_is_set() -> Result<(), Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let archive = archive(dir.path())?;
        let kept = Replaceable::default();
        let builds = Cell::new(0);

        let first = kept.get_or_build(&archive, || build(&archive, SECRET, &builds));
        let same = kept.get_or_build(&archive, || build(&archive, SECRET, &builds));
        archive.set_key(test_key());
        let later = kept.get_or_build(&archive, || build(&archive, SECRET, &builds));

        assert!(Arc::ptr_eq(&first, &same), "no new key, no new build");
        assert!(!Arc::ptr_eq(&first, &later), "a key now: built again");
        assert_eq!(builds.get(), 2);
        Ok(())
    }

    #[test]
    fn an_incomplete_result_is_built_again_for_another_key_but_not_for_the_same_one()
    -> Result<(), Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let archive = archive(dir.path())?;
        archive.set_key(other_test_key());
        let kept = Replaceable::default();
        let builds = Cell::new(0);

        kept.get_or_build(&archive, || build(&archive, SECRET, &builds));
        archive.set_key(other_test_key());
        kept.get_or_build(&archive, || build(&archive, SECRET, &builds));
        assert_eq!(builds.get(), 1, "the same wrong key again");

        archive.set_key(test_key());
        kept.get_or_build(&archive, || build(&archive, SECRET, &builds));
        assert_eq!(builds.get(), 2, "another key");
        Ok(())
    }

    #[test]
    fn a_complete_result_is_never_built_again_for_the_key() -> Result<(), Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let archive = archive(dir.path())?;
        let kept = Replaceable::default();
        let builds = Cell::new(0);

        let first = kept.get_or_build(&archive, || build(&archive, PLAIN, &builds));
        archive.set_key(test_key());
        archive.set_key(other_test_key());
        let later = kept.get_or_build(&archive, || build(&archive, PLAIN, &builds));

        assert!(Arc::ptr_eq(&first, &later));
        assert_eq!(builds.get(), 1);
        Ok(())
    }

    #[test]
    fn a_result_built_with_the_key_stays_while_the_key_does() -> Result<(), Box<dyn Error>> {
        let dir = tempfile::tempdir()?;
        let archive = archive(dir.path())?;
        archive.set_key(test_key());
        let ((), stamp) = KeyStamp::measure(&archive, || read(&archive, SECRET));

        assert!(stamp.is_current(&archive));
        archive.clear_key();
        assert!(stamp.is_current(&archive), "nothing was left unread");
        Ok(())
    }
}

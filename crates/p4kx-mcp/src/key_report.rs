// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Archive key note of a tool result: which encrypted Entries the call could not read.

use std::cell::RefCell;
use std::sync::Arc;

use p4kx_export::{ArchiveKeyState, ArchiveKeyUse};
use p4kx_extraction::DemandScope;
use rmcp::model::MetaObject;
use serde_json::json;

use crate::archives::Opened;
use crate::keys;

/// An Archive a call read, with the scope that counts what the call asked of its key.
struct Watched {
    opened: Arc<Opened>,
    scope: DemandScope,
}

thread_local! {
    static WATCHED: RefCell<Vec<Watched>> = const { RefCell::new(Vec::new()) };
}

/// Forgets the Archives of an earlier call on this thread.
pub(crate) fn clear() {
    WATCHED.with_borrow_mut(Vec::clear);
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
/// The count is of the Entries asked for while the call ran. Another call running at the same time
/// on the same Archive counts into it too, and a result kept from an earlier call is not counted
/// again.
pub(crate) fn take() -> Option<ArchiveKeyUse> {
    WATCHED
        .with_borrow_mut(std::mem::take)
        .iter()
        .map(|item| ArchiveKeyUse::scoped(item.opened.archive(), &item.scope))
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

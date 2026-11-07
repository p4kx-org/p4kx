// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

pub(crate) mod api;
#[cfg(any(windows, test))]
mod argument_folder;
#[cfg(any(windows, test))]
mod argument_identity;
#[cfg(any(windows, test))]
mod arguments;
#[cfg(any(windows, test))]
mod binding;
#[cfg(any(windows, test))]
mod bound_phase;
#[cfg(any(windows, test))]
mod caller;
#[cfg(any(windows, test))]
mod claim;
#[cfg(any(windows, test))]
mod compensation;
#[cfg(any(windows, test))]
mod declaration;
#[cfg(any(windows, test))]
mod driver;
#[cfg(any(windows, test))]
mod fault;
#[cfg(any(windows, test))]
pub(crate) mod invocation_nonce;
#[cfg(any(windows, test))]
mod journal_codec;
#[cfg(any(windows, test))]
mod journal_decode;
#[cfg(any(windows, test))]
mod journal_encode;
#[cfg(any(windows, test))]
mod lease;
pub mod notification;
#[cfg(any(windows, test))]
mod path_edit;
#[cfg(any(windows, test))]
mod raw_path;
#[cfg(any(windows, test))]
mod reservation;
#[cfg(any(windows, test))]
mod rollback_receipt;
#[cfg(any(windows, test))]
mod sequence;
#[cfg(any(windows, test))]
mod state;
#[cfg(any(windows, test))]
mod transaction;

#[cfg(windows)]
mod registry_adapter;
#[cfg(windows)]
mod registry_store;

#[cfg(test)]
mod store_tests;
#[cfg(test)]
mod tests;

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

pub(crate) mod api;
mod argument_folder;
mod argument_identity;
mod arguments;
mod caller;
mod claim;
mod compensation;
mod driver;
mod fault;
pub(crate) mod invocation_nonce;
mod journal_codec;
mod journal_decode;
mod journal_encode;
mod lease;
pub mod notification;
mod path_edit;
mod raw_path;
mod rollback_receipt;
mod sequence;
mod state;
mod transaction;

#[cfg(windows)]
mod registry_adapter;
#[cfg(windows)]
mod registry_store;

#[cfg(test)]
mod store_tests;
#[cfg(test)]
mod tests;

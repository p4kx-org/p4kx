// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Transactional per-user PATH ownership for the private Windows MSI helper.

#![forbid(unsafe_code)]

mod installer;

#[cfg(all(windows, feature = "native-fixtures"))]
pub use installer::native_fixture;

pub use installer::api::{Error, execute};
#[cfg(any(windows, test))]
pub use installer::invocation_nonce::{InvocationNonce, NonceError};
pub use installer::notification;

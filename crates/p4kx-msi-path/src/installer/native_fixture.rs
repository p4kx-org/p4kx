// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Opt-in native Registry evidence for disposable Windows runners.

#[path = "native_fixture/guard.rs"]
mod guard;
#[path = "native_fixture/image.rs"]
mod image;
#[path = "native_fixture/inspection.rs"]
mod inspection;
#[path = "native_fixture/registry.rs"]
mod registry;
#[path = "native_fixture/session.rs"]
mod session;

pub use guard::FixtureGuard;
pub use image::{FixtureImage, FixtureKeys, FixtureValue, KeyPresence};
pub use inspection::{CoreInspection, CorePhase, FixtureOwner, OwnerClaim};
pub use session::FixtureSession;

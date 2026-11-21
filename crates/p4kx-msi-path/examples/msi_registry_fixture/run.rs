// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::OsString;
use std::io;
use std::path::Path;

use p4kx_msi_path::native_fixture::{FixtureGuard, FixtureSession};

use crate::codec::{Envelope, Seed};
use crate::file_io;

const MAX_ARGUMENTS: usize = 6;
const MAX_ARGUMENT_BYTES: usize = 8192;

pub(super) fn execute(arguments: impl IntoIterator<Item = OsString>) -> io::Result<()> {
    let mut args = Vec::new();
    for argument in arguments {
        if args.len() == MAX_ARGUMENTS || argument.len() > MAX_ARGUMENT_BYTES {
            return Err(invalid());
        }
        args.push(argument);
    }
    match args.as_slice() {
        [command, root, out] if command == "capture" => {
            let session = open(root)?;
            let output = file_io::create(session.guard(), out)?;
            let image = session.capture()?;
            file_io::write(output, &Envelope::from_image(session.guard(), &image)?)
        }
        [command, root, expected, seed, out] if command == "seed-path" => {
            let session = open(root)?;
            let expected: Envelope = file_io::read(session.guard(), expected)?;
            let seed: Seed = file_io::read(session.guard(), seed)?;
            let expected = expected.into_image(session.guard())?;
            let seed = seed.into_path()?;
            let output = file_io::create(session.guard(), out)?;
            let after = session.seed_path(&expected, seed)?;
            file_io::write(output, &Envelope::from_image(session.guard(), &after)?)
        }
        [command, root, expected, original, out] if command == "restore" => {
            let session = open(root)?;
            let expected: Envelope = file_io::read(session.guard(), expected)?;
            let original: Envelope = file_io::read(session.guard(), original)?;
            let expected = expected.into_image(session.guard())?;
            let original = original.into_image(session.guard())?;
            let output = file_io::create(session.guard(), out)?;
            let after = session.restore(&expected, &original)?;
            file_io::write(output, &Envelope::from_image(session.guard(), &after)?)
        }
        [command, root, input, out] if command == "inspect" => {
            let session = open(root)?;
            let envelope: Envelope = file_io::read(session.guard(), input)?;
            let image = envelope.into_image(session.guard())?;
            let output = file_io::create(session.guard(), out)?;
            file_io::write(output, &Envelope::from_image(session.guard(), &image)?)
        }
        _ => Err(invalid()),
    }
}

fn open(root: &OsString) -> io::Result<FixtureSession> {
    Ok(FixtureSession::new(FixtureGuard::open(Path::new(root))?))
}

fn invalid() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        "expected capture, seed-path, restore or inspect with a fixed task root and evidence filenames",
    )
}

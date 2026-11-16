// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::io::Write as _;
use std::path::Path;

use crate::file_receipt::{FreshPublication, OrdinaryReplacement, try_create_fresh};
use crate::output_file::StagedOutput;
use crate::{CancelToken, OwnedFileToken, create_output_file};

use super::{Outcome, install_without_clobbering};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn locked_skip_cannot_overwrite_a_clean_writer_that_wins_after_invalidation() -> TestResult {
    let folder = tempfile::tempdir()?;
    let cancel = CancelToken::new();
    let output = folder.path().join("result.bin");

    let delayed = folder.path().join("delayed.part");
    let delayed_owner = write_stage(&delayed, b"delayed locked writer", &cancel)?;
    let delayed_stage = StagedOutput::new(&delayed, &output, &delayed_owner, None, Some(&cancel));
    let held = OrdinaryReplacement::for_entry(&output, None, Some(&cancel))?;
    held.invalidate()?;

    let first = folder.path().join("first.part");
    let first_owner = write_stage(&first, b"first clean writer", &cancel)?;
    let first_stage = StagedOutput::new(&first, &output, &first_owner, None, Some(&cancel));
    assert_eq!(
        try_create_fresh(&first_stage, Some(&cancel), || false)?,
        FreshPublication::Installed,
        "a clean writer can win while the other writer holds the parent lock"
    );
    first_owner.remove(&first, cancel.flag())?;

    delayed_stage.check()?;
    let outcome = install_without_clobbering(&delayed_stage, Outcome::Written)?;
    let finished = held.finish_extraction(&output, Ok(outcome))?;
    assert!(
        matches!(finished, Outcome::Kept),
        "the locked writer keeps the winner"
    );
    assert_eq!(
        fs::read(output)?,
        b"first clean writer",
        "the winning bytes"
    );
    delayed_owner.remove(&delayed, cancel.flag())?;
    assert_eq!(
        fs::read_dir(folder.path())?.count(),
        1,
        "both stages and owned lock are gone"
    );
    Ok(())
}

fn write_stage(path: &Path, bytes: &[u8], cancel: &CancelToken) -> TestResult<OwnedFileToken> {
    let (mut file, owner) = create_output_file(path, None, cancel.flag())?.into_parts();
    file.write_all(bytes)?;
    Ok(owner)
}

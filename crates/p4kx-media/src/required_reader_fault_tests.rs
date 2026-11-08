// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use p4kx_extraction::Exclusion;
use p4kx_testkit::TestEntry;

use crate::MediaError;
use crate::required_input_fault_support::{self as support, Inventory, TestResult};

#[test]
fn a_real_io_failure_and_actual_post_check_change_keep_both_typed_causes() -> TestResult {
    use std::error::Error as _;

    support::with_fixture(
        [TestEntry::stored("owner.dds", b"H")],
        Inventory::Intact,
        &Exclusion::default(),
        |context, _writer, checkpoint, folder| {
            let missing = folder.join("not-present");
            let original = File::open(&missing).err().ok_or("missing file opened")?;
            let original_kind = original.kind();
            let original_os = original.raw_os_error();
            let operation: Result<File, MediaError> =
                Err(crate::error::io(Path::new("owner.dds.1"), original));
            OpenOptions::new()
                .append(true)
                .open(context.directory.index.archive.path())?
                .write_all(b"actual synthetic source change")?;
            let failure = super::checked(operation, checkpoint)
                .err()
                .ok_or("two failures vanished")?;
            assert!(
                matches!(failure.source().and_then(|cause| cause.downcast_ref::<MediaError>()),
                    Some(MediaError::Io { source, .. }) if source.kind() == original_kind),
                "standard error traversal reaches the original operation before the checkpoint",
            );
            let MediaError::InputCheckFailed {
                operation,
                checkpoint,
            } = failure
            else {
                return Err("operation/checkpoint causes were not combined".into());
            };
            let MediaError::Io { path, source } = *operation else {
                return Err("original I/O category changed".into());
            };
            assert_eq!(
                path,
                Path::new("owner.dds.1"),
                "no private filesystem spelling"
            );
            assert_eq!(source.kind(), original_kind);
            assert_eq!(source.raw_os_error(), original_os);
            assert_eq!(source.kind(), io::ErrorKind::NotFound);
            assert!(
                matches!(*checkpoint, MediaError::Changed(_)),
                "actual Watch cause remains separate"
            );
            Ok(())
        },
    )
}

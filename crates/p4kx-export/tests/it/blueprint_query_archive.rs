// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry};

use crate::commodity_v3_fixture::{DcbOrigin, TestResult};

pub(super) fn open(
    database: &DataForgeBuilder,
    origin: DcbOrigin,
    with_key: bool,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("source.p4k");
    let dcb = TestEntry::stored("Data/Game2.dcb", database.build());
    let dcb = match origin {
        DcbOrigin::Plain => dcb,
        DcbOrigin::Encrypted => dcb.encrypted(),
        DcbOrigin::Unknown => dcb.encrypted().with_encryption_field(7),
    };
    ArchiveBuilder::new()
        .entry(dcb)
        .entry(
            TestEntry::stored(
                Language::English.global_ini_path(),
                b"sdk_blueprint_name=Protected recipe\n",
            )
            .encrypted(),
        )
        .entry(
            TestEntry::stored(
                "Data/ObjectContainers/PU/system/synthetic/syntheticsystem.socpak",
                b"locked-world",
            )
            .encrypted(),
        )
        .entry(TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ))
        .write_to(&path)?;
    let key = with_key.then(p4kx_testkit::test_key);
    Ok((folder, Archive::open_with_key(path, key)?))
}

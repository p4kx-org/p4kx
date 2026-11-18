// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry};

use crate::commodity_v3_fixture::{DcbOrigin, TestResult};

pub(crate) fn open(
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
            TestEntry::stored(Language::English.global_ini_path(), english().as_bytes())
                .encrypted(),
        )
        .entry(TestEntry::stored(
            Language::German.global_ini_path(),
            german().as_bytes(),
        ))
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

const fn english() -> &'static str {
    "fixture_resource_name=Resource English\nfixture_resource_description=Resource description\nsdk_mineable_deposit=Protected deposit\nfixture_group_name=Group\nfixture_group_description=Group description\n"
}

const fn german() -> &'static str {
    "fixture_resource_name=Ressource Deutsch\nfixture_resource_description=Ressourcenbeschreibung\nsdk_mineable_deposit=Geschuetzter Lagerstaettentext\nfixture_group_name=Gruppe\nfixture_group_description=Gruppenbeschreibung\n"
}

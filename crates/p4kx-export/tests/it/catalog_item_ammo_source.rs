// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, DataForgeEntry, DataForgeError, Record, RecordTree, Value, ValueError,
};

use super::{TestResult, item_stats_fixture};

#[derive(Clone, Copy)]
pub(super) enum AmmoSource {
    CyclicMembers,
    InvalidLayout,
}

pub(super) fn archive(
    source: AmmoSource,
) -> Result<(tempfile::TempDir, Archive), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = match source {
        AmmoSource::CyclicMembers => item_stats_fixture::write_archive(directory.path())?,
        AmmoSource::InvalidLayout => {
            item_stats_fixture::write_archive_with_invalid_ammo(directory.path())?
        }
    };
    let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
    original(&archive, source)?;
    Ok((directory, archive))
}

fn original(archive: &Archive, source: AmmoSource) -> TestResult {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let ammo = forge
        .record(1)
        .ok_or("the original exceptional ammo record")?;
    assert_eq!(
        ammo.type_name()?,
        "AmmoParams",
        "the actual selected ammo source type"
    );
    assert_eq!(
        ammo.record_path()?,
        "libs/foundry/records/ammoparams/vehicle/ammo_unbuildable",
        "the same original exceptional ammo source"
    );
    for index in [0, 2] {
        let record = forge
            .record(index)
            .ok_or("the unaffected original source record")?;
        assert_eq!(
            record.tree()?.error_count(),
            0,
            "the normal ammo and the weapon Reference tree stay readable"
        );
    }
    match source {
        AmmoSource::CyclicMembers => cyclic(&ammo.tree()?),
        AmmoSource::InvalidLayout => invalid(&ammo),
    }
}

fn cyclic(tree: &RecordTree<'_>) -> TestResult {
    assert_eq!(
        tree.error_count(),
        2,
        "the two actual recursive declarations remain errors"
    );
    for name in ["speed", "lifetime"] {
        assert!(
            matches!(tree.field(name), Some(Value::Single(value)) if value.to_bits() == 0),
            "the unrelated original Single {name} is exactly positive zero"
        );
    }
    assert!(
        matches!(tree.field("projectileParams"), Some(Value::Null)),
        "the original projectile is absent"
    );
    let Some(Value::Struct(trap)) = tree.field("trap") else {
        return Err("the actual original trap instance".into());
    };
    assert_eq!(
        trap.type_name(),
        "Loop",
        "the actual original recursive instance"
    );
    let names: Vec<_> = trap
        .fields()
        .iter()
        .map(p4kx_extraction::Field::name)
        .collect();
    assert_eq!(
        names,
        ["left", "right"],
        "the original cyclic declaration order"
    );
    for field in trap.fields() {
        assert!(
            matches!(
                field.value(),
                Value::Error(ValueError::InlineClassCycle { .. })
            ),
            "each original recursive Class member keeps its specific error"
        );
    }
    Ok(())
}

fn invalid(ammo: &Record<'_>) -> TestResult {
    let error = ammo
        .tree()
        .err()
        .ok_or("the original ammo must genuinely fail construction")?;
    assert!(
        matches!(
            error,
            DataForgeError::StructLayout {
                declared: 3,
                fields: 2,
                ..
            }
        ),
        "a genuine layout defect is distinct from an implementation quota: {error}"
    );
    Ok(())
}

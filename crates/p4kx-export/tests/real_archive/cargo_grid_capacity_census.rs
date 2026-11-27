// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    Archive, ArchiveWatch, DataForge, DataForgeEntry, Filter, RecordTree, Struct, TreeBuilder,
    Value,
};
use p4kx_testkit::{real_archive_key, real_archive_path};

use super::original::{GRID, Grid};
use super::{TestResult, print_line};

pub(super) fn run() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let filter = std::env::var("P4KX_CATALOG_CARGO_DISCOVERY_FILTER")
        .ok()
        .filter(|pattern| !pattern.is_empty())
        .map(|pattern| Filter::parse(&pattern))
        .transpose()?;
    let named =
        std::env::var_os("P4KX_CATALOG_CARGO_REPORT_PATHS").is_some_and(|value| value == "1");
    let (declarations, instances, derived_instances) = cargo_grid_instances(&forge)?;
    print_line(&format!(
        "cargo Grid source census: exact declarations={declarations} physical instances={instances} derived physical instances={derived_instances}"
    ));
    if instances == 0 {
        check_archive(&watch)?;
        return Ok(());
    }
    let probe = forge
        .pointer_probe("Components", GRID)
        .ok_or("original Cargo Grid instances must have an exact pointer probe")?;
    let mut totals = Census::default();
    let mut builder = TreeBuilder::new();
    for record in forge.records() {
        if record.type_name()? != "EntityClassDefinition" {
            continue;
        }
        let path = record.record_path()?;
        if filter.as_ref().is_some_and(|filter| !filter.matches(path)) {
            continue;
        }
        match builder.holds(&probe, &record) {
            Ok(true) => {
                totals.owners += 1;
                if let Ok(tree) = builder.build(&record) {
                    totals.record(&forge, tree, named);
                } else {
                    totals.unreadable_owners += 1;
                }
            }
            Ok(false) => {}
            Err(_) => totals.unreadable_owners += 1,
        }
    }
    check_archive(&watch)?;
    totals.report(named);
    Ok(())
}

fn cargo_grid_instances(forge: &DataForge<'_>) -> TestResult<(u64, u64, u64)> {
    let limit = forge.structs().len();
    let mut declarations = 0;
    let mut instances = 0;
    let mut derived_instances = 0;
    for structure in forge.structs() {
        let count = u64::from(structure.instance_count());
        if structure.name()? == GRID {
            declarations += 1;
            instances += count;
        } else if count != 0 && derives_grid(structure, limit)? {
            derived_instances += count;
        }
    }
    Ok((declarations, instances, derived_instances))
}

fn derives_grid(mut structure: Struct<'_>, declarations: usize) -> TestResult<bool> {
    for _ in 0..declarations {
        let Some(parent) = structure.parent() else {
            return Ok(false);
        };
        if parent.name()? == GRID {
            return Ok(true);
        }
        structure = parent;
    }
    Err("the original structure ancestry contains a cycle".into())
}

fn check_archive(watch: &ArchiveWatch) -> TestResult {
    watch
        .check()
        .map_err(|_| "Archive changed during the cargo Grid source census".into())
}

#[derive(Default)]
struct Census {
    owners: usize,
    unreadable_owners: usize,
    mining: usize,
    control: usize,
    absent: usize,
    null: usize,
    unreadable: usize,
    invisible: usize,
    mining_example: Option<Example>,
    control_example: Option<Example>,
    invisible_control_example: Option<Example>,
}

struct Example {
    item: String,
    container: String,
}

impl Census {
    fn record<'source>(
        &mut self,
        forge: &'source DataForge<'_>,
        tree: RecordTree<'source>,
        named: bool,
    ) {
        let Some(Value::Array(components)) = tree.field("Components") else {
            self.unreadable_owners += 1;
            return;
        };
        let mut example = None;
        for component in components {
            let Value::Struct(grid) = component else {
                continue;
            };
            if grid.type_name() != GRID {
                continue;
            }
            let invisible = matches!(grid.field("invisible"), Some(Value::Bool(true)));
            self.invisible += usize::from(invisible);
            match grid.field("miningOnly") {
                Some(Value::Bool(true)) => {
                    self.mining += 1;
                    example = Some((true, invisible));
                }
                Some(Value::Bool(false)) => {
                    self.control += 1;
                    example = Some((false, invisible));
                }
                Some(Value::Null) => self.null += 1,
                None => self.absent += 1,
                _ => self.unreadable += 1,
            }
        }
        if named && let Some((mining, invisible)) = example {
            self.keep_example(forge, tree, mining, invisible);
        }
    }

    fn keep_example<'source>(
        &mut self,
        forge: &'source DataForge<'_>,
        tree: RecordTree<'source>,
        mining: bool,
        invisible: bool,
    ) {
        let target = if mining {
            &mut self.mining_example
        } else if invisible {
            &mut self.invisible_control_example
        } else {
            &mut self.control_example
        };
        if target.is_none()
            && let Ok(grid) = Grid::from_item(forge, tree, mining)
        {
            *target = Some(Example {
                item: grid.item.record_path().to_owned(),
                container: grid.container.record_path().to_owned(),
            });
        }
    }

    fn report(&self, named: bool) {
        print_line(&format!(
            "cargo Grid source census: owners={} unreadableOwners={} miningOnlyTrue={} miningOnlyFalse={} miningOnlyAbsent={} miningOnlyNull={} miningOnlyUnreadable={} invisibleTrue={}",
            self.owners,
            self.unreadable_owners,
            self.mining,
            self.control,
            self.absent,
            self.null,
            self.unreadable,
            self.invisible,
        ));
        if named {
            for (label, example) in [
                ("mining", &self.mining_example),
                ("control", &self.control_example),
                ("invisible control", &self.invisible_control_example),
            ] {
                if let Some(example) = example {
                    print_line(&format!(
                        "{label} Item: {} | Container: {}",
                        example.item, example.container
                    ));
                }
            }
        }
    }
}

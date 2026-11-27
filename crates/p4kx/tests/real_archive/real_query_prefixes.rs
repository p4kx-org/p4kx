// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Times a Query with several selected paths under one component over the ships of a real
//! Archive, with and without the walk of the shared steps, and checks that both give the same
//! rows.

#![expect(clippy::print_stdout, reason = "the measurements are the output")]

use std::error::Error;
use std::fmt::Write as _;
use std::time::{Duration, Instant};

use p4kx_extraction::query::{CompiledQuery, Query};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::real_archive_path;

type TestResult = Result<(), Box<dyn Error>>;

const SHIPS: &str = "from EntityClassDefinition at \"libs/foundry/records/entities/spaceships/*\" \
select Components[SAttachableComponentParams].AttachDef.Size, \
Components[SAttachableComponentParams].AttachDef.Type, \
Components[SAttachableComponentParams].AttachDef.SubType, \
Components[SAttachableComponentParams].AttachDef.Grade, \
Components[SAttachableComponentParams].AttachDef.Manufacturer->Code, \
Components[SAttachableComponentParams].AttachDef.Manufacturer->Localization.Name, \
Components[VehicleComponentParams].vehicleName, \
Components[VehicleComponentParams].crewSize";

const EVERYTHING: &str = "from EntityClassDefinition \
select Components[SAttachableComponentParams].AttachDef.Size, \
Components[SAttachableComponentParams].AttachDef.Type, \
Components[SAttachableComponentParams].AttachDef.SubType, \
Components[SAttachableComponentParams].AttachDef.Grade, \
Components[SAttachableComponentParams].AttachDef.Manufacturer->Code, \
Components[SAttachableComponentParams].AttachDef.Manufacturer->Localization.Name";

struct Timing {
    records: usize,
    text: String,
    select: Duration,
    rows_median: Duration,
    rows_min: Duration,
}

fn render(compiled: &CompiledQuery<'_>, runs: usize) -> Result<Timing, Box<dyn Error>> {
    let started = Instant::now();
    let run = compiled.run()?;
    let select = started.elapsed();
    let mut times = Vec::with_capacity(runs);
    let mut rows = Vec::new();
    for _ in 0..runs {
        let started = Instant::now();
        rows = run.rows(0..run.len());
        times.push(started.elapsed());
    }
    times.sort();
    let mut text = String::new();
    for row in &rows {
        let _ = write!(
            text,
            "{}|{}",
            row.record_path(),
            row.unresolved_references()
        );
        match row.cells() {
            Ok(cells) => {
                for cell in cells {
                    match cell {
                        Some(value) => {
                            let _ = write!(text, "|{value}");
                        }
                        None => text.push('|'),
                    }
                }
            }
            Err(error) => {
                let _ = write!(text, "|{error}");
            }
        }
        text.push('\n');
    }
    Ok(Timing {
        records: rows.len(),
        text,
        select,
        rows_median: times.get(runs / 2).copied().unwrap_or_default(),
        rows_min: times.first().copied().unwrap_or_default(),
    })
}

fn measure(name: &str, query: &str, runs: usize, archive: &Archive) -> TestResult {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let shared = Query::parse(query)?.compile(&forge)?;
    let alone = Query::parse(query)?
        .compile(&forge)?
        .without_shared_prefixes();
    for round in 0..5 {
        let with = render(&shared, runs)?;
        let without = render(&alone, runs)?;
        println!(
            "{name} round {round}: {} records, {} bytes of cells; rows median/min of {runs}: \
             shared {:?}/{:?}, alone {:?}/{:?}; selection {:?}/{:?}",
            with.records,
            with.text.len(),
            with.rows_median,
            with.rows_min,
            without.rows_median,
            without.rows_min,
            with.select,
            without.select,
        );
        assert_eq!(with.text, without.text, "{name}: the rows are the same");
        assert!(with.records > 0, "{name}: the Query selects Records");
    }
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn query_time_of_several_paths_under_one_component() -> TestResult {
    let archive = Archive::open_with_key(
        real_archive_path()?,
        Some(p4kx_testkit::real_archive_key()?),
    )?;
    measure("ships", SHIPS, 300, &archive)?;
    measure("all entities", EVERYTHING, 40, &archive)
}

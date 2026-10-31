// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The MFD of the Avenger Titan once `CanvasReferenceRecord` is evaluated.

use std::error::Error;

use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_screens::{ScreenComposer, Variable};
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult = Result<(), Box<dyn Error>>;

const MFD: &str = "libs/foundry/records/ui/buildingblocks/ships/displays/mfdscreens/mc_mfdcomponents/screens/general/m_mfd_screen";
const POWER_PAGE: &str = "libs/foundry/records/ui/buildingblocks/ships/displays/mfdscreens/mc_mfdcomponents/screens/power/mc_s_power_master";
const BRAND: &str = "aegs";

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn the_canvases_an_mfd_swaps_in_are_entered_and_a_namespace_reaches_one_of_them() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let power = [
        Variable::new("powerstate", "1")?,
        Variable::new("landscapecanvasguid", POWER_PAGE)?,
    ];
    let on = ScreenComposer::new(&forge)
        .with_variables(&power)
        .compose(MFD, Some(BRAND))?;
    assert!(
        on.report().canvases >= 8,
        "{} canvases",
        on.report().canvases
    );
    assert!(
        on.report().widgets >= 100,
        "{} widgets",
        on.report().widgets
    );
    assert_eq!(on.report().missing_canvases.first(), None);

    let scoped = [Variable::new("m_mfd_screen::powerstate", "1")?];
    let only = ScreenComposer::new(&forge)
        .with_variables(&scoped)
        .compose(MFD, Some(BRAND))?;
    assert_eq!(only.report().unknown_variables().count(), 0);
    let elsewhere = [Variable::new("no_such_canvas::powerstate", "1")?];
    let none = ScreenComposer::new(&forge)
        .with_variables(&elsewhere)
        .compose(MFD, Some(BRAND))?;
    assert_eq!(none.report().widgets, 1, "the namespace names no canvas");
    assert_eq!(none.report().unknown_variables().count(), 1);
    Ok(())
}

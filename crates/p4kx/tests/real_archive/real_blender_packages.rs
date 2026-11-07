// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Capability-bound CLI packages preserve schema, sharing, texture and size checks.

use std::ffi::OsStr;

use serde_json::Value;

use crate::media_fixtures::Family;
use crate::package_fault::{self, Check};
use crate::package_fixtures::PackageScope;
use crate::package_support::{self, Built, Measured, TestResult};
use crate::{package_character, package_fixtures};

const SHIPS: [Family; 4] = [
    Family::Gladius,
    Family::Avenger,
    Family::Polaris,
    Family::Constellation,
];
const INTERIOR_SHIPS: [Family; 2] = [Family::Constellation, Family::Cutlass];
const MAX_SHIP_BYTES: u64 = 2 << 30;
const MAX_COARSE_SHIP_BYTES: u64 = 128 << 20;
const MAX_INTERIOR_BYTES: u64 = 3 << 29;
const MAX_MESH_RATIO: f64 = 0.85;
const MIN_SHARING: f64 = 0.3;
const MIN_SHIP_TEXTURES: usize = 100;

fn shared(built: &Built, measured: &Measured) -> TestResult {
    package_fault::require(measured.textures() >= MIN_SHIP_TEXTURES, Check::Textures)?;
    package_fault::require(
        built.report.get("drawn_nodes").and_then(Value::as_u64) == Some(measured.instances),
        Check::MeshInstances,
    )?;
    package_fault::require(measured.sharing() >= MIN_SHARING, Check::MeshSharing)?;
    package_fault::require(measured.mesh_ratio() <= MAX_MESH_RATIO, Check::MeshRatio)
}

fn ship(built: &Built, measured: &Measured) -> TestResult {
    shared(built, measured)?;
    package_fault::require(measured.meshes > 1, Check::MinimumMeshes)?;
    package_fault::require(
        measured.instances > measured.meshes as u64,
        Check::MeshSharing,
    )?;
    package_fault::require(measured.bytes <= MAX_SHIP_BYTES, Check::PackageSize)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn documented_ships_are_packages_within_the_documented_size() {
    package_support::run("ships", |scratch| {
        package_fixtures::matrix(scratch, &SHIPS, PackageScope::Hull, &[], ship)
    });
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_coarser_texture_mip_makes_a_ship_package_much_smaller() {
    package_support::run("coarse", |scratch| {
        package_fixtures::matrix(
            scratch,
            &[Family::Gladius],
            PackageScope::Hull,
            &[OsStr::new("--texture-mip"), OsStr::new("2")],
            |built, measured| {
                shared(built, measured)?;
                package_fault::require(measured.bytes <= MAX_COARSE_SHIP_BYTES, Check::PackageSize)
            },
        )
    });
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn ships_with_an_interior_are_packages_with_containers_objects_and_lights() {
    package_support::run("interiors", |scratch| {
        package_fixtures::matrix(
            scratch,
            &INTERIOR_SHIPS,
            PackageScope::Interior,
            &[OsStr::new("--interior")],
            interior,
        )
    });
}

fn interior(built: &Built, measured: &Measured) -> TestResult {
    shared(built, measured)?;
    let containers = package_support::nodes_of(&built.manifest, "container")?;
    let objects = package_support::nodes_of(&built.manifest, "object")?;
    let lights = package_support::nodes_of(&built.manifest, "light")?;
    package_fault::require(containers > 1 && objects > 0, Check::Interior)?;
    for (key, count) in [
        ("containers", containers),
        ("objects", objects),
        ("lights", lights),
    ] {
        package_fault::require(
            built.report.get(key).and_then(Value::as_u64) == Some(count as u64),
            Check::ReportCount,
        )?;
    }
    package_fault::require(measured.bytes <= MAX_INTERIOR_BYTES, Check::PackageSize)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn a_character_with_armour_is_one_skinned_package() {
    package_support::run("character", package_character::verify);
}

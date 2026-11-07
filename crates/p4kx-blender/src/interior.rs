// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;
use std::sync::atomic::AtomicBool;

use p4kx_assembly::{Interior, ModelProvider};
use p4kx_gltf::{GlbOptions, TextureLoader};

use crate::items::interior_items;
use crate::manifest::EntityRef;
use crate::write::{Scene, write_scene};
use crate::{PackageInfo, PackageResult, PackageSummary};

/// An [`Interior`] ready to be written as a package on its own, without an Entity around it.
#[derive(Debug, Clone, Copy)]
pub struct InteriorScene<'a> {
    name: &'a str,
    interior: &'a Interior,
}

impl<'a> InteriorScene<'a> {
    /// Names the Interior `name`, which the manifest records as `source.entity.class_name`, and
    /// puts `interior` in the scene.
    #[must_use]
    pub const fn new(name: &'a str, interior: &'a Interior) -> Self {
        Self { name, interior }
    }
}

/// Writes an Interior on its own as a package into the folder `out`.
///
/// The package is the folder of [`write_package`](crate::write_package): the Interior's root
/// container is the root node, below it the `.soc` files, models, props, lights and nested
/// containers as nodes of the kinds `container`, `object` and `light`, the same as below the
/// `Interior` node of an Entity's package. The manifest has no node of the kind `entity`;
/// `source.entity` carries the name of `scene` as `class_name` and an empty `guid`, because an
/// Interior is not an Entity of the Archive. The level of detail, the mip level and the vertex
/// colours are those of `options`; `options.materials` is ignored. `models` supplies the layer
/// materials the models name.
///
/// `out` is treated as in [`write_package`](crate::write_package): checked before anything is
/// done, written into a temporary folder next to it which replaces it once complete, polled for
/// `cancelled` between the files.
///
/// # Errors
///
/// Returns [`PackageError::Refused`](crate::PackageError::Refused) when `out` lies inside a Star
/// Citizen installation, [`PackageError::NotAPackage`](crate::PackageError::NotAPackage) when
/// `out` is something else than an empty folder or a package,
/// [`PackageError::Mesh`](crate::PackageError::Mesh) when a mesh does not fit in a GLB file,
/// [`PackageError::Write`](crate::PackageError::Write) when a file or folder cannot be written
/// and [`PackageError::Cancelled`](crate::PackageError::Cancelled) when `cancelled` returned
/// true.
pub fn write_interior_package(
    scene: &InteriorScene<'_>,
    models: &dyn ModelProvider,
    textures: &dyn TextureLoader,
    options: &GlbOptions,
    info: &PackageInfo,
    out: &Path,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> PackageResult<PackageSummary> {
    let observed = AtomicBool::new(false);
    write_interior_package_with_context(
        scene,
        models,
        textures,
        options,
        info,
        out,
        &crate::PackageWriteContext::standalone(cancelled, &observed),
    )
}

/// Writes an Interior package with source protection at every output boundary.
///
/// # Errors
/// Returns protection, cancellation, package validation, mesh or file failures.
pub fn write_interior_package_with_context(
    scene: &InteriorScene<'_>,
    models: &dyn ModelProvider,
    textures: &dyn TextureLoader,
    options: &GlbOptions,
    info: &PackageInfo,
    out: &Path,
    context: &crate::PackageWriteContext<'_>,
) -> PackageResult<PackageSummary> {
    let scene = Scene {
        entity: EntityRef {
            class_name: scene.name.to_owned(),
            guid: String::new(),
        },
        items: interior_items(scene.interior),
        issues: scene.interior.issues(),
    };
    write_scene(&scene, models, textures, options, info, out, context)
}

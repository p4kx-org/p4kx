// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_foundation::read_text_number;

use crate::CdfAttachmentSite;
use crate::cdf::slashes;
use crate::markup::Element;
use crate::read::narrow;

/// A bone attachment (`CA_BONE`) of a character definition: a rigid model bound to a bone of the
/// skeleton and offset from it.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BoneAttachment {
    /// The attachment's name (`AName`), else the file stem of its model.
    pub(crate) name: String,
    /// The model file, with `/` as separator.
    pub(crate) binding: String,
    /// The bone of the skeleton the model is bound to.
    pub(crate) bone: String,
    /// The material library that replaces the one the model names.
    pub(crate) material: Option<String>,
    /// The offset from the bone in metres (`RelPosition`).
    pub(crate) position: [f32; 3],
    /// The rotation from the bone as a quaternion `[w, x, y, z]` (`RelRotation`).
    pub(crate) rotation: [f32; 4],
    pub(crate) sources: Option<Arc<[CdfAttachmentSite]>>,
}

impl BoneAttachment {
    /// Reads the attachment `element` bound to `binding`, or returns `None` when it is no bone
    /// attachment or its bone, offset or rotation cannot be read.
    pub(crate) fn read(element: &Element, binding: &str) -> Option<Self> {
        let is_bone = element
            .attribute("Type")
            .is_some_and(|kind| kind.eq_ignore_ascii_case("CA_BONE"));
        let bone = element
            .attribute("BoneName")
            .filter(|bone| !bone.is_empty());
        let (true, Some(bone)) = (is_bone, bone) else {
            return None;
        };
        let position = match element.attribute("RelPosition") {
            Some(text) => numbers::<3>(text)?,
            None => [0.0; 3],
        };
        let rotation = match element.attribute("RelRotation") {
            Some(text) => numbers::<4>(text).filter(|q| q.iter().any(|part| *part != 0.0))?,
            None => [1.0, 0.0, 0.0, 0.0],
        };
        let binding = slashes(binding);
        let name = element
            .attribute("AName")
            .filter(|name| !name.is_empty())
            .map_or_else(|| stem(&binding), str::to_owned);
        Some(Self {
            name,
            bone: bone.to_owned(),
            material: element.attribute("Material").map(slashes),
            binding,
            position,
            rotation,
            sources: None,
        })
    }
}

/// Reads `N` comma-separated numbers, or `None` for another count, an unreadable number or one
/// that is not finite as an `f32`.
fn numbers<const N: usize>(text: &str) -> Option<[f32; N]> {
    let mut out = [0.0_f32; N];
    let mut parts = text.split(',');
    for slot in &mut out {
        let number = read_text_number(parts.next()?).filter(|number| !number.is_partial())?;
        *slot = Some(narrow(number.value())).filter(|value| value.is_finite())?;
    }
    parts.next().is_none().then_some(out)
}

/// Returns the file name of `path` without its extension.
fn stem(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.rsplit_once('.')
        .map_or(name, |(stem, _)| stem)
        .to_owned()
}

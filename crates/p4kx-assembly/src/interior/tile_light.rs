// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::PathIndex;
use p4kx_extraction::cryxml::SourceElementPosition;
use p4kx_model::{
    Model, SocContainerDescriptor, SocGroupPlacement, SocLight, SocSourceValue, Transform,
    tile_attachment_point_suffix,
};

use crate::placement;
use crate::{
    SocTileLightAnchor, SocTileLightAnchorError, SocTilePortIndex, SocTilePortTarget, ValueOrigin,
    VehicleImplementation,
};

#[derive(Clone, Copy)]
pub(crate) struct TileLightOwner<'a> {
    pub(crate) frame: Option<Transform>,
    pub(crate) model: Option<&'a Model>,
    pub(crate) vehicle: Option<&'a VehicleImplementation>,
}

pub(super) struct TileDescriptor {
    pub(super) name: String,
    pub(super) bytes: Vec<u8>,
}

/// Authored and resolved placement of one baked light in its direct vehicle container.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct InteriorLightPlacement {
    /// Direct owner frame used for the correction when it was proven.
    pub owner_from_container: Option<Transform>,
    /// Exact source port element in its owning descriptor.
    pub port: Option<InteriorTilePort>,
    /// Effective group frame or the reason authored placement was retained.
    pub anchor: Option<Result<SocTileLightAnchor, SocTileLightAnchorError>>,
    /// Exact helper result when the group requested one or a point could be joined.
    pub helper: InteriorLightHelper,
}

/// The physical tile descriptor and the selected `ItemPort` within it.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct InteriorTilePort {
    /// Package-relative path of the original descriptor inside its SOC container.
    pub descriptor: Arc<str>,
    /// Position of the original item-port declaration in the descriptor.
    pub position: SourceElementPosition,
    /// Original interaction offset in owner space.
    pub offset: SocSourceValue<[f64; 3]>,
}

/// How a baked group's helper was selected within the actual owner.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum InteriorLightHelper {
    /// The group declares no usable helper or attachment point.
    Undeclared,
    /// A helper or `MultiLight` selector could not be resolved uniquely.
    Unresolved(InteriorLightHelperError),
    /// One exact authored helper resolves on this owner's model.
    Matched(Box<InteriorLightHelperFrame>),
}

/// Why no unique helper frame was available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum InteriorLightHelperError {
    /// Explicit helper spelling was empty or repeated.
    InvalidExplicitHelper,
    /// The complete selector does not resolve to one physical descriptor port.
    TilePortUnavailable,
    /// No physical owner vehicle implementation is available.
    VehicleUnavailable,
    /// No `MultiLight` attachment point has the requested exact name.
    PointUnmatched,
    /// More than one `MultiLight` attachment point has the requested exact name.
    PointAmbiguous,
    /// The selected `MultiLight` point has no unique authored bone.
    PointWithoutBone,
    /// The owning model has no matching node or a usable path to its root.
    ModelHelperUnavailable,
    /// The same helper name appears more than once in the owning model.
    ModelHelperAmbiguous,
    /// A helper-relative frame could not be represented.
    InvalidFrame,
}

/// Where the helper name came from.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InteriorLightHelperSource {
    /// The original group's helper attribute.
    Group,
    /// One selected vehicle declaration and its value's modification origin.
    VehiclePoint {
        /// The effective part that declares the `MultiLight` port.
        part: String,
        /// The attachment point's original spelling.
        point: String,
        /// The XML document used by the effective declaration.
        document: String,
        /// The selected value came from the base, patch or an Elem.
        origin: ValueOrigin,
    },
}

/// A rest-frame binding to a unique node of the owning vehicle.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct InteriorLightHelperFrame {
    /// The exact name read from the group or effective vehicle definition.
    pub name: String,
    /// The node's original ordinal on the owning model.
    pub node: usize,
    /// The helper transform in the owning Entity's frame.
    pub owner_from_helper: Transform,
    /// The corrected baked light expressed in the helper's local rest frame.
    pub helper_from_light: Transform,
    /// The source of the helper name.
    pub source: InteriorLightHelperSource,
}

pub(super) struct TileLightPlacement<'a> {
    owner: TileLightOwner<'a>,
    descriptor: Option<&'a str>,
    ports: Option<SocTilePortIndex<'a>>,
}

impl<'a> TileLightPlacement<'a> {
    pub(super) fn new(
        owner: TileLightOwner<'a>,
        descriptor: Option<(&'a str, &'a SocContainerDescriptor<'_>)>,
    ) -> Self {
        Self {
            owner,
            descriptor: descriptor.map(|(name, _)| name),
            ports: descriptor.map(|(_, descriptor)| SocTilePortIndex::read(descriptor)),
        }
    }

    pub(super) fn resolve(
        &self,
        light: &SocLight,
        group: &SocGroupPlacement,
    ) -> (Transform, InteriorLightPlacement) {
        let (anchor, port) = self.anchor(group);
        let transform = match anchor {
            Some(Ok(anchor)) => anchor.move_light(light.transform),
            _ => Some(light.transform),
        };
        let (transform, anchor) = transform.map_or(
            (
                light.transform,
                Some(Err(SocTileLightAnchorError::UnrepresentableLight)),
            ),
            |transform| (transform, anchor),
        );
        let helper = self.helper(group, transform, port.is_some());
        (
            transform,
            InteriorLightPlacement {
                owner_from_container: self.owner.frame,
                port,
                anchor,
                helper,
            },
        )
    }

    fn anchor(
        &self,
        group: &SocGroupPlacement,
    ) -> (
        Option<Result<SocTileLightAnchor, SocTileLightAnchorError>>,
        Option<InteriorTilePort>,
    ) {
        use SocTileLightAnchorError as Error;
        let name = match &group.tile {
            SocSourceValue::Absent => return (None, None),
            SocSourceValue::Present(name) => name,
            _ => return (Some(Err(Error::InvalidSelector)), None),
        };
        let (Some(ports), Some(descriptor)) = (&self.ports, self.descriptor) else {
            return (Some(Err(Error::DescriptorUnavailable)), None);
        };
        let target = ports.named(name);
        let port = if let SocTilePortTarget::Matched(site) = target {
            Some(InteriorTilePort {
                descriptor: descriptor.into(),
                position: site.position.clone(),
                offset: site.offset,
            })
        } else {
            None
        };
        (
            Some(SocTileLightAnchor::resolve(
                target,
                self.owner.frame,
                Some(group.authored),
            )),
            port,
        )
    }

    fn helper(
        &self,
        group: &SocGroupPlacement,
        light: Transform,
        has_tile_port: bool,
    ) -> InteriorLightHelper {
        let binding = match &group.helper {
            SocSourceValue::Present(name) => Ok((name.as_str(), InteriorLightHelperSource::Group)),
            SocSourceValue::Absent => match attachment_point(&group.tile) {
                Some(point) if has_tile_port => vehicle_helper(self.owner.vehicle, point),
                Some(_) => Err(InteriorLightHelperError::TilePortUnavailable),
                None => return InteriorLightHelper::Undeclared,
            },
            _ => Err(InteriorLightHelperError::InvalidExplicitHelper),
        };
        let result = binding.and_then(|(name, source)| self.helper_frame(name, source, light));
        match result {
            Ok(frame) => InteriorLightHelper::Matched(Box::new(frame)),
            Err(error) => InteriorLightHelper::Unresolved(error),
        }
    }

    fn helper_frame(
        &self,
        name: &str,
        source: InteriorLightHelperSource,
        light: Transform,
    ) -> Result<InteriorLightHelperFrame, InteriorLightHelperError> {
        use InteriorLightHelperError as Error;
        let model = self.owner.model.ok_or(Error::ModelHelperUnavailable)?;
        if model
            .nodes
            .iter()
            .filter(|node| node.name.eq_ignore_ascii_case(name))
            .nth(1)
            .is_some()
        {
            return Err(Error::ModelHelperAmbiguous);
        }
        let helper = placement::node_placement(model, name).ok_or(Error::ModelHelperUnavailable)?;
        let owner_from_container = self.owner.frame.ok_or(Error::InvalidFrame)?;
        let helper_from_owner = helper.transform.inverse().ok_or(Error::InvalidFrame)?;
        let helper_from_light = helper_from_owner
            .then(&owner_from_container)
            .then(&light)
            .finite()
            .ok_or(Error::InvalidFrame)?;
        Ok(InteriorLightHelperFrame {
            name: name.to_owned(),
            node: helper.node,
            owner_from_helper: helper.transform,
            helper_from_light,
            source,
        })
    }
}

fn attachment_point(selector: &SocSourceValue<String>) -> Option<&str> {
    let SocSourceValue::Present(name) = selector else {
        return None;
    };
    tile_attachment_point_suffix(name)
}

fn vehicle_helper<'a>(
    vehicle: Option<&'a VehicleImplementation>,
    point: &str,
) -> Result<(&'a str, InteriorLightHelperSource), InteriorLightHelperError> {
    use InteriorLightHelperError as Error;
    let vehicle = vehicle.ok_or(Error::VehicleUnavailable)?;
    let mut matches = vehicle.light_ports().flat_map(|port| {
        port.attachments().iter().filter_map(move |attachment| {
            (attachment.name() == SocSourceValue::Present(point)).then_some((port, attachment))
        })
    });
    let (port, attachment) = matches.next().ok_or(Error::PointUnmatched)?;
    if matches.next().is_some() {
        return Err(Error::PointAmbiguous);
    }
    let SocSourceValue::Present(bone) = attachment.bone() else {
        return Err(Error::PointWithoutBone);
    };
    let (_, origin) = attachment
        .bone_values()
        .next()
        .ok_or(Error::PointWithoutBone)?;
    let document = vehicle.source_path(origin).ok_or(Error::PointWithoutBone)?;
    Ok((
        bone,
        InteriorLightHelperSource::VehiclePoint {
            part: port.part().name().to_owned(),
            point: point.to_owned(),
            document: document.to_owned(),
            origin,
        },
    ))
}

pub(super) fn unique_descriptor(index: &PathIndex<'_>, paths: &[String]) -> bool {
    let mut matches = paths
        .iter()
        .filter_map(|path| {
            index
                .matching_entries_with(path, |_| Ok::<(), std::convert::Infallible>(()))
                .ok()
        })
        .flatten();
    matches.next().is_some_and(|entry| entry.is_ok()) && matches.next().is_none()
}

// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[cfg(target_os = "linux")]
use std::sync::atomic::AtomicBool;

#[cfg(target_os = "linux")]
use crate::portal_alias::AliasUncertainty;
use crate::portal_alias::UnknownAlias;
use crate::portal_mount::PortalMount;
use crate::portal_mount_binding::MountBinding;
use crate::portal_resolution::DocumentsReply;

/// An unmodified Documents reply and the corresponding visible mount evidence.
#[derive(Debug)]
pub(crate) struct BoundDocumentsReply {
    reply: DocumentsReply,
    binding: MountBinding,
}

impl BoundDocumentsReply {
    #[cfg(target_os = "linux")]
    pub(crate) fn bind(
        reply: DocumentsReply,
        cancel: &AtomicBool,
        inventory: impl FnMut() -> Result<Vec<u8>, UnknownAlias>,
    ) -> Result<Self, UnknownAlias> {
        reply.validate()?;
        let binding = MountBinding::capture(&reply.mount_point, cancel, inventory)?;
        Ok(Self { reply, binding })
    }

    pub(crate) const fn reply(&self) -> &DocumentsReply {
        &self.reply
    }

    pub(crate) fn check_mount(&self, mount: &PortalMount) -> Result<(), UnknownAlias> {
        self.binding.check_declared(&self.reply.mount_point)?;
        self.binding.check_mount(mount)
    }

    #[cfg(target_os = "linux")]
    pub(crate) fn probe(&self) -> Result<(), UnknownAlias> {
        if !self.reply.paths.is_empty() {
            return Err(UnknownAlias::new(AliasUncertainty::MalformedReply));
        }
        self.binding.check_declared(&self.reply.mount_point)
    }

    #[cfg(test)]
    pub(crate) fn synthetic(
        reply: DocumentsReply,
        binding: MountBinding,
    ) -> Result<Self, UnknownAlias> {
        reply.validate()?;
        binding.check_declared(&reply.mount_point)?;
        Ok(Self { reply, binding })
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests;

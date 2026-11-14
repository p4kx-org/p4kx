// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fmt;

pub(crate) type Result<T> = std::result::Result<T, Box<dyn Error>>;

#[derive(Debug, Clone, Copy)]
pub(crate) enum Check {
    SourceIdentity,
    SourceAmbiguous,
    ProcessExit,
    Schema,
    ManifestField,
    TextureFile,
    TextureDuplicateRendition,
    TextureDuplicateId,
    TextureDuplicateFile,
    TextureReference,
    LayerBudget,
    Glb,
    MeshBytes,
    MeshInstances,
    ReportCount,
    Textures,
    MeshSharing,
    MeshRatio,
    MinimumMeshes,
    PackageSize,
    Interior,
    CharacterRig,
    CharacterAnimations,
    CharacterParts,
    CountOverflow,
    Cleanup,
    Build,
    Verify,
    Scratch,
}

impl fmt::Display for Check {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "package_{self:?}")
    }
}
impl Error for Check {}

pub(crate) struct Fault {
    check: Check,
    source: Option<Box<dyn Error>>,
}

impl Fault {
    pub(crate) fn at(check: Check, source: impl Error + 'static) -> Box<dyn Error> {
        Box::new(Self {
            check,
            source: Some(Box::new(source)),
        })
    }
    pub(crate) fn boxed(check: Check, source: Box<dyn Error>) -> Box<dyn Error> {
        if source.is::<Self>() || source.is::<Check>() {
            source
        } else {
            Box::new(Self {
                check,
                source: Some(source),
            })
        }
    }

    #[must_use]
    pub(crate) fn original(&self) -> Option<&(dyn Error + 'static)> {
        self.source.as_deref()
    }
}

impl fmt::Debug for Fault {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(output, "package_{:?}", self.check)
    }
}
impl fmt::Display for Fault {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, output)
    }
}
impl Error for Fault {}

pub(crate) fn require(condition: bool, check: Check) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Box::new(check))
    }
}

#[expect(
    clippy::print_stderr,
    reason = "only static package failure categories are reported"
)]
pub(crate) fn report<T>(result: Result<T>) -> Result<T> {
    if let Err(error) = &result {
        let mut cause = Some(error.as_ref());
        for _ in 0..64 {
            let Some(error) = cause else {
                break;
            };
            if let Some(check) = error.downcast_ref::<Check>() {
                eprintln!("package verification failed: {check:?}");
                break;
            }
            if let Some(fault) = error.downcast_ref::<Fault>() {
                eprintln!("package verification failed: {:?}", fault.check);
                break;
            }
            cause = error.source();
        }
    }
    result
}

#[cfg(test)]
#[path = "package_fault_tests.rs"]
mod tests;

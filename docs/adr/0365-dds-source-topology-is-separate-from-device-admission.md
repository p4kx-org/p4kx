# DDS source topology is separate from device admission

Status: accepted

## Context

DDS headers describe source arrays, cubemaps and volumes independently of a particular rendering
device. Microsoft defines the DX10 array count as an unsigned word, with six images per cube;
see [DDS_HEADER_DXT10](https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dds-header-dxt10)
and the [DDS file layout](https://learn.microsoft.com/en-us/windows/win32/direct3ddds/dx-graphics-dds-pguide).
A fixed device-derived count is insufficient reason to classify a structurally readable header
as invalid. Complete 2,049-element R8 arrays and volumes occupy only a few kilobytes.

Removing a count fence must also preserve aggregate cardinality. Six faces times an original
32-bit cube count can exceed 32 bits. Saturation, unchecked multiplication and narrowing an image
plan would lose source identity even when no payload is decoded.

## Decision

The DDS reader retains source topology without fixed array-size or depth ceilings. Existing
contradiction and checked byte-extent validation stay independent. The raw header continues to
preserve every original word before normalization.

Aggregate surface/image counts, Mip image indices, preview-unit counts and the corresponding
unsupported-image error use 64 bits. Original layer, face and slice coordinates retain their
original 32-bit domains. Image plans check addressable iterator length, keep exact size hints and
support direct nth access without constructing or traversing every preceding image.

Parsing a header and planning images do not admit a complete payload or device resource. Actual
header-tail/part extents must still match before conversion. Preview pixel/data budgets and
checked target dimensions remain separate. Sheet dimensions are checked before pixel allocation.
An incomplete body cannot become a successful image list merely because its header is readable.

## Consequences

Rust callers use the exact wider aggregate/count interfaces. The two unchanged historical
4,096-element fixtures validate their original counts and complete bytes; they do not enforce a
removed policy as a format rule. Existing malformed topology, truncated data, mip bounds and
trailer controls continue to apply.

The synthetic large-cube fixture proves only header cardinality, exact planning and direct tail
access. It deliberately has a small incomplete body and asserts its independent refusal; it is
not evidence of materializing billions of images or measured peak memory. Supported native
AMD64/ARM64 platforms can represent every normalized source image count in their iterator length.

# v2 directory sections

P4KX reads the base and extension directories and their combined name table. The 175-byte v2
trailer declares separate counts and sizes; each directory section contains exactly 204 bytes per
Entry. Extension Entries follow base Entries. Name offsets address the complete name table,
including extension names.

Counts and sizes use checked addition. Each section is checked independently, and the combined
Entry count is subject to the same nested-container limit as an all-base directory. The complete
directory must end at the name table; all directory/name bytes must precede the trailer. Invalid
declarations fail the open before an apparently complete base subset is exposed.

The tests independently encode the control and split directories, verify both payloads through the
public API and cover empty sections, overflow, truncated data, bad names and nested limits. No game
Archive or writer implementation is required.

Format field evidence:
[the pinned independent trailer description](https://github.com/19h/svarog/blob/2e46eff4f7d475b8826ccfdc01c4a95db764db0d/crates/svarog-p4k/src/zip/eocd_v2.rs)
and [its checked range interpretation](https://github.com/19h/svarog/blob/2e46eff4f7d475b8826ccfdc01c4a95db764db0d/crates/svarog-p4k/src/archive.rs).

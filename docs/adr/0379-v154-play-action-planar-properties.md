# Version-154 Play actions retain original planar properties and flags

The checked Play continuation belongs to bank version 154 and Action group
`0x04xx`. After the original target ID, it contains the original target flag
byte, the ordinary property list and the ranged modifier list. The fade flag
byte, bank ID and bank type follow the lists. The reader preserves complete flag
bytes, including unknown bits.

Both lists use the same physical format as the version-154 node property
lists. Each count is an unsigned byte followed by its planar array of property
IDs. The ordinary list supplies one raw little-endian word per ID. The ranged
list supplies a little-endian minimum and maximum word per ID. The reader
preserves repeated IDs, explicit zero and every complete original word,
including nonfinite binary32 encodings. Property-specific interpretation can
use the original bits and code as separate inputs.

A partial ID plane supplies its readable IDs. A short word plane supplies its
complete prefix, including an independently readable ranged minimum. Completed
property and fade fields retain their coverage if the later bank reference is
truncated. Media replays the checked ranges from the owning physical bank and
attaches a receipt to each typed value. The original Action resolution retains
its established bank ID, bank type and layout rules.

The source-field iterator supplies an owner-bound cursor for the qualified
Play continuation. The existing Action detail in ObjectView, CLI and MCP keeps
its established shape. Media records expose the new property and flag evidence.
The layout follows [wwiser's Action header](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L1386),
[Play continuation](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L1277),
[ordinary property bundle](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L108)
and [ranged property bundle](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L148).

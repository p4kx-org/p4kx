# Version-154 Switch containers expose authored memberships and configurations

HIRC type 6 in version 154 stores a group type byte, group ID, default value
ID and continuous-validation byte after its shared node base. The original
child list is followed by a 32-bit count of membership groups. Each group
contains a value ID, a 32-bit member count and an ordered list of child IDs.
The final list contains a 32-bit count followed by fixed 13-byte child
configurations. Each configuration contains a child ID, flags byte,
signed fade-out word and signed fade-in word.

The reader uses the owning object's checked range for both declared lists.
It preserves ordering, repeated IDs, empty lists, zero defaults, unknown
codes and flags, and every complete field in a truncated row. Per-child
configuration words retain their original bits and signed interpretations.
The original child order and authored memberships are separate observations.
Known group type codes 0 and 1 identify Switch and State respectively.

The Media summary, membership groups, members and configurations use
physical ordinals and field origins in the owning bank. Child reference
resolution stays unchecked. The raw envelope and existing child list
remain available. The rows preserve original fade units and carry no
claim about evaluated selection or playback.

The v154 layout is established by [wwiser's Switch reader](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wparser.py#L1592)
and [StarBreaker's container reader](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/containers.rs#L106).
The known group type codes follow [wwiser's group type table](https://github.com/bnnm/wwiser/blob/6d4b5bfb45afc52cda9d68fbbe0cbd97de0e26a3/wwiser/parser/wdefs.py).


The CLI and MCP ObjectView stream the same original Switch fields and
membership and configuration rows. The owning object records its checked
coverage and per-value physical receipts. Partial groups and configurations
retain the complete fields that fit. MCP applies its existing whole-object
response budget.

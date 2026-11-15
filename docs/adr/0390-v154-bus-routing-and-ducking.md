# Version-154 bus declarations retain exact routing words

AudioBus (type `0x08`) and AuxiliaryBus (type `0x12`) share the checked v154
layout. The override bus word selects the root device word when it is zero.
A planar property list, positioning and auxiliary routing precede an eight-byte
bus settings group. The group stores one playback flags byte, a 16-bit instance
limit, a packed 32-bit channel configuration and one HDR flags byte.

Recovery and fade times keep their authored signed words. The maximum and
per-target duck volumes keep their binary32 words. Every ordered duck row
holds the target bus ID, volume, fade-out and fade-in words, curve code and
property code. Effect and metadata lists start at their own counts. The
conditional effect bypass byte belongs only to a nonempty effect list.
State chunks and RTPCs reuse the original v154 checked readers.

The independent decoder compared the retained original bank frame and exact
HIRC object end across four CRC-verified banks from LIVE 13165946 and four
from PTU 13189690. All 2,029 bus objects matched the eight-byte settings and
direct metadata count. The alternative that omits the settings and inserts a
metadata override byte had zero exact matches. The installed scope exercises
properties, positioning, Aux sends, root devices, ducking, effects, RTPCs and
states. The scoped metadata lists were empty, so synthetic fixtures exercise
nonempty metadata. [wwiser v154 bus layout](https://github.com/bnnm/wwiser/blob/24eb88f3b2a99d54c1062d8438a7bd04be88335d/wwiser/parser/wparser.py#L1687-L1998) · [StarBreaker comparison](https://github.com/diogotr7/StarBreaker/blob/7d3f01e76307354bbc56a64a70eb5167b1189b64/crates/starbreaker-wwise/src/hirc/bus.rs#L40-L105)

Typed source coverage is exposed through the checked original object projection.
The same grammar classifies eager HIRC coverage and retains the existing
relationships. Unsupported versions and partial or trailing bytes retain the
original body through the raw envelope. Field provenance comes from the
owning BNK byte ranges. A routing target is an authored reference and its
payload does not contribute to the source of that reference.

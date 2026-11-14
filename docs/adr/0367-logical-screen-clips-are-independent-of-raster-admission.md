# Logical screen clips are independent of raster admission

Status: accepted

A BuildingBlocks composition retains every logical clip mask it needs. A fixed mask count does
not measure raster memory: masks of independent items can be released between items, and the
actual buffer cost depends on output dimensions and overlapping lifetimes. The extra count-24
fence dropped later clips and could draw source-hidden content outside its authored boundary.

Remove that composition fence and its obsolete overflow flag/note. Keep mask reuse and parent
links intact. The existing rasterizer lazily builds masks, charges live mask bytes and fill work,
and releases masks after their final use. If a required mask cannot be admitted, it skips the
affected drawing item with a reason rather than drawing that item without its clip. SVG retains
its separate output/work admission.

The same two synthetic original-source fixtures validate 25 independent clips and a 25-mask
ancestor chain before composition. Pixel checks cover the final excluded boundary; scene parent
links and SVG mask definitions/usages cover complete logical retention. These small cases do
not establish a whole-process memory bound or remove the other screen or GFx source policies.

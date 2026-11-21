# Screen omissions identify the authored widget and binding

## Context

The state at rest evaluates Screen operations with their default inputs. A
caller can supply a typed game variable for supported operations through
[ADR-0259](0259-screens-take-game-variables-from-the-caller.md). The authored
assets, graph bindings and images produced by the game have different inputs.

| Record source | Use in a Screen rendered at rest |
| --- | --- |
| `WidgetImage.imagePath`, SVG path | Load the referenced Archive image using the existing path and format rules. Retain the asset failure if it is unavailable. |
| `WidgetRuntimeImage.imageSource` | Retain the authored game render-target name, for example the target that supplies a ship silhouette. |
| `NumberField` or `IntegerField` | Evaluate the supported numeric layout and visibility operations with their typed values. |
| `LocalizedField` bound to a localised variable or text parameter | Display resolved localisation or supplied text. Static text that contains digits is rendered as authored. |
| `WidgetList.arrayVariable` | Identify the runtime array for absent list rows, including pips that rely on those rows. |

## Decision

- A named runtime image and an unavailable runtime array produce diagnostics
  with the original field and its authored name. Render-target names are
  runtime identifiers. Static image paths use
  [ADR-0105](0105-screen-images-are-bitmap-fills-and-a-small-svg-reader-bounded-per-screen.md).
- Supported numeric and boolean fields keep their type's state-at-rest
  value when the caller supplies none. A supplied zero is valid. A
  directly bound localised variable with no caller value produces a
  diagnostic and keeps an authored label or parameter fallback. An
  explicitly supplied empty value clears that text.
- Type and range checks apply at the operation and the destination field.
  The renderer reports an unsupported binding or a failed input with the
  original widget identity, the field and the direct variable path when
  known. `variables` keeps the settings the caller actually supplied.
- The structured report keeps each original canvas Record index and
  array slot and one row per authored field or operation and omission code.
  Grouped diagnostics continue to count rendered visits. Values from a
  widget or an operation carry the corresponding
  DataForge source. The drawn artifact and the diagnostic use their own
  source sets.
- The available format evidence identifies the silhouette as a runtime
  target and the shield and power readouts as runtime values. It leaves the
  numeric-to-text operation and its exact format unresolved. The renderer
  reports unsupported bindings until the source can establish that operation.

[Screen layout numbers](0296-screens-colour-the-text-of-a-field-by-parent-entries-and-evaluate-bound-layout-numbers.md)
and the [MFD source comparison](0306-screens-read-integer-constants-in-full-and-choose-the-page-of-an-mfd-by-a-variable.md)
record the observed operations and readouts.

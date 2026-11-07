# Shader extension source reader

`p4kx-shader` reads bounded `.ext` source bytes into ordered, source-spanned declarations.
It has no filesystem, Archive, exporter, material renderer or shader compiler dependency.
`read` accepts bytes and limits; `read_with` additionally preserves a fallible caller checkpoint's
original error. Invalid UTF-8 has an explicit `UnknownEncoding` failure and never becomes lossy text.
The caller retains raw bytes independently when structural reading fails.

The original payload is borrowed unchanged, including UTF-8 BOM, comments and whitespace.
All spans are byte offsets from that original decoded payload, with end-exclusive bounds.
Known-field views borrow the same syntax; repeated fields, unknown names and unknown scopes remain
ordered. Missing declarations, bare markers, explicit empty values and zero remain distinct.
Quoted token spelling and escape sequences stay raw; the reader does not infer engine escape semantics.

The structural profile recognises balanced braces/parentheses, single/double quoted tokens with
backslash-protected delimiters, line/block comments, equals values and parenthesised values.
Newlines and semicolons end equals values outside groups; a braced body can begin on the next line
after its name. Other shapes remain source-spanned unknown declarations. Block-comment support and
both quote forms are explicit lexical support, not claims that the installed samples use every form.

Top-level `Property { ... }` bodies expose ordered `Name`, `Mask`, `Property` label, `Description`,
`DependencySet`, `DependencyReset` and `Hidden` fields. Root `Version` and `UsesCommonGlobalFlags`
retain all authored forms and duplicates. A version string is not assigned a guessed version grammar;
common flags do not acquire inferred inheritance, and dependency symbols are not filesystem paths.

Mask interpretation supports `0x`/`0X` hexadecimal unsigned 64-bit words and the unambiguous `0` token.
The width is the reader's stated support boundary, not a claim about every source format variant.
Unprefixed nonzero tokens retain an explicit unknown-radix result. Symbolic/compound/quoted expressions,
malformed hex and overflow retain distinct results plus the raw value. Consumers serialize interpreted
words as decimal or hexadecimal strings, never through JSON floating-point precision.

Grammar facts come from the installed-PTU structural audit described in issue #1109 and the pinned
[primary comparison parser](https://github.com/Markemp/Cryengine-Converter/blob/7b951dbb254ab99f560e8ffbf64a771c70ccef74/CgfConverter/Parsers/ShaderExtParser.cs).
The implementation is independent; the reference's regex, dictionary, signed-mask conversion and
zero fallback are not copied. Two installed samples do not establish a complete language grammar.
The source span is authoritative when this reader cannot assign a supported interpretation.

Limits bound source/token/value bytes, token/declaration/property counts and combined nesting before
storage grows. Recursive structural work has an additional hard depth ceiling of 128. `Limits` is an
option struct with `Default` and functional record update. Caller checkpoints run before/after UTF-8
validation, during lexical byte scanning and during structural units. Allocation failures are fallible.

Compiled `.cfib`, `.cfxb` and `.shcb` caches have no decoder here. No `.dpsi`, `.fx`, MTL link,
renderer appearance or numeric GenMask interpretation is inferred by this reader.

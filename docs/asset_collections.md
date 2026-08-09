# Asset collections

An asset collection is a multi-output procedural build document. Its extension
is `.spriterize-collection`; physically it is a zip archive with a versioned,
human-readable `manifest.json`.

It organizes `.spriterize` projects without changing what a project means:

- A project is one editable canvas with layers, frames, undo history, and
  optional layer generators.
- A collection owns editable projects plus shared generator resources and
  named export specifications, potentially with different dimensions.

## Manifest model

The version-2 manifest contains:

- `script_libraries`: reusable Rune helper functions.
- `generators`: script or graph definitions. A script generator may include an
  ordered list of shared script libraries.
- `assets`: output id, generator reference, dimensions, relative output path,
  knob overrides, export options, and metadata.
- `projects`: editable project id, display name, archive entry, output path,
  export options, and metadata. The project payload itself is stored under
  `projects/` with the same compatibility header as a standalone project.

Asset metadata currently supports tags and Defold-style slice9 margins. A
single generator can serve any number of outputs with different knob values.
Graph resources are shared as complete generator templates; script libraries
provide finer-grained reuse for text generators.

## Safety and reproducibility

Collections are validated when loaded, saved, or exported. Validation rejects
duplicate or empty ids, missing resources, unsafe paths, duplicate output
paths, zero dimensions, invalid slice9 margins, and script libraries attached
to graph generators.

Export generation is preflighted in memory before the destination directory is
modified. Output paths must contain only normal relative components, so an
archive cannot write outside the selected export directory. Noise is stable for
the Rust artlib implementation but is not expected to reproduce CPython's
random stream byte-for-byte.

## Application workflow

Spriterize starts on a document launcher instead of assuming a blank painting
project. From there you can create a collection, open a collection/project/image,
start a new image project, or reopen a recent document.

Creating a collection asks for its archive path and immediately writes a valid,
empty manifest whose collection name comes from the filename. Use **File → New
Asset Collection…** to do the same while the editor is open. In the collection
browser, **New Project** creates a sized editable asset and loads it into the
ordinary painting/generator editor. Select another project name to save the
current project back into the archive and switch editing contexts.

Use **File → Open Asset Collection…** to load a collection alongside the active
project. Newly created and opened collections show their browser immediately;
outputs can be exported individually or all at once. **File → Start Screen**
returns to the launcher without discarding the dormant project. Opening a
collection does not replace or mutate the current raster project.

Embedded projects retain their layers, frames, generator definitions, knob
values, and other normal editor state. Saving, switching projects, exporting,
returning to the start screen, and exiting all persist the active project back
to its collection. Version-1 generator-only collections remain readable.

# PreviewWindow Status

## Current State

`PreviewWindow` is in a better state than it was at the start of this refactor, but the core complexity still lives in `src/components/PreviewWindow.tsx`.

Completed work so far:

- Lifecycle and preview-image event intake moved to `src/hooks/usePreviewWindowLifecycle.ts`.
- File navigation and index recovery moved to `src/hooks/usePreviewNavigation.ts`.
- The `navigate-image` bridge now has a shared event constant in `src/hooks/preview-window-events.ts`.
- The preview config write path now goes through the shared `configStore` update path instead of ad hoc writes.

This means the easy extractions are done. The top-level component is thinner, but `PreviewWindowContent` is still the real controller.

## Remaining Responsibility Clusters

The following concerns still live together inside `src/components/PreviewWindow.tsx`:

- EXIF loading and metadata display.
- Zoom, pan, drag, and reset behavior.
- Fullscreen and always-on-top window behavior.
- Global keyboard shortcuts.
- Toolbar visibility timing and hover behavior.
- Local preview config state (`autoBringToFront`) and event sync.
- Rendering for image stage, toolbar, metadata, and navigation controls.

The component therefore remains a controller-view hybrid rather than a mostly presentational composition layer.

## Why Small Further Extractions Have Diminishing Returns

Another small hook extraction would mostly move code without improving ownership much.

The remaining complexity is tightly coupled behavior:

- zoom and drag math depend on image stage state,
- navigation depends on recovery and image-path updates,
- fullscreen and keyboard handling depend on current window state,
- preview config sync depends on both backend events and local UI state.

Because of that, further piecemeal splitting is likely to create more indirection without removing the real coupling.

## Recommended Refactor Direction

The next worthwhile `PreviewWindow` refactor should be a controller-level redesign, not another small extraction.

Recommended target shape:

- Create a single preview controller hook or module that owns:
  - current image path,
  - navigation state,
  - fullscreen state,
  - preview config sync,
  - side-effectful commands.
- Reduce `PreviewWindow.tsx` to composition and rendering.
- Split the rendered UI into smaller presentational pieces if helpful, such as:
  - image stage,
  - toolbar,
  - metadata panel,
  - navigation controls.

## Priority Guidance

`PreviewWindow` is still worth refactoring, but it should not be the next piecemeal target.

At the current repository state, `GalleryCard` is the better next investment because:

- it is larger,
- it still mixes more unrelated responsibilities,
- and its first decomposition slice is easier to isolate safely.

Recommended order:

1. Continue with `GalleryCard` decomposition.
2. Return to `PreviewWindow` only when ready to do a larger controller-level refactor.

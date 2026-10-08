# Oxide UI redesign plan

## Current UI architecture

- **Framework:** native Rust `eframe`/`egui` 0.36.2 on top of `winit`; one native window is used so Windows capture exclusion applies to every Oxide-owned surface.
- **Application shell:** `crates/app/src/app.rs` owns `App`, the native window, lifecycle, backend responses, search state, note editor state, screenshot state, and all view transitions.
- **Surface modules:** `app/notes.rs` renders the note sidebar/editor; `app/screenshots.rs` renders capture controls, screenshot library, detail, OCR lines, and highlights; `app/settings.rs` renders settings and filter controls.
- **Navigation:** no router. `library`, `all_view`, `trash_view`, `settings`, `detail`, `compact`, and `capture` state determine the visible surface. Existing requests/responses and lifecycle actions remain the integration boundary.
- **State management:** `App` receives bounded asynchronous backend responses, keeps the editor generation state in `Editor`, and invalidates thumbnail/full-image textures by screenshot digest/update revision.
- **Styling:** `apply_theme` sets the egui theme and global body/button typography. Most visual decisions are currently inline: hardcoded orange `Color32` values, default egui panels/buttons, ad-hoc spacing, and per-surface frame styling.
- **Reusable UI:** currently limited to `capture_controls`, `picture`, `shot_tile`, `highlight`, and local egui closures. There is no semantic design-token layer, shared button hierarchy, reusable navigation item, or common surface/card helper.

## Problems found

### Hierarchy and layout

- The top row carries navigation, a wide search field, exclusion state, settings, compact mode, hide, and quit. It is visually dense and makes primary navigation compete with window/lifecycle controls.
- The requested workspace/sidebar model is missing: note browsing is only a narrow left panel while screenshots and all-results controls compete for the same top row.
- Capture/import/paste controls are repeated in the note editor and screenshot library, with equal visual weight despite different frequency and importance.
- Screenshot tiles use a default grouped frame and checkbox, making a media library look like a form rather than a visual index.
- Detail, settings, confirmation, and recovery states are generic egui windows with no consistent surface hierarchy or spacing system.

### Typography and color

- Type scale is mostly default egui plus a few isolated 11/12/22/27 px values. Metadata, section labels, body copy, and primary headings do not form a consistent rhythm.
- Oxide orange is repeated as raw RGB values rather than a named accent token, making hover/selected/focus states inconsistent.
- Theme behavior relies on egui defaults; dark mode has no deliberately tuned surfaces, borders, muted text, selection fill, or focus treatment.
- Status labels use all-caps `LOCAL ONLY` and `ATTACHMENTS` without a broader label convention.

### Controls and states

- Default egui buttons make destructive, primary, secondary, and tertiary actions visually equivalent.
- Active navigation is only a default selectable label; there is no clear selected rail treatment.
- Keyboard focus exists through egui but is not visually calibrated as part of the product design.
- Empty, loading, unavailable, OCR-pending, and error states are functional but visually abrupt and inconsistent.
- Recording exclusion, save status, reader status, and notices are all placed in the bottom status area or header without a clear status hierarchy.

### Code quality / maintainability

- Visual constants are duplicated across files.
- Large UI closures mix layout, state mutation, and actions, which makes visual changes risky even though the backend contracts are stable.
- There is no single place to tune density, panel widths, border radii, elevation, or accent contrast for both light and dark modes.

## Target architecture

The redesign remains a single native egui window and keeps existing state/request behavior intact.

```text
App
└── shell
    ├── top_bar
    │   ├── oxide wordmark / active workspace context
    │   ├── global search + keyboard hint
    │   ├── new-note action
    │   ├── privacy status
    │   └── settings
    ├── workspace_rail
    │   ├── Workspace: All / Notes / Screenshots
    │   ├── Collections: Work / Projects / future saved scopes
    │   ├── Recent / note trash
    │   └── New note / Capture
    ├── content_area
    │   ├── section heading + result context
    │   ├── notes list/editor
    │   ├── screenshot library
    │   └── all-results sections
    └── persistent_status
        ├── save state
        ├── reader/OCR state
        └── local-only indicator
```

### Design system

- **Palette:** warm graphite neutrals for dark mode, cool-white neutrals for light mode, one restrained oxide-orange accent (`#B86D4B` family), semantic red only for errors. No gradients or glass layers.
- **Surfaces:** application background, rail background, content surface, raised overlay. Use subtle 1 px borders and small elevation only for floating detail/settings/confirmation surfaces.
- **Geometry:** 8 px base rhythm; 6 px controls, 8 px tiles, 10 px dialogs, 12 px primary surfaces. Avoid pill shapes except compact status indicators.
- **Type:** strong 24 px page title, 16 px body/editor, 13 px navigation/controls, 11 px metadata with tabular figures where useful. Sentence case by default.
- **Control hierarchy:** filled accent for one primary action, quiet bordered buttons for secondary actions, text buttons for tertiary actions, explicit destructive treatment for trash operations.
- **Interaction:** selected rail items have a tinted accent background and left marker; hover shifts surface color; pressed states darken slightly; keyboard focus uses a visible accent outline.
- **Density:** spacious content padding, compact metadata, and a stable 240 px rail / flexible content area. Compact mode keeps the same hierarchy at a narrower width.

### Modal/surface system

- Keep note editing, screenshot browsing, and capture controls inline in the main shell.
- Keep screenshot detail, settings, confirmation, and recovery as consistent centered overlays with the same title/action/footer treatment.
- Keep native file pickers and external viewers as explicit OS-boundary actions.

### Integration constraints

- Do not replace egui or add a UI framework.
- Do not change `Request`, `Response`, `Editor`, lifecycle, capture, OCR, storage, or exclusion semantics.
- Add only UI state needed for navigation presentation and theme tokens.
- Preserve existing keyboard shortcuts and add the visible `⌘K`/`Ctrl+K` search affordance without changing the existing search behavior.

# Oxide - UX, UI, Motion, and Design Review

## Design Direction

Oxide should feel **calm, native, tactile, and technically trustworthy**. The content is screenshots and notes; the interface should provide structure without competing with either. The reference reports describe Gyotaku's neutral chrome and visual emphasis as a useful precedent. Oxide should carry that restraint into a workspace with a stronger note/capture relationship.

### Design principles

1. **The screenshot is the evidence.** Use color and image detail for content; keep chrome quiet.
2. **Capture is primary, organization is secondary.** The fastest path must not require choosing a collection.
3. **State is part of the product.** Saved, pending, unavailable, excluded, and failed are different conditions and need different treatments.
4. **Native behavior over visual novelty.** Prefer platform keyboard conventions, focus, menus, dialogs, and predictable window lifecycle.
5. **Privacy should be explicit and calm.** Show what the exclusion toggle means and what it has not verified.
6. **Motion explains continuity.** An item entering a note, a detail view expanding, and a deletion undo should have understandable spatial relationships.
7. **Density follows task.** Compact mode is dense for live use; full mode gives breathing room for review and organization.

## Current UX Walkthrough

### First launch

**Observed:** `crates/app/src/main.rs` creates a missing config with defaults and opens Settings as onboarding. `App::recovery` in `crates/app/src/app.rs` handles malformed configuration, storage failure, and exclusion failure. The settings surface contains General, Storage, Shortcuts, and OCR/maintenance tabs.

**Assessment:** The app purpose is visible, but first launch asks the user to understand a full workspace before experiencing the first note. OCR model readiness is operationally important but should not block the first note. The recording-exclusion toggle deserves an explanation that distinguishes API treatment from validated recorder compatibility.

**Improve:** Use a three-step first-run sequence:

```text
1. Where should new captures go?
2. Do you want to watch existing screenshot folders?
3. Recording exclusion: On / Off / Unavailable, with one-sentence explanation
```

End with “Create your first note” and allow the worker to prepare in the background.

### Open and summon

**Observed:** `Resident::claim` prevents duplicate app instances. Unix uses a mode-600 socket; Windows uses localhost with a random token. Hide keeps the process alive and clears textures. `--background` starts hidden.

**Assessment:** This is a good foundation for an always-ready tool. The UI does not clearly communicate that close means hide while Quit means exit.

**Improve:** Put a small “Resident” explanation in the menu and use “Hide workspace” consistently. Keep Quit visually separated as a lifecycle action.

### Capture

**Observed:** `start_capture` freezes the active note ID, saves it first, hides the app, and starts a display/region capture. Region selection supports drag and coordinate fields. Escape cancels. Save publishes PNG atomically and attaches it.

**Assessment:** The data behavior is thoughtfully designed. The visual behavior feels like a utility dialog rather than a fast creative gesture.

**Improve:** Use a capture status strip and an attachment confirmation:

```text
Capturing display 1...
    -> Saved to “Incident notes”
    -> OCR pending
```

The confirmation should be an app-owned surface so it remains subject to exclusion. It should not use a content-bearing OS notification.

### Notes

**Observed:** The title and body are plain egui editors, autosave is debounced at 500 ms, and error recovery offers Retry save and Export buffer.

**Assessment:** The plain editor is the correct foundation. It is fast, local, and compatible with Markdown syntax. The current attachment rail and capture menu do not give the screenshot-to-note relationship enough prominence.

**Improve:** Place save state beside the note title, place the primary capture button near the editor, and make attachments read as evidence cards with OCR status and source provenance. Keep Markdown preview optional and secondary.

### Search and retrieval

**Observed:** The header search field advertises `Ctrl/Cmd+K`, and both K and F focus it. Notes and screenshots can be searched separately or together. Filters are provided through a menu and query grammar. Screenshot detail draws normalized line rectangles over the image.

**Assessment:** The semantics are stronger than the presentation. The user needs to understand why a result matched and whether it is a note or screenshot before opening it.

**Improve:** Show:

- result type badge: Note / Screenshot;
- matched phrase or note excerpt;
- capture/import time provenance;
- folder/source when relevant;
- a highlighted OCR line before opening detail;
- keyboard up/down/enter and escape behavior.

### Detail and reuse

**Observed:** Detail offers Copy all, Copy selected, Insert selected, Attach to note, Retry OCR, Trash original, Open externally, and Reveal folder. It also lists linked notes.

**Assessment:** The action inventory is excellent, but the hierarchy is flat. “Trash original” should not be visually adjacent to routine copy actions without stronger separation. The detail surface should feel like a reading view rather than a generic window.

**Improve:** Group actions as:

```text
Primary: Copy selected / Insert selected
Context: Attach to note / Linked notes
Utility: Open externally / Reveal folder
Maintenance: Retry OCR
Destructive: Move original to trash
```

Disable actions that do not apply while detail data for the selected ID is still loading.

## Information Architecture

### Recommended shell

```text
┌─────────────────────────────────────────────────────────────────┐
│ Oxide / workspace context   Search... Ctrl/Cmd+K   + New   ⚙   │
├──────────────┬──────────────────────────────┬───────────────────┤
│ Workspace    │ Current view                 │ Note context      │
│ All          │ heading / filters / status   │ active note       │
│ Notes        │ notes or screenshot results  │ attachments       │
│ Screenshots  │                              │ capture           │
│              │                              │                   │
│ Recent       │                              │                   │
│ Note trash   │                              │                   │
├──────────────┴──────────────────────────────┴───────────────────┤
│ Saved locally      Reader: Up to date        Local only           │
└─────────────────────────────────────────────────────────────────┘
```

The current shell in `crates/app/src/app/shell.rs` already has a rail and note sidebar in full mode. The next design step is not adding another navigation layer; it is making the current layers explain their relationships.

### Navigation rules

- **Workspace rail:** All, Notes, Screenshots, Note trash.
- **Source collections:** watched folders, clearly labeled as sources rather than user-created collections.
- **Recent:** true recent notes or screenshots, not only the current loaded query prefix.
- **Content area:** one task at a time; search context stays visible.
- **Detail:** an overlay or focused panel that returns to the same query, scroll position, and selected result.
- **Settings:** consistent modal surface; advanced maintenance can be a separate tab but not a separate window language.
- **Capture:** an inline action sequence, not a permanent navigation destination.

### Empty states

Each empty state should answer “what happened?” and “what can I do?”

| State | Message direction | Primary action |
|---|---|---|
| No notes | “Your first note starts here.” | Create note |
| No screenshots | “Capture a region, import an image, or connect a folder.” | Capture region |
| No search results | “Nothing matched those terms.” | Clear query / edit search |
| OCR pending | “Saved locally. Text search will be available when the reader finishes.” | View status / pause |
| OCR failed | “The original is safe; recognition needs another attempt.” | Retry OCR |
| Original unavailable | “The note link remains, but the source file is missing or trashed.” | Reveal note / reconcile |
| Storage error | “Oxide cannot read its database.” | Retry / open data folder |
| Exclusion failed | “The workspace is hidden until recording treatment is restored.” | Disable exclusion explicitly |

## UI System

The current `crates/app/src/app/design.rs` is a useful beginning: `Palette` defines canvas, rail, surface, raised, hover, border, text, muted, accent, soft accent, and danger; `apply_theme` establishes typography, spacing, selection, cursor, shadows, and widget states. Continue this direction by treating every visual value as a named semantic token rather than adding raw `Color32` values in feature modules.

### Geometry and density

- Base spacing: 4 px for micro gaps, 8 px rhythm, 12 px control group, 16 px section, 24 px primary content padding.
- Full workspace rail: 224-240 px; current exact 212 px is workable but leaves little room for source names.
- Notes list: 260-300 px depending on window width.
- Compact mode: 440-520 px wide, editor-first, no full library rail.
- Control height: 32-36 px, with 44 px minimum pointer target where space allows.
- Screenshot tile radius: 8 px; modal radius: 10-12 px; avoid pill shapes except status badges.
- Borders: one-pixel low-contrast boundaries; rely on spacing and surface shifts more than heavy outlines.

### Typography

| Role | Size / weight | Usage |
|---|---|---|
| Page title | 24 px semibold | Main view heading |
| Note title | 26-28 px regular/medium | Editing focus |
| Body/editor | 16 px regular, 1.45 line height | Note text and readable detail |
| Navigation/control | 13 px medium | Rail, buttons, filter controls |
| Metadata | 11-12 px regular | OCR state, timestamp, provenance |
| Monospace | 13 px | Shortcut values and technical identifiers |

Sentence case should be the default. Reserve all caps for compact status tags only. Do not make `LOCAL ONLY` a decorative badge that competes with save or privacy state.

### Dark mode tokens

Use the existing warm graphite direction and tune it as a semantic system:

| Token | Suggested role | Direction |
|---|---|---|
| `canvas` | Main background | Warm near-black, current `24/25/27` is a good base |
| `rail` | Navigation surfaces | One step lighter than canvas |
| `surface` | Cards/input backgrounds | Subtle elevation, not glossy |
| `raised` | Dialog/detail overlay | Distinct enough from surface for focus |
| `border` | Dividers/focus context | Quiet cool gray |
| `text` | Primary copy | High contrast warm white |
| `muted` | Metadata/helper copy | Legible, not disabled-gray |
| `accent` | One primary action/highlight | Restrained oxide orange; avoid filling every control |
| `accent_soft` | Selected rail/result | Tinted background with readable text |
| `danger` | Trash/failure | Semantic red, not orange |
| `focus` | Keyboard focus | Bright accent outline with adequate contrast |

### Light mode tokens

Use the existing off-white direction:

| Token | Suggested role | Direction |
|---|---|---|
| `canvas` | Main background | Cool/warm neutral near `250/249/247` |
| `rail` | Navigation | Slightly darker neutral |
| `surface` | Cards/input | Near white, separated by border |
| `raised` | Modal/detail | White with restrained shadow |
| `border` | Structure | Visible but low saturation |
| `text` | Primary | Near-black charcoal |
| `muted` | Metadata | At least readable contrast; do not use placeholder gray for core information |
| `accent` | Primary action | Dark enough for white text or use dark accent text on soft fill |
| `accent_soft` | Selection | Warm low-saturation tint |
| `danger` | Destructive/error | Deep red with text label |
| `focus` | Keyboard focus | Two-pixel visible outline or equivalent |

### Rust identity

A restrained oxide-orange accent is appropriate because `crates/app/src/app/design.rs` already uses a muted `#B86D4B`-family value and the product name supports a material/oxide association. It should not turn the entire UI orange. Use it for:

- the primary capture action;
- selected rail marker and selected OCR line;
- keyboard focus;
- save/capture confirmation icon where semantic color is not more important.

Use blue/green only if a platform semantic convention requires it. Danger remains red. “Local only” is a trust label, not an orange warning.

## Component Style

### Buttons

1. **Primary:** filled accent, one per surface, used for Create note, Capture region, Save settings.
2. **Secondary:** quiet bordered or lightly filled, used for Import, Attach, Retry.
3. **Tertiary:** text action, used for Reveal folder, Clear selection, Dismiss.
4. **Destructive:** red text or outlined red with confirmation; never a neighboring visual peer to Copy.

`primary` and `quiet` exist in `crates/app/src/app/design.rs`; add destructive and icon-with-label helpers only where repeated behavior justifies them.

### Cards and screenshot tiles

Make the image the first-class object. Put filename, OCR status, and selection controls into a quiet footer. Show the selected state with a tinted border/rail marker, not only a checkbox. On hover, reveal copy/open/attach actions without shifting tile geometry.

### Menus

Use menus for infrequent secondary actions. The current “Capture…” and “Actions” menus are reasonable; the primary capture path should also be visible in compact mode. Menus should have keyboard focus, stable ordering, and a clear destructive separator.

### Dialogs and detail

All Settings, detail, confirmation, and recovery surfaces should share:

```text
Title and concise purpose
    -> content with explicit state
    -> primary/secondary actions
    -> optional footer status or keyboard hint
```

Do not use a dialog for a state the user must monitor continuously. OCR progress and save state belong in the workspace shell; confirmation belongs in a modal.

### Inputs and search

Search should look like the product's main command surface. Include the shortcut hint, preserve focus, and show filter chips for parsed date/folder constraints. Do not hide all semantic meaning in a single freeform string; the grammar remains available for power users.

### Tags and status

Tags are future scope. If added, use compact rectangular labels, not a cloud of colored pills. OCR states should use icon + text + color:

```text
● Saved locally
◌ OCR pending
✓ Text indexed
! OCR failed
× Original unavailable
```

Never communicate important state through color alone.

### Notifications and undo

Use an in-app bottom status/toast surface, protected by the same native window. It should enter near the status bar, remain long enough to read, expose Undo when a reversible action exists, and disappear after success. Do not use system notifications for note titles, OCR text, or image previews.

## Motion System

### Principles

- No motion without a communicative purpose.
- No animation should delay a save, capture, search response, or privacy transition.
- Prefer transform and opacity; avoid repeatedly scaling large decoded images.
- Interrupt motion when the user interacts again.
- Never use infinite motion except an active process indicator.
- Respect reduced-motion preferences and provide immediate state changes.

### Motion tokens

| Token | Duration | Easing | Use |
|---|---:|---|---|
| `motion.fast` | 100-140 ms | ease-out | Hover, focus, pressed, selection tint |
| `motion.medium` | 180-240 ms | ease-in-out | Detail open, filter results, rail transition, toast entry |
| `motion.slow` | 280-380 ms | ease-out | First-run or major layout continuity only |
| `motion.instant` | 0-50 ms | none | Reduced motion, errors, privacy transitions, direct keyboard action |

### Interaction choreography

#### Screenshot capture

```text
Capture invoked
    -> workspace hides immediately and safely
    -> selector appears with a clear source label
    -> Save region
    -> thumbnail fades/slides into the active note attachment rail
    -> OCR pending badge appears without waiting for inference
    -> badge resolves to indexed/no text/failed
```

The first two transitions should be short or immediate; privacy and capture correctness are more important than visual polish. The thumbnail insertion motion communicates where the screenshot went.

#### Opening detail

```text
Tile image bounds
    -> detail surface opens from the selected tile's location
    -> image keeps aspect and line coordinates
    -> controls and metadata fade in after the image is usable
```

If egui cannot safely provide a shared-element transition without ownership problems, use a simple opacity/scale transition and preserve the selected tile/query context instead of faking spatial continuity.

#### Search

Do not animate every keystroke with large card movement. Update results quickly, use a 100-140 ms opacity/position transition for changed rows, and highlight the matched line immediately when its texture is ready. An old result must never animate over a newer query generation.

#### Sidebar and filters

Rail expand/collapse can use 180-220 ms width interpolation in full mode. In compact mode, switch directly. Filter chips can use fast tint/size feedback; the result body should not wait for the chip animation.

#### Delete and undo

```text
Confirm
    -> tile fades and collapses into the surrounding grid
    -> status toast says exactly how many originals moved
    -> Undo appears while the in-memory batch is available
```

If native trash fails partially, do not animate all targets as removed. Reflect the per-item result.

#### Theme switching

Use a short 180 ms color interpolation only if egui's rendering keeps text legible during the transition. Otherwise swap theme immediately. Avoid a white flash, and apply the selected theme before showing a newly summoned content window.

## Delightful Micro-interactions

| Interaction | Behavior | Why it helps |
|---|---|---|
| Capture confirmation | Small thumbnail enters the note rail with “Saved locally” and “OCR pending” | Closes the loop between an invisible capture operation and its destination |
| Save acknowledgement | Title-adjacent status changes from “Saving…” to “Saved locally” with a quiet check | Makes durability understandable without a disruptive toast |
| Copy confirmation | Brief “Copied 3 lines” status; never a system notification | Confirms an action that otherwise has no visible effect |
| OCR pending | Badge changes in place to indexed/no text/failed | Shows that the original is usable before intelligence is ready |
| Hover actions | Copy/open/attach controls appear without changing tile size | Keeps the visual library clean while making common actions discoverable |
| Search focus | Shortcut focuses the field and places the caret without changing the current view | Supports keyboard-first retrieval |
| Match highlight | Matched line border and concise snippet appear together | Explains why the result is relevant |
| Attachment link | Clicking a note attachment offers “Open screenshot” and detail links back to the note | Makes the two-way evidence relationship discoverable |
| Drag/drop import | Drop target appears only while dragging a supported image | Reduces file-picker friction without adding permanent chrome |
| Undo toast | Reversible trash shows Undo with clear expiration | Builds confidence in recoverable deletion |
| Focus ring | Accent outline remains visible on keyboard navigation | Makes keyboard interaction spatially understandable |
| Empty-state shortcut | Empty library offers Capture, Import, and Add folder with shortcut hints | Turns a blank state into a first success path |

## Accessibility and Motion Acceptance

Before calling the design polished, verify:

- keyboard-only creation, editing, capture cancellation, search, detail, attach, Settings, trash, undo/recovery;
- visible focus in dark/light themes and compact/full modes;
- screen-reader labels for menus, toggles, tiles, OCR lines, status changes, and dialogs;
- text scaling at 125%, 150%, and a compact-width window;
- IME, Unicode selection, editor undo/redo, and clipboard actions;
- no color-only meaning for OCR/exclusion/save state;
- reduced-motion preference removes or shortens the transitions above;
- exclusion transition shows no content-bearing frame before treatment is applied;
- capture failure and OCR failure leave a useful original/retry path.

The existing `tests/gui_smoke.py` is a valuable interaction smoke test, but its fixed coordinates and X11-only environment do not replace this acceptance set.

## UX Priorities

### Fix first

1. Explain first launch and separate OCR setup from note availability.
2. Make capture destination, attachment, and OCR state visibly continuous.
3. Correlate detail actions to the selected screenshot ID and show loading state.
4. Distinguish saved, pending, failed, unavailable, and exclusion transitions.
5. Validate keyboard focus, accessible names, contrast, scaling, and reduced motion.

### Improve next

1. Add result type/snippets/provenance.
2. Improve compact mode for one-hand/keyboard use.
3. Add drag/drop and repeated capture only after measuring demand.
4. Give source folders health and rescan controls.
5. Make links between notes and screenshots a primary navigation pattern.

### Defer

Tags, semantic search, generated summaries, animated visual timelines, and broad design effects should wait until the core path is both reliable and loved.

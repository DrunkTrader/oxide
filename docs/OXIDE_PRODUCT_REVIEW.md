# Oxide - Product Review

## Product Thesis

Oxide should be a **fast local visual-memory workspace**: the place a person keeps the fleeting screen state that matters, adds just enough written context, and retrieves it later by what was visible rather than by what the file was called.

That thesis is supported by the repository:

- `README.md` calls the product a local workspace for notes and searchable screenshots.
- `crates/core/src/store.rs` stores notes and stable screenshot/attachment relationships together.
- `crates/ocr/src/lib.rs` recognizes text locally and returns line geometry.
- `crates/app/src/app/notes.rs` makes screenshots part of note editing rather than a separate import-only library.
- `crates/app/src/app/screenshots.rs` exposes original image detail, OCR lines, copy, insert, attach, and retry.
- `crates/app/src/lifecycle.rs` supports resident summon/hide, which fits a tool used alongside another active application.

The product is not yet a general knowledge base. It is strongest when the user has visual evidence first and needs context and retrieval second.

## What Problem Does Oxide Solve?

Screenshots are useful because they preserve a visual state, but they are poor memory objects:

- filenames are usually weak search keys;
- a screenshot rarely contains the context of why it mattered;
- copying the visible text loses visual evidence and layout;
- switching to a cloud note or upload service may be unacceptable for sensitive work;
- a person presenting or sharing their screen may need a private scratchpad available at the same time.

Oxide connects the evidence and the explanation:

```text
Visual moment -> local original -> OCR text -> note context -> remembered phrase -> original evidence
```

The user value is highest when retrieval is based on an imperfect memory such as an error fragment, command, person, heading, or distinctive sentence. `Query::parse` and `Store` support literal terms, punctuation, cross-line terms, date boundaries, and source folder filters for this job.

## Target Users

These personas are product hypotheses. The repository has no user interviews or production analytics, so they should be validated with usability sessions rather than treated as market facts.

### Developer or technical operator

**Situation:** An error, terminal command, dashboard state, deployment result, or configuration is visible for a short time.

**Desired path:** summon, capture, add a one-line explanation, continue working, later search “connection refused” or a service name, open the original, copy the relevant lines.

**Why Oxide fits:** line-level OCR, literal punctuation handling, local storage, note attachment, copy/insert actions, and capture shortcuts.

### Presenter or meeting participant

**Situation:** The screen is being shared or recorded while private working notes and references are needed.

**Desired path:** summon a compact workspace, type, capture evidence, hide quickly, and keep the workspace out of a validated recording path when the user enables exclusion.

**Why Oxide fits:** resident lifecycle, compact mode, global hide/capture shortcuts, and the Windows affinity feature.

**Important qualification:** this is a high-value job only on configurations with recorded-output evidence. `docs/COMPATIBILITY.md` currently marks every Windows OBS/Zoom/Teams/Meet profile unverified.

### Researcher or knowledge worker

**Situation:** A folder contains screenshots of documents, references, receipts, conversations, or visual research.

**Desired path:** import/watch a folder, search remembered words, inspect the original, annotate the meaning in a note, export a portable note when needed.

**Why Oxide fits:** watched folders, imported images, OCR state, folder/date filters, stable attachments, Markdown export.

## Core Workflow Review

### 1. Open Oxide

**Current experience:** A resident native app opens with a hidden-first window lifecycle. On first run, config is created and Settings is shown. The shell provides a search field, workspace rail, notes sidebar, editor/library, and status bar.

**Good:** The purpose is stated in empty states, the app is local by default, and a new note can be created without OCR setup.

**Friction:** The first-run state looks like a full settings task rather than a short path to first value. The privacy state, worker state, save state, and local-only message are all visible but compete for attention.

**Recommended product behavior:** A first launch should guide the user through only the minimum choices: capture location, optional screenshot roots, and whether the platform's exclusion capability is enabled. Explain that notes work before OCR models are ready. Move advanced OCR and maintenance choices out of the first-use path.

### 2. Capture a screenshot

**Current experience:** Choose a display, select display or region, hide Oxide, capture via X11/Windows, and save a collision-safe PNG. Region selection can be made by drag or exact coordinate fields. The note active when capture starts is the attachment destination.

**Good:** The capture target is frozen; the own UI is omitted in tested X11 flow; failure leaves the app recoverable; originals are atomically published.

**Friction:** The full-display preview and selection screen are explicit but visually disconnected from the note. The user does not immediately see a thumbnail entering the note after Save. No window-target capture or repeated-region mode exists.

**Recommended product behavior:** Treat capture as a short transaction with three visible milestones: “capturing,” “saved to note,” and “OCR pending.” Show a small thumbnail in the attachment rail immediately. Keep the full selector for deliberate region work, but add a fast path for repeated display/region capture later.

### 3. Review the screenshot

**Current experience:** The attachment strip and screenshot library display an image, availability, and OCR state. Detail opens a generic window with metadata, full image, OCR boxes, linked notes, and actions.

**Good:** Original availability is separate from OCR availability; a no-text image remains useful; line selection can be made on the image or checkboxes.

**Friction:** Detail lacks a strong relationship to the note and query that led there. The state before a new snapshot arrives can be ambiguous. Metadata and actions have similar visual weight.

**Recommended product behavior:** The detail view should open as a focused continuation of the selected tile, retain the search phrase and source note context, and place “copy,” “insert,” and “attach” before maintenance actions.

### 4. Add or edit notes

**Current experience:** Title and body are plain multiline egui fields. Markdown syntax is accepted but not rendered. Autosave starts after approximately 500 ms; status says saving, saved, unsaved, or failed. A failed buffer can be retried or exported.

**Good:** Plain text is fast, testable, and compatible with IME/editor semantics. The save acknowledgement is meaningful because it maps to a durable SQLite transaction.

**Friction:** “Saved locally” is small and the relationship between note and screenshot is visually secondary. A person may not know that a capture is attached to the note active at capture start.

**Recommended product behavior:** Add a compact save indicator near the title, a stronger attachment insertion confirmation, and an optional rendered preview only if users ask for it. Do not replace the simple editor with a large rich-text surface as the next step.

### 5. Organize information

**Current experience:** Notes have a sidebar, source folders become sidebar collections, screenshots have All/Notes/Screenshots views, and note/screenshot trash exist. There are no user-created tags, collections, saved searches, backlinks, or history.

**Good:** The product does not force heavy organization before capture. Note/screenshot ownership is distinct.

**Friction:** The word “Collections” currently represents watched folders, not user-created semantic collections. Recent items are drawn from the loaded note list rather than a true recent-history query. A user cannot create a durable scope such as “release incident” without writing it in a note.

**Recommended product behavior:** First improve retrieval context and attachment navigation. Add tags or collections only after observing that folders and notes do not cover repeated retrieval patterns.

### 6. Search

**Current experience:** `Ctrl/Cmd+F` and `Ctrl/Cmd+K` focus the search field. Search targets Notes, Screenshots, or All views. All query terms must occur in one item. Date and folder filters can be entered directly or through a menu. Results load in prefixes with “Load more.”

**Good:** Literal punctuation, underscores, percent signs, FTS keywords, and across-line screenshot terms are handled deliberately. Search remains local and does not invoke OCR.

**Friction:** The user sees a list but not always the match reason. A screenshot result can show a highlighted line, but All results do not yet provide a consistent match excerpt. Search failures and empty results need stronger differentiation.

**Recommended product behavior:** Show result type, matched text snippet, source folder/time provenance, and keyboard focus. Preserve exact/literal semantics. If approximate or semantic matching is added, label it separately and keep exact hits first.

### 7. Reuse and share

**Current experience:** Users can copy all or selected OCR lines, insert selected text into an active note, attach a screenshot, open externally, reveal the folder, or export a note with copied images and relative Markdown links.

**Good:** The output actions are reversible or explicit; external viewing is labeled by action rather than hidden automation.

**Friction:** Copy success has little feedback, export is folder-based rather than previewed, and there is no shareable package manifest or export verification.

**Recommended product behavior:** Add subtle copy/save confirmation, export preflight, and a portable bundle check. Avoid automatic cloud sharing.

## Feature Inventory by Product Category

### Core

- Local note creation/edit/autosave.
- Display and region capture on supported platforms.
- Image import and explicit clipboard paste.
- Stable screenshot registration and note attachments.
- Local OCR with line boxes.
- Literal note/screenshot search.
- Original-image detail and text extraction.

### Important

- Watched screenshot folders and background reconciliation.
- Date/folder filters and incremental loading.
- Resident summon/hide, hide-all, configurable global shortcuts.
- Recording exclusion on supported Windows configurations.
- Note and screenshot trash behavior.
- Markdown export and database backup.
- OCR retry/reindex, worker pause/status, theme, startup, always-on-top.

### Nice-to-have

- Better capture feedback and attachment navigation.
- Drag-and-drop import.
- Saved searches or user collections.
- Search snippets and match explanation.
- Per-image processing history.
- In-app screenshot trash undo where platform support is real.

### Experimental or future

- Approximate OCR matches and look-alike detection.
- Screenshot burst grouping.
- Tags, templates, note history, backlinks.
- Window-target and repeated-region capture.
- Automatic titles or tags.
- Semantic similarity.
- Local assistant or generated summaries.

### Incomplete or platform-limited

- Windows recorder/meeting compatibility evidence.
- Linux Wayland and macOS capture.
- Cross-platform recording exclusion.
- Screen-reader/focus/reduced-motion validation.
- OCR confidence values.

## Gyotaku as a Product Reference

The 37 supplied Gyotaku reports describe a more mature screenshot search utility. Its “fish printing” metaphor, neutral chrome, spatial line emphasis, keyboard-first retrieval, justified image grid, native resident lifecycle, and separate OCR reader are useful reference ideas. The reports also identify limitations that matter to Oxide: count-driven UI refresh, disposable identity, destructive unknown-schema rebuilds, a separate website whose demos simplify native search semantics, and platform-specific trash/clipboard gaps.

### Transferable lessons

1. **Search should preserve the visual object.** The result is an image with spatial evidence, not an OCR text record detached from its source.
2. **Local OCR belongs outside the interactive window.** Oxide correctly preserves this boundary in `oxide-ocr` and `oxide-worker`.
3. **Search semantics should be literal by default.** Error codes and identifiers are harmed by overly broad correction.
4. **Native lifecycle matters.** Resident summon and direct local filesystem access are central to the workflow.
5. **Derived caches must be disposable.** Oxide improves on the reference by protecting notes and attachments as canonical data.

### Deliberate differences

1. Oxide captures its own originals and attaches them to authored notes; Gyotaku reads existing screenshots and does not implement screen capture.
2. Oxide's stable screenshot identity is required for note relationships; Gyotaku can replace an indexed row during re-read.
3. Oxide must validate recording exclusion against actual output; Gyotaku has no equivalent private-workspace contract.
4. Oxide should use an application revision and explicit invalidation; Gyotaku's count-based refresh weakness is unacceptable when note edits and OCR replacement can preserve item counts.
5. Oxide should present OCR pending/failed/no-text/missing states as product states because a capture can be useful before it is searchable.

## Signature Feature Candidates

| Candidate | Fit with current code | Differentiation | Risk | Verdict |
|---|---|---:|---:|---|
| Extremely fast capture-to-note | Strong: capture, active-note freeze, atomic save, attachments exist | High | Medium | Build and polish now |
| Search by visible text | Strong: local OCR/FTS/line boxes exist | High | Medium | Core signature; improve quality and explanation |
| Private workspace while sharing | Mechanism exists on Windows, evidence absent | Very high | Very high | Make a validated platform feature, never a universal claim |
| Local-first visual history | Strong foundation: stable originals, OCR, notes, timestamps | High | Medium | Build through recent context/timeline before semantic AI |
| Screenshot-to-note evidence graph | Strong: `note_screenshots`, linked notes, insert/attach | High | Medium | Make navigation and backlinks more discoverable |
| Intelligent auto-tagging | No current domain model or test evidence | Medium | High | Defer until retrieval data shows a need |
| Semantic “that error last week” search | Date parser exists; semantic ranking does not | Medium | High | Start with structured date/type terms, measure before embeddings |
| Background invisible capture | Current product has manual capture and privacy boundaries | High but intrusive | Very high | Avoid; risks privacy, storage, and user trust |
| Developer context capture | Fits screenshots, notes, clipboard, source folders | Medium-High | Medium | Explore as focused workflows, not a generic recorder |

The strongest signature is a sequence, not a single gimmick:

```text
Capture in one gesture
    -> attach without thinking
    -> search by what was visible
    -> return to the original and its explanation
```

If Windows exclusion passes the real compatibility matrix, private working context becomes a second memorable promise. Until then, the local visual-memory workflow is the reliable product center.

## Product Opportunities

### Screenshot intelligence

- OCR queue states and expected freshness.
- Search snippets with line-level provenance.
- Duplicate and near-duplicate warnings without merging identity.
- Burst grouping only when repeated captures actually create retrieval noise.
- Smart titles based on user confirmation, not silent replacement of authored titles.
- Source application and display metadata only if capture APIs can provide it reliably.

### Notes

- One-keystroke quick note from the resident panel.
- Screenshot-attached note sections with backlinks in both directions.
- Pinned notes and note templates for incident/meeting/research contexts.
- Revision history only if users need recovery beyond database backup; it is not a default requirement.
- Explicit “insert OCR” command, preserving authored text exactly as current code does.

### Search

- Type filters and filter chips rather than query-only discoverability.
- Human-readable time and provenance: “captured,” “imported from file time,” or “legacy.”
- Exact, near, and semantic result labels with separate controls.
- Recent-first visual history for empty search.
- Match navigation in detail and return to previous query/scroll position.

### Developer workflow

These fit if kept narrow:

- capture a terminal region and attach it to an incident note;
- paste an error image explicitly;
- insert recognized command/error lines into a note;
- export a self-contained Markdown evidence bundle;
- search a known error across screenshots and notes.

These do not fit the current product center without a separate decision: full screen recording, meeting transcription, issue tracker replacement, cloud collaboration, or a generic project-management system.

## Things Oxide Should Not Build

1. **A cloud screenshot backend.** It weakens the local/privacy value and introduces account, upload, storage, and deletion obligations before the local workflow is fully polished.
2. **A generic productivity suite.** Tasks, calendars, chat, whiteboards, and project management dilute the visual evidence job.
3. **Always-on invisible screen capture.** It increases privacy risk, disk growth, consent complexity, and platform compatibility burden. Manual capture is the clearer trust model.
4. **An LLM chat panel by default.** Generated answers do not solve the first retrieval problem and can make local data handling harder to explain.
5. **Semantic embeddings before exact search is excellent.** Approximate retrieval can be useful, but it must not obscure literal errors, codes, or provenance.
6. **A rich-text editor rewrite as a proxy for polish.** The current plain editor is fast and appropriate. Improve context and states first.
7. **Automatic destructive organization.** Silent moves, merges, or screenshot deletion undermine original preservation and stable attachments.
8. **A local HTTP API for internal components.** The current request channel and SQLite boundaries are simpler and private; an HTTP port would add attack surface.
9. **Infinite animation and decorative glass layers.** They consume attention and contradict a calm technical workspace.
10. **Broad platform promises without evidence.** A binary that compiles on Linux/macOS/Windows is not proof of capture, exclusion, accessibility, or startup parity on each platform.

## Ideal Oxide Experience

```text
Capture
    -> Understand
    -> Organize lightly
    -> Find
    -> Reuse
```

### Capture

The user summons Oxide, presses one shortcut, and the original is saved without including Oxide's own UI. A small protected confirmation says where it went and which note owns it. If no note is active, the capture remains a library item without pretending it has context.

### Understand

The screenshot appears immediately. OCR runs in the background and reports pending/ready/no-text/failed honestly. The user can write context without waiting for models. Recognized lines can be copied or inserted explicitly; they never silently replace the note.

### Organize lightly

Attachments, source folders, timestamps, and optional tags provide enough structure. The user should not need to select a collection before capture. Note and screenshot identity remain independent so one screenshot can support several notes.

### Find

The user remembers a phrase, code, date, or folder. Search shows Notes and Screenshots as distinct result types, exact literal matches first, line-level visual highlights, provenance, and a clear way back to the note. Empty search is a recent visual history, not a blank database table.

### Reuse

Copy text, insert selected lines, attach to another note, open the original, and export a portable Markdown bundle are immediate. Every action communicates success. Deletion is recoverable or explicit; no hidden cleanup breaks context.

The application stays out of the way by being resident, keyboard-first, local, and quiet. It is always ready, but it does not become the user's entire desktop.

## Product Success Measures

The repository already suggests useful acceptance goals in `PRD.md`; translate them into a small product scorecard:

- A new user can create a note, capture/attach an image, and retrieve it by a visible phrase without assistance.
- Acknowledged note saves have zero loss in crash/restart testing.
- A capture is viewable before OCR is ready and remains attached through OCR replacement and source rename.
- Exact literal queries work with punctuation, short identifiers, cross-line terms, date filters, and folder filters.
- On every declared Windows privacy profile, no protected content or opaque placeholder appears in receiver/recorded output when exclusion is On; normal capture returns when it is Off.
- Search, typing, and note editing remain responsive during backfill and fresh OCR.
- Keyboard navigation, focus visibility, status semantics, text scaling, IME, screen reader behavior, contrast, and reduced motion pass an actual supported-platform check.

These measures are more valuable than feature count, download count, or a generic AI demo.

# Oxide - Feature Roadmap and Prioritization

## Roadmap Principles

This roadmap is for a solo developer inheriting an already working development build. It is intentionally staged:

```text
Release evidence and correctness
    -> core workflow polish
    -> retrieval quality and organization
    -> power-user expansion
    -> carefully validated differentiation
```

The roadmap treats `PRD.md` and `docs/IMPLEMENTATION_STATUS.md` as the current acceptance baseline. It does not turn every idea in the product review into a commitment.

Scores use a 1-5 scale:

- **User Value:** how directly the idea improves the main capture/find/reuse job.
- **Impact:** breadth and frequency of benefit.
- **Effort:** 1 is small, 5 is large.
- **Risk:** 1 is low, 5 is high.
- **Priority:** P0 release/blocker, P1 high value, P2 expansion, P3 experimental.

## Priority Table

| Feature | User Value | Impact | Effort | Risk | Priority |
|---|---:|---:|---:|---:|---|
| Windows recorder/meeting exclusion evidence matrix | 5 | 5 | 5 | 5 | P0 |
| Detail-state ID correlation and loading guard | 5 | 4 | 2 | 2 | P0 |
| Accessibility and reduced-motion acceptance | 5 | 5 | 3 | 3 | P0 |
| Capture/storage/OCR failure injection and reconciliation | 5 | 5 | 4 | 4 | P0 |
| Installer/offline/bootstrap/data-preservation validation | 5 | 4 | 4 | 4 | P0 |
| Typed worker/OCR/save/exclusion status presentation | 4 | 5 | 3 | 2 | P1 |
| Capture-to-note confirmation and OCR lifecycle UI | 5 | 5 | 2 | 1 | P1 |
| Search result snippets, provenance, and type labels | 5 | 5 | 3 | 2 | P1 |
| Queue latency and texture/memory instrumentation | 4 | 4 | 3 | 2 | P1 |
| Drag/drop import and explicit source health | 4 | 4 | 3 | 2 | P1 |
| Two-way note/screenshot navigation | 4 | 4 | 2 | 1 | P1 |
| In-app screenshot trash undo where supported | 3 | 3 | 3 | 4 | P1 |
| Exact search continuation cursors | 3 | 3 | 3 | 2 | P2 |
| Repeated-region/window-target capture | 4 | 3 | 4 | 4 | P2 |
| Tags, saved collections, templates | 3 | 4 | 4 | 3 | P2 |
| Screenshot burst grouping | 3 | 3 | 4 | 4 | P2 |
| Bounded OCR near matches | 3 | 3 | 4 | 4 | P2 |
| Portable workspace bundle | 3 | 3 | 4 | 3 | P2 |
| OCR quality corpus and confidence decision | 4 | 4 | 3 | 3 | P2 |
| Semantic local search | 3 | 3 | 5 | 5 | P3 |
| Smart titles/auto-tagging | 3 | 3 | 4 | 4 | P3 |
| Local assistant/summaries | 2 | 2 | 5 | 5 | P3 |
| Continuous background screen capture | 2 | 2 | 5 | 5 | P3 |
| Cloud sync/shared workspaces | 2 | 3 | 5 | 5 | P3 |

## P0 - Fix Now: Release and Correctness

### 1. Validate recording exclusion on the declared Windows matrix

- **Problem:** The product's strongest differentiator is not validated against actual receiver or recorded output.
- **User benefit:** Users can make a trustworthy decision about private working context while sharing.
- **Implementation complexity:** Very high operational effort; the application API call already exists.
- **Technical risk:** Very high because renderer, capture backend, client version, DWM, monitor/DPI, and lifecycle can differ.
- **Why it fits:** It is the reason to choose Oxide for presenters and meeting participants.
- **Exit evidence:** OBS display/window capture backends; Zoom; Teams; Meet; On/Off transitions; moving background; menus/detail/selection; restart; suspend/resume; mixed DPI; saved receiver/recording artifacts. Update `docs/COMPATIBILITY.md` per exact profile.

### 2. Fix selected-detail data correlation

- **Problem:** `App::select_shot` changes detail ID and requests a snapshot, but `lines` and `linked` can still describe the previous item until the response arrives.
- **User benefit:** Copy, insert, attach, and metadata always apply to the screenshot the user opened.
- **Implementation complexity:** Small: carry a detail snapshot ID and disable actions while mismatched.
- **Technical risk:** Low; impacts snapshot rendering and detail action guards.
- **Why it fits:** Detail is the bridge between search, evidence, and note reuse.
- **Exit evidence:** Rapidly open different screenshots, keyboard navigation, slow backend response, no stale line copy or attachment.

### 3. Execute accessibility and reduced-motion checks

- **Problem:** AccessKit is enabled, but source inspection and X11 smoke do not prove semantic accessibility.
- **User benefit:** Keyboard-first workflows work for more users and remain reliable at larger text sizes.
- **Implementation complexity:** Medium: toolkit labels/focus improvements plus manual/platform checks.
- **Technical risk:** Medium because custom composition can expose toolkit gaps.
- **Why it fits:** Keyboard operation is a core product promise, not an optional polish item.
- **Exit evidence:** Keyboard-only workflow, screen reader names/status, focus visibility, light/dark contrast, text scaling, IME, reduced motion.

### 4. Add failure injection for capture, storage, OCR, and sources

- **Problem:** Cross-resource actions can leave an original, database row, attachment, or thumbnail at different stages.
- **User benefit:** Failures are recoverable and do not create false saved/captured states.
- **Implementation complexity:** Medium-high: disposable filesystem/SQLite tests and reconciliation fixtures.
- **Technical risk:** High around process interruption and native trash behavior.
- **Why it fits:** Local user data needs stronger durability than a disposable OCR cache.
- **Exit evidence:** Permission denial, disk full, database busy/corrupt, worker stop during capture/OCR, missing source, partial native trash, restart reconciliation.

### 5. Validate packaging, offline bootstrap, update, and data preservation

- **Problem:** CI builds/install definitions exist, but current evidence is incomplete for the actual Windows release path and model availability.
- **User benefit:** A user can install, operate offline after provisioning, update, and uninstall without losing notes/originals.
- **Implementation complexity:** High release/test-lab effort.
- **Technical risk:** High due to native runtime/model packaging and startup registration.
- **Why it fits:** Reliability at install is part of product quality.
- **Exit evidence:** Clean install, model setup, offline run, migration failure, downgrade refusal, update, uninstall, data preservation, resident worker sibling binaries.

## P1 - High Value: Make the Existing Product Feel Finished

### 6. Make capture-to-note continuity visible

- **Problem:** Capture is saved and attached correctly, but the user receives a generic notice and waits for OCR without a strong visual handoff.
- **User benefit:** Immediate confidence that evidence went to the intended note.
- **Implementation complexity:** Small-medium UI work in `crates/app/src/app.rs`, `crates/app/src/app/notes.rs`, `crates/app/src/app/screenshots.rs`, and design tokens.
- **Technical risk:** Low if confirmation remains inside the protected native window.
- **Why it fits:** It amplifies the current stable attachment contract.
- **Design:** Thumbnail enters the attachment rail, shows “Saved locally / OCR pending,” then resolves to text/no text/failed.

### 7. Model statuses as explicit user-facing states

- **Problem:** `ocr_state`, `WorkerStatus.state`, privacy error, save error, and notices mix strings and generic text.
- **User benefit:** Users understand whether they can continue writing, why search is incomplete, and what to do next.
- **Implementation complexity:** Medium: enums internally, mapping at UI boundary, migration-safe strings.
- **Technical risk:** Low-medium; status changes touch worker and app contracts.
- **Why it fits:** Trust is a product feature for local processing and privacy.
- **Design:** Separate note save state, worker readiness, per-image OCR state, exclusion state, and database state.

### 8. Add search snippets, provenance, and result type labels

- **Problem:** Search returns useful results but does not consistently explain the match before detail opens.
- **User benefit:** Faster scanning and fewer false openings.
- **Implementation complexity:** Medium: return matched lines/snippets for visible results and render distinct note/screenshot result rows.
- **Technical risk:** Low-medium; avoid loading all OCR lines for huge result sets.
- **Why it fits:** It improves the product's core retrieval loop without changing literal grammar.
- **Design:** “Screenshot · captured today · 2 matching lines,” “Note · title/body match,” and source folder/time provenance.

### 9. Instrument queue latency and image memory

- **Problem:** Search benchmark and one OCR fixture exist, but UI queue wait, fresh/backlog OCR, summon latency, and repeated browse memory are not established.
- **User benefit:** A responsive editor and predictable capture pipeline.
- **Implementation complexity:** Medium: local timing counters and repeatable benchmark scenarios.
- **Technical risk:** Low; instrumentation stays local and opt-in if needed.
- **Why it fits:** Performance claims should be evidence-based and guide optimization.
- **Exit evidence:** Input-to-paint, backend queue wait, fresh OCR, 4K save, warm summon, worker idle CPU, repeated texture browse, two-process RSS.

### 10. Improve source health and drag/drop import

- **Problem:** Watched folder failure, paused reader, unavailable roots, and import paths are not easy to understand from the UI.
- **User benefit:** Existing screenshot libraries become dependable without terminal diagnosis.
- **Implementation complexity:** Medium.
- **Technical risk:** Medium around platform event/watcher semantics.
- **Why it fits:** Folder indexing is a core path for users who already have screenshots.
- **Design:** Per-source status, Rescan, Pause source, Unavailable, last successful reconciliation, and a drag/drop target for one-off import.

### 11. Make note/screenshot links first-class

- **Problem:** `linked_notes` and attachments exist but are visually quiet and navigation state is not always preserved.
- **User benefit:** Users can move from evidence to explanation and back without losing search context.
- **Implementation complexity:** Small-medium.
- **Technical risk:** Low.
- **Why it fits:** Stable many-to-many attachment identity is one of Oxide's strongest domain decisions.
- **Design:** Link counts, “Open linked note,” “View attached screenshots,” and return-to-query behavior.

### 12. Add in-app screenshot undo only where reliable

- **Problem:** Screenshot trash uses recoverable OS trash but has no in-app undo; native support differs by platform.
- **User benefit:** Fast recovery from an accidental selection.
- **Implementation complexity:** Medium-high.
- **Technical risk:** High because native restore and collision behavior differ.
- **Why it fits:** Existing deletion is already explicit and recoverable.
- **Constraint:** Advertise Undo only on tested platforms; never substitute permanent deletion or a fake success.

## P2 - Product Expansion

### 13. Search continuation cursors

- **Problem:** Incremental loading grows a requested result prefix and has no stable continuation token.
- **User benefit:** Large libraries remain responsive and pagination is stable during worker changes.
- **Implementation complexity:** Medium.
- **Technical risk:** Medium around query snapshots and ordering.
- **Why it fits:** It strengthens a current result path without changing user mental models.

### 14. Repeated-region and window-target capture

- **Problem:** Repeated work often captures the same area, while full display selection can be excessive.
- **User benefit:** Faster technical and presentation workflows.
- **Implementation complexity:** High and platform-specific.
- **Technical risk:** High for DPI, window movement, privacy, and exclusion behavior.
- **Why it fits:** It builds on capture without becoming a full editor.

### 15. Tags, saved collections, and note templates

- **Problem:** Source folders are not semantic organization and users may repeat note formats.
- **User benefit:** Better retrieval for ongoing projects/incidents.
- **Implementation complexity:** High-medium: schema, UI, search grammar, export semantics.
- **Technical risk:** Medium product risk of becoming a generic organizer.
- **Why it fits:** Only after real use shows notes/folders are insufficient.

### 16. Screenshot burst grouping

- **Problem:** Repeated captures can clutter results.
- **User benefit:** Faster scanning of visual history.
- **Implementation complexity:** High due to similarity heuristics and identity preservation.
- **Technical risk:** High false grouping risk.
- **Why it fits:** Gyotaku reports provide a useful reference, but Oxide should preserve every screenshot and attachment independently.

### 17. Bounded near/OCR look-alike search

- **Problem:** OCR may misread a phrase or identifier.
- **User benefit:** Find a screenshot despite predictable character confusion.
- **Implementation complexity:** High enough to require fixtures, labels, and ranking.
- **Technical risk:** High for false positives in developer errors and codes.
- **Why it fits:** Gyotaku demonstrates a conservative pattern; implement only with exact-first and raw verification.

### 18. Portable workspace bundle

- **Problem:** Database backup and original-image backup are currently separate.
- **User benefit:** Move or archive a note plus its evidence safely.
- **Implementation complexity:** High: manifest, path rewriting, collision policy, integrity, restore.
- **Technical risk:** Medium-high around duplicate originals and sensitive data.
- **Why it fits:** Markdown export proves a need for portability; a full workspace bundle should follow real demand.

### 19. OCR quality corpus and confidence decision

- **Problem:** ocrs provides no confidence in this implementation, and the product cannot claim measured multilingual accuracy from one fixture.
- **User benefit:** Honest expectations and better search quality.
- **Implementation complexity:** Medium, including fixture curation and alignment scoring.
- **Technical risk:** Medium around representative data and model licensing.
- **Why it fits:** OCR quality determines retrieval value.
- **Decision:** Either keep confidence explicitly unavailable or select an engine with a documented, calibrated contract; do not fabricate it.

## P3 - Experimental

### 20. Semantic local search

- **Problem:** Literal search cannot answer vague memory queries.
- **User benefit:** Find “that database error from last week” when exact words are forgotten.
- **Implementation complexity:** Very high: embedding model, index, storage, ranking, privacy, background cost.
- **Technical risk:** Very high for local resource use and surprising matches.
- **Why it fits:** It could become a differentiator only after exact retrieval, date filters, and snippets are excellent.
- **Guardrails:** Opt-in, local-only, exact hits first, explainable dimensions, rebuildable derived index.

### 21. Smart titles and automatic tags

- **Problem:** A screenshot often lacks a meaningful title.
- **User benefit:** Better browse scanning.
- **Implementation complexity:** High-medium.
- **Technical risk:** High because generated text can be wrong or expose sensitive content in labels.
- **Why it fits:** Only if user-confirmed and stored as derived suggestions, never silently authored truth.

### 22. Local assistant or summaries

- **Problem:** Users may want a compact explanation of a screenshot set.
- **User benefit:** Reduced manual synthesis in research or incidents.
- **Implementation complexity:** Very high.
- **Technical risk:** Very high for hallucination, model bootstrap, privacy, and product dilution.
- **Why it fits:** Weak until note/evidence retrieval has proven demand for synthesis.

### 23. Continuous background screen capture

- **Problem:** Users forget to capture important moments.
- **User benefit:** Potentially complete visual history.
- **Implementation complexity:** Very high.
- **Technical risk:** Very high for privacy, storage, consent, recorder behavior, and platform APIs.
- **Why it fits:** It conflicts with Oxide's deliberate local/private trust model and should probably remain out of scope.

### 24. Cloud sync and shared workspaces

- **Problem:** Users may want multi-device or team access.
- **User benefit:** Collaboration and availability.
- **Implementation complexity:** Very high.
- **Technical risk:** Very high for sensitive images, conflict resolution, accounts, deletion, hosting, and licensing.
- **Why it fits:** It does not fit the current local-first thesis without a new product decision.

## Recommended Solo-Developer Roadmap

### Phase 0 - Release evidence and decisions

**Objective:** establish what can honestly be released and on which platform.

**Engineering:**

- Run the Windows exclusion/receiver matrix.
- Run native failure injection and startup/restart/recovery checks.
- Decide intended software license; current Cargo metadata and `LICENSE` disagree.
- Validate Inno Setup payload, model bootstrap, offline mode, update/uninstall data preservation.

**UI:**

- Make unsupported platform capabilities explicit.
- Separate exclusion state from compatibility evidence.
- Show storage/worker/OCR errors as actionable states.

**Testing:**

- P0 PRD scenarios A1-A15 on a disposable Windows baseline.
- Keep Linux X11 evidence as development/declared capability, not Windows proof.

**Expected result:** a defensible release support matrix and no privacy promise based only on API success.

### Phase 1 - Stability and correctness

**Objective:** remove stale-state and cross-resource ambiguity.

**Engineering:**

- Correlate detail payloads with screenshot ID.
- Acknowledge destructive state changes before clearing editor/detail context.
- Add digest/version guards to OCR failures.
- Add capture intent/reconciliation where failure tests demonstrate a need.
- Add typed internal state for OCR, worker, save, exclusion, and operation errors.

**UI:**

- Make loading, pending, failed, unavailable, and saved distinct.
- Keep old results visible only with an explicit “updating” state.

**Testing:**

- Focused store, backend response-order, editor, detail, capture failure, and restart tests.

**Expected result:** the app never presents an old screenshot's actions as if they belonged to a newly selected screenshot.

### Phase 2 - Core UX polish

**Objective:** make the existing workflow fast and legible.

**Engineering:**

- Instrument backend queue latency, OCR fresh/backlog latency, texture cache, and search refresh.
- Reduce unnecessary full image decode/copy work only where measurement points to it.

**UI:**

- Capture-to-note thumbnail confirmation.
- Title-adjacent save status.
- Better primary/secondary/destructive button hierarchy.
- Search result type/snippet/provenance.
- Two-way note/screenshot navigation.
- Compact mode tuned for live use.

**Testing:**

- Keyboard/focus/accessibility execution; both themes; text scaling; reduced motion.
- Repeat the real GUI workflow against current coordinates and no-worker/worker modes.

**Expected result:** first-time users can reach the aha moment without interpreting internal state.

### Phase 3 - Library and search depth

**Objective:** improve retrieval before adding AI.

**Engineering:**

- Source health/reconciliation controls.
- Search continuation only if benchmark/corpus measurements show prefix cost.
- OCR corpus and tokenizer/script/case coverage.
- Optional bounded near match with exact-first ranking if fixtures justify it.

**UI:**

- Filter chips, result keyboard navigation, visual history, match explanation.
- Clear source folder and time provenance.

**Testing:**

- Corpus-level search/OCR accuracy, large-library search, rename/reindex/cache repair, filter edge cases.

**Expected result:** users can retrieve old evidence confidently without needing increasingly complex organization.

### Phase 4 - Power-user capture and organization

**Objective:** reduce repeated-work friction for established users.

**Engineering:**

- Repeated-region/window-target capture feasibility.
- Tags/collections/templates only after product validation.
- Portable workspace bundle only with a clear manifest and restore contract.

**UI:**

- Saved scopes, source health, quick capture presets, attachment backlinks.

**Testing:**

- Mixed DPI/window movement, identity preservation, export/import collisions, schema migration.

**Expected result:** power users can shape recurring workflows without turning Oxide into a suite.

### Phase 5 - Differentiation experiments

**Objective:** test one memorable extension at a time.

**Candidates:** semantic local search, user-confirmed smart titles, or deeper private-context workflows.

**Engineering:** local-only derived indexes, opt-in resources, explainable ranking, rebuildability.

**UI:** explicit experimental labels, exact/semantic mode distinction, resource/privacy controls.

**Testing:** usefulness studies, false-positive review, offline behavior, memory/CPU budget, privacy inspection.

**Expected result:** one validated differentiator, not an accumulation of speculative intelligence.

## Testing Strategy

### Unit tests

Prioritize pure or mostly pure boundaries:

- `Query::parse`: literal punctuation, FTS keywords, quoted folders, malformed/partial dates, local DST/day bounds.
- `Line::valid` and normalized geometry.
- `Editor`: generation acknowledgement, edits during save, failed-save retry, deferred note switch/quit.
- Config validation, atomic write, workspace descriptor, cache/original separation.
- Media size/dimension limits, collision-safe capture names, export no-clobber behavior.
- Shortcut duplicate validation and resident message bounds.

### Persistence/integration tests

- schema 1/2 -> 3 migration, backup preservation, unknown/newer schema refusal;
- note/FTS transaction consistency and revision conflicts;
- screenshot registration, invalid repair, rename/path identity, preserved-mtime replacement;
- attachment survives OCR replacement, source rename, missing original, cache reset;
- clear derived data never removes originals, notes, or relationships;
- date/folder/type search and continuation ordering.

### Screenshot lifecycle tests

Use disposable fixtures and failure injection:

- capture writes temp then publishes only a complete PNG;
- DB registration failure leaves a recoverable managed original;
- attachment failure leaves an unattached library screenshot;
- thumbnail failure leaves OCR/attachment usable and queues repair;
- worker stop at each capture stage reconciles after restart;
- own UI is absent from the saved original;
- region coordinates and mixed-DPI display bounds are correct.

### OCR tests

- Keep synthetic geometry/unit tests.
- Add a small licensed, nonprivate corpus with known lines, languages/scripts, size/rotation cases, and expected normalized boxes.
- Measure character error rate and line alignment; do not claim confidence if the engine does not supply it.
- Verify offline model loading, checksum mismatch, incomplete download, model replacement, and worker retry.

### Search tests

- Exact multi-term across lines.
- Short terms and punctuation.
- Notes versus screenshot type separation.
- Folder scope never expands to filename-only matches.
- Date boundaries under local timezone/DST.
- Exact results before any near/semantic candidates.
- Query generation prevents stale responses from replacing newer results.
- Large synthetic corpus benchmark remains separate from UI latency claims.

### Keyboard/native/UI tests

- X11 smoke for actual typing/autosave/own-capture/hide/wake/restart.
- Windows native startup and actual output matrix for exclusion.
- Accessibility execution with screen reader, focus, scaling, IME, contrast, and reduced motion.
- Native picker return focus, tray/shortcut conflicts, always-on-top, multi-monitor/mixed DPI, and shutdown.

### Performance tests

Measure on a published baseline:

- warm summon p95;
- keystroke-to-paint p95 during backfill;
- backend queue wait by operation;
- screenshot save p95 for 4K;
- fresh and backlog OCR separately;
- search p50/p95 across corpus sizes;
- idle CPU and model memory;
- repeated browse/detail/hide texture and two-process memory;
- capture allocation failures on large displays.

Avoid a large framework until one of these tests shows a specific missing boundary.

## 10 Small Improvements

| Improvement | Problem / benefit | Complexity | Risk | Why it fits |
|---|---|---|---|---|
| Add explicit “Updating results” label | Old results can remain while a snapshot is pending; makes continuity honest | Low | Low | Improves search trust without changing semantics |
| Disable detail actions until target ID matches | Prevents stale OCR copy/attach | Low | Low | Direct correctness fix |
| Show “Saved locally” beside note title | Bottom status is easy to miss | Low | Low | Reinforces durable autosave |
| Show capture destination/note in confirmation | Users need to know where evidence went | Low | Low | Strengthens existing frozen-target contract |
| Add OCR state icon plus text | Status labels are visually similar | Low | Low | Helps scanning and color-independent understanding |
| Add “Copy N lines” feedback | Copy has no visible result | Low | Low | Small, high-frequency polish |
| Separate destructive menu group | Trash currently competes with routine actions | Low | Low | Reduces accidental actions |
| Add source provenance in detail | Imported time and capture time differ | Low | Low | Existing `time_source` supports honest labeling |
| Add “Open linked note” prominence | Links exist but are quiet | Low | Low | Uses canonical attachment relation |
| Add worker “Rescan now” button near status | Source/event failures require diagnosis | Low | Low | Makes reconciliation actionable |

## 10 Medium Features

| Feature | Problem / benefit | Complexity | Risk | Why it fits |
|---|---|---|---|---|
| Typed operational state model | Generic strings obscure recovery | Medium | Low-Medium | Makes local states trustworthy |
| Capture intent/reconciliation record | Cross-file/DB failure can leave ambiguous state | Medium | Medium | Protects authored attachments/originals |
| Search snippets and result type rows | Users scan results inefficiently | Medium | Low | Improves core retrieval without AI |
| Source health panel | Watched folders can fail silently or become unavailable | Medium | Medium | Existing worker status and reconciliation support it |
| Drag/drop image import | File picker adds steps | Medium | Low | Complements explicit import without clipboard watching |
| Detail previous/next navigation | Reopening grid interrupts review | Medium | Low | Fits screenshot inspection flow |
| Save/search operation priority | Long import/export can delay interaction | Medium | Medium | Protects editor responsiveness |
| OCR corpus and quality report | Current evidence is one fixture | Medium | Medium | Supports honest model/product claims |
| Portable workspace bundle | Database backup omits originals | Medium-High | Medium-High | Extends existing Markdown export when demanded |
| Platform-specific screenshot undo | Recovery is currently external/native | Medium-High | High | Adds polish only where OS APIs are reliable |

## 10 Ambitious Ideas

| Idea | Problem / benefit | Complexity | Risk | Why it fits or does not |
|---|---|---|---|---|
| Validated private-sharing mode | Makes the central privacy workflow memorable | Very high | Very high | Fits strongly, but only per tested configuration |
| Window-target capture | Full display is excessive for many tasks | High | High | Natural extension of capture; platform feasibility first |
| Repeated-region presets | Repeated work needs fewer steps | High | Medium-High | Fits developers/presenters after basic capture is reliable |
| Local semantic search | Users remember meaning, not exact words | Very high | Very high | Potential differentiator after exact search is excellent |
| Screenshot timeline | Users remember when something happened | High | Medium | Fits local visual history; use timestamps already stored |
| User-confirmed smart titles | Filenames are weak | High | High | Could help browse but must not invent authored truth |
| Evidence graph view | Notes and screenshots form a useful context network | High | Medium | Fits stable many-to-many attachment model |
| Local OCR language packs | More users need non-Latin scripts | High | High | Fits OCR mission but requires model/quality/licensing evidence |
| Focused incident/research templates | Users repeat note structures | Medium-High | Medium | Fits only if templates remain lightweight |
| Team/cloud workspace | Share evidence across devices/people | Very high | Very high | Product expansion that conflicts with current local-first center; requires a separate decision |

## If I Could Only Improve 5 Things

1. **Prove the Windows exclusion contract.** It is the highest-value promise and the highest privacy risk if misunderstood.
2. **Fix stale detail state and cross-resource recovery paths.** Users must never copy or attach the wrong screenshot, and failures must preserve evidence.
3. **Make save, capture, OCR, worker, and privacy state unmistakable.** Trust is the product's foundation.
4. **Polish the capture-to-note and search-to-detail loops.** The core aha should happen quickly and explain itself.
5. **Run accessibility, performance, installer, and failure acceptance on the launch baseline.** A loved local tool is dependable under real conditions, not only in unit tests.

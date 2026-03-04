# Reference Tool Implementation Plan (Rust + Tauri)

## 1. Project Goal

Build a lightweight, out-of-the-box desktop reference-citation tool that:

- Imports `.bib` entries and stores them locally.
- Resolves one or more citation keys entered by users.
- Returns compressed citation index output (for example, `[1]-[3], [5]`).
- Maintains a full ordered reference list based on first citation order.
- Provides a clear GUI using Tauri.

This plan covers architecture, data models, feature behavior, implementation phases, validation criteria, and git execution rules.

## 2. Scope and Constraints

### 2.1 In Scope

- Rust backend with Tauri command bridge.
- Frontend GUI for import, citation input, citation output, and key list browsing.
- Local persistence for imported library and citation order.
- Import deduplication by key with overwrite tracking.
- Default reference output format with extension point for future formats.

### 2.2 Out of Scope (Current Version)

- Cloud sync.
- Multi-user collaboration.
- Citation style switching UI (only one default style is implemented now).
- Advanced bib semantic normalization beyond current parser strategy.

### 2.3 Non-Functional Constraints

- Lightweight startup and runtime.
- No external database requirement.
- Works after install with no additional setup.
- Rust code quality with strict error handling and tests for key logic.

## 3. Functional Requirements Mapping

### 3.1 Layout Mapping

GUI layout must match the required structure:

1. Top-left area:
   - Citation input box (supports multiple keys).
   - Citation output box (read-only).
   - `Cite` button in the lower-right of this area.
   - Copy button for citation output.
2. Bottom-left area:
   - Cited references panel, read-only, scrollable.
   - Copy button for full cited reference text.
3. Entire right side:
   - Imported key list (one key per line, scrollable).
   - `Import .bib` button.

### 3.2 Import Mapping

- Import `.bib` file into local storage.
- Use entry key as unique identifier.
- If key is new: insert and count as `new`.
- If key exists: overwrite and count as `overwritten`.
- Return summary: total, imported, new, overwritten, failed.

### 3.3 Citation Mapping

Input example:

`10495806,10648348,10980318,10807485`

Rules:

1. Parse multiple keys from comma/whitespace/newline separated input.
2. Transaction behavior: if any key is missing in library, reject whole operation.
3. For each key:
   - If already cited, reuse existing index.
   - If not cited, assign next index and append reference text source.
4. Compress consecutive indexes:
   - `[1][2][3]` -> `[1]-[3]`
   - `[1][2][3][5]` -> `[1]-[3], [5]`
5. Put final string into citation output box.

### 3.4 Default Output Format Mapping

Default formatter output target:

`Author. Title[J]. Journal, Year, Volume(Number): Pages. DOI: ...`

Current release only includes this format, while code keeps extension space.

## 4. System Architecture

## 4.1 High-Level Components

- Frontend (Tauri webview):
  - Layout and user interaction.
  - File picker for `.bib` import.
  - Clipboard copy actions.
  - Calls Rust commands.
- Backend (Rust in `src-tauri`):
  - Bib parser.
  - In-memory state.
  - Persistence layer.
  - Citation engine.
  - Formatter engine.

### 4.2 Module Plan (Rust)

- `models.rs`
  - Core data structures.
- `storage.rs`
  - Load/save state from app data JSON.
- `bib_parser.rs`
  - Parse bib content into `LibraryEntry`.
- `formatter.rs`
  - `ReferenceFormatter` trait and `DefaultFormatterV1` implementation.
- `citation_engine.rs`
  - Index allocation and range compression.
- `commands.rs`
  - Tauri `#[tauri::command]` APIs.

### 4.3 Frontend Structure Plan

- `App.tsx`
  - Page layout orchestration.
- Components:
  - `CitationInputPanel`
  - `CitationOutputPanel`
  - `CitedReferencesPanel`
  - `ImportedKeysPanel`
  - `ImportButton`
- Shared state:
  - Current library key list.
  - Current cited references text.
  - Current citation output text.
  - Status and error messages.

## 5. Data Model Design

### 5.1 Core Models

- `LibraryEntry`
  - `key: String`
  - `entry_type: String`
  - `fields: BTreeMap<String, String>`
  - `raw: Option<String>`

- `PersistedState`
  - `version: u32`
  - `entries: BTreeMap<String, LibraryEntry>`
  - `citation_order: Vec<String>`

- `ImportResult`
  - `total: usize`
  - `imported: usize`
  - `new_count: usize`
  - `overwritten_count: usize`
  - `failed: usize`
  - `message: String`

- `CiteResult`
  - `citation_text: String`
  - `cited_references_text: String`
  - `newly_added_count: usize`

### 5.2 Runtime State

- `AppState`
  - Wrapped in `RwLock<AppState>` inside Tauri managed state.
  - Contains current loaded `PersistedState` and formatter config.

## 6. Core Workflow Details

### 6.1 Startup Workflow

1. Determine app data directory.
2. Load JSON state file if exists.
3. If missing, initialize empty state.
4. Expose snapshot command to frontend for initial render.

### 6.2 Import Workflow

1. Frontend opens file picker for `.bib`.
2. Frontend passes selected path to Rust command.
3. Rust reads content and parses bib entries.
4. For each parsed entry:
   - Upsert by key.
   - Count new vs overwritten.
5. Save state file.
6. Return import summary + refreshed key list.

### 6.3 Citation Workflow (Transactional)

1. Parse raw input into normalized key vector.
2. Validate all keys exist in `entries`.
3. If missing keys exist:
   - Return error with missing key list.
   - Do not mutate `citation_order`.
4. Build citation indexes:
   - Existing keys: find existing index.
   - New keys: append to `citation_order` and assign next index.
5. Compress indexes into target citation text.
6. Build full cited references text from `citation_order`.
7. Save state and return `CiteResult`.

### 6.4 Range Compression Rules

Algorithm input: integer list of indexes.

Steps:

1. Sort ascending.
2. Remove duplicates.
3. Group consecutive numbers.
4. Emit:
   - Single item group: `[n]`
   - Multi item group: `[start]-[end]`
5. Join with `, `.

## 7. Formatting Strategy

### 7.1 Trait-Based Design

Define trait:

- `format_entry(&LibraryEntry) -> String`

Implement:

- `DefaultFormatterV1`

Future styles can be added by adding new implementations and selecting by enum/config.

### 7.2 Field Handling

- Required preference order:
  - Authors -> Title -> Venue -> Year -> Volume/Issue -> Pages -> DOI.
- Missing fields:
  - Skip gracefully.
  - Keep output valid and readable.
- Author parsing:
  - Support `and` list.
  - Handle both `Last, First` and plain names.

## 8. UI and Visual Design Plan

Design direction (using frontend design guidance):

- Workspace-like academic desk style.
- Strong typographic hierarchy for long text readability.
- Distinct section cards for left-top, left-bottom, and right panel.
- Controlled accent color and contrast (avoid generic default patterns).
- Subtle motion for panel reveal and action feedback.

Interaction principles:

- Read-only text areas for output and cited list.
- One-click copy actions with visible confirmation.
- Clear error and success messaging.
- Scroll behavior confined to long-content panels.

Responsive behavior:

- Desktop first split layout.
- Narrow width fallback into stacked sections while keeping action order intact.

## 9. Error Handling and Messaging

Error categories:

- File read/open error.
- Bib parse error.
- Missing citation key error.
- Persistence write error.
- Unexpected internal error.

Message policy:

- User-facing errors are concise and actionable.
- Missing key errors include exact key list.
- Import result always includes counts.

## 10. Testing and Quality Plan

### 10.1 Unit Tests (Rust)

- Parse key list normalization.
- Citation index allocation and reuse.
- Range compression.
- Formatter behavior with full and partial fields.
- Import upsert counts.

### 10.2 Integration Checks

- Import then cite then restart then cite again (persistence check).
- Mixed existing/new keys in one citation request.
- Transaction rollback behavior on missing key.

### 10.3 Build Checks

- `cargo check`
- `cargo test`
- Frontend build command (`npm run build`)

## 11. Implementation Phases and Git Steps

Each phase ends with:

1. `git add -A`
2. `git commit -m "<phase message>"`
3. `git push origin main`

Planned phases:

- Step 0: Create this implementation plan document.
  - Commit: `docs: add detailed implementation plan for tauri reference manager`
- Step 1: Scaffold Tauri app and baseline frontend layout.
  - Commit: `chore: scaffold tauri app and baseline frontend layout`
- Step 2: Add models and persistence layer.
  - Commit: `feat(core): add persistent models and storage layer`
- Step 3: Implement bib import parser and upsert logic.
  - Commit: `feat(import): implement bib import with dedup and overwrite metrics`
- Step 4: Implement formatter and citation range compression with tests.
  - Commit: `feat(format): add default reference formatter and citation range compression`
- Step 5: Implement transactional cite workflow command.
  - Commit: `feat(citation): implement multi-key citation transaction workflow`
- Step 6: Wire full UI interactions (import, cite, copy, scroll, readonly).
  - Commit: `feat(ui): wire complete citation workspace interactions`
- Step 7: Stabilize with tests/build validation.
  - Commit: `chore: stabilize app with tests and build verification`
- Step 8: Add user guide documentation.
  - Commit: `docs: add user guide for import and citation workflow`

## 12. Definition of Done (Per Release Scope)

Release is accepted when all are true:

- `.bib` import works with new/overwrite counts.
- Right panel lists all imported keys and is scrollable.
- Multi-key cite input works with transaction behavior.
- Missing keys return clear error and no partial state mutation.
- Existing cited keys reuse index; new keys append indexes.
- Citation output uses compressed ranges.
- Cited reference list shows numbered formatted entries.
- Copy actions for output and cited list both work.
- State persists across restart.
- Tests and build checks pass.

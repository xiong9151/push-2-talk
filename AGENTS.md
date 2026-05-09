# Repository Guidelines

## Project Structure & Module Organization
- `src/` holds the React + TypeScript UI (main window, overlay, and notification windows).
- `index.html`, `overlay.html`, and `notification.html` are the Vite entry points for the three windows.
- `src-tauri/` contains the Rust backend, `src-tauri/tauri.conf.json`, and app icons in `src-tauri/icons/`.
- `ui/` is for auxiliary UI assets or prototypes used by the frontend.
- `scripts/` has build/release helpers; `dist/` is generated frontend output.
- `tests/` contains TypeScript runtime regression tests (`tsx --test`).

### Key Backend Modules (src-tauri/src/)
- `asr/` - Multi-provider ASR with HTTP and realtime modes (Qwen, Doubao, Doubao IME, SenseVoice)
- `pipeline/` - Processing pipelines for dictation and assistant modes
- `tnl/` - Technical Normalization Layer between ASR and LLM (pinyin/phonetic matching, letter merge, hyphen rewrite)
- `learning/` - Auto vocabulary learning (coordinator, diff_analyzer, llm_judge, validator, store)
- `builtin_dictionary_updater.rs` - Remote builtin hotwords fetch + atomic cache persistence + runtime refresh events
- `uia_text_reader.rs` - Windows UI Automation text capture
- `openai_client.rs` - Shared LLM client with connection testing
- `config.rs` - Configuration management with automatic migration

### Key Frontend Structure (src/)
- `pages/` - Page components (Dashboard, ASR, Models, LLM, Assistant, Hotkeys, Dictionary, Preferences, History, Help)
- `components/` - Reusable UI components (common/, layout/, learning/, live/, modals/, history/, notice/)
- `windows/` - Overlay and notification window components
- `hooks/` - Custom React hooks (useAppServiceController, useDictionary, useHotkeyRecording, useUpdater, useTauriEventListeners)
- `types/` - TypeScript type definitions
- `utils/` - Utility functions (dictionaryUtils)

## Build, Test, and Development Commands
- `npm install` installs frontend dependencies.
- `npm run dev` starts the Vite dev server for the UI.
- `npm run build` type-checks and builds the frontend bundle.
- `npm run preview` serves the built UI locally.
- `npm run test:ts` runs TypeScript runtime tests in `tests/*.test.ts`.
- `npm run tauri dev` runs the desktop app in dev mode; run as Administrator so global hotkeys work.
- `npm run tauri build` builds the NSIS installer only; output in `src-tauri/target/release/bundle/`.
- `cd src-tauri` then `cargo build`, `cargo check`, or `cargo test` for the Rust backend.
- `cd src-tauri` then `cargo run --bin test_api` to manually verify ASR API behavior.

## Coding Style & Naming Conventions
- TypeScript/React: 2-space indent, double quotes, and semicolons; components use `PascalCase`, hooks use `useX`, and UI files live in `*.tsx`.
- Rust: 4-space indent, `snake_case` for modules/functions and `CamelCase` for types; run `cargo fmt` before pushing.
- Tailwind CSS is used in JSX; keep class ordering consistent with nearby files.

## Testing Guidelines
- Development must follow TDD: write/adjust test methods first, then implement code.
- Validate test feasibility before implementation by running targeted tests and confirming they execute meaningfully.
- Implement only after test validation, then make tests pass and refactor within scope.
- Backend: run `cargo test` in `src-tauri/` for Rust tests.
- API checks: use `cargo run --bin test_api` when touching ASR integrations.
- Frontend: run `npm run test:ts`; additionally smoke-test via `npm run dev` and `npm run build`.
- Final quality gate: ensure overall Cargo compilation passes in `src-tauri/` (at least `cargo check`; prefer `cargo build` for release readiness).

## Windows-Only & Architecture Notes
- This repo targets Windows 10/11 only; avoid cross-platform abstractions and `#[cfg(target_os = ...)]` branches unless required.
- All compile/build/package steps are Windows-only; always use Windows tooling/commands (PowerShell, `npm run tauri ...`, `cargo` on Windows) and avoid Linux/macOS build paths.
- Prefer Win32 APIs for hotkeys/input (GetAsyncKeyState, SendInput) and registry for auto-start.
- Global hotkeys require admin rights; preserve ghost-key detection and the 500ms watchdog when editing hotkey logic.
- Keep clipboard/focus timing safeguards (100ms delay before capture, 150ms delay before insert) in assistant/overlay flows.
- Config lives at `%APPDATA%\PushToTalk\config.json`; migration logic is in `src-tauri/src/config.rs`.
- UIA text reader uses Windows UI Automation API; maintain COM initialization guards and timeout protection.
- Learning module uses async observation tasks; respect the deduplication mechanism per window handle.

## Commit & Pull Request Guidelines
- Follow Conventional Commit-style prefixes seen in history: `feat:`, `fix:`, `perf:`, `refactor:`, `chore:`; short summaries can be Chinese or English.
- PRs should include a clear description, test steps, and screenshots for UI changes; link related issues when possible.
- Keep changes scoped and call out any Windows/admin-impacting behavior.

## Security & Configuration Tips
- Do not commit API keys or local config files.
- Auto-update uses NSIS; avoid reintroducing MSI or multi-instance installers.
- LLM provider credentials are stored in config.json; ensure proper migration when changing schema.
- For deeper architecture details, see `CLAUDE.md`.

## Recent Feature Areas
- **LLM Provider Registry**: Multi-provider management in `ModelsPage.tsx` and `config.rs`
- **Auto Vocabulary Learning**: `learning/` module with UIA text capture
- **TNL Normalization Layer**: `tnl/` module for deterministic ASR normalization (pinyin/phonetic/hyphen/letter-merge)
- **Doubao IME First-Class ASR**: default provider, automatic credential bootstrap, and startup fallback behavior
- **Builtin Dictionary Runtime Refresh**: `builtin_dictionary_updater.rs` + `builtin_dictionary_updated` event + dynamic frontend domain snapshot
- **Tray Quick Switches**: runtime toggles for post-processing/dictionary enhancement and ASR provider switching from tray menu
- **Update Notes Aggregation**: cross-version release notes merge in updater modal (`releaseNotes.ts`)
- **Global Notice Capsule**: floating notification host (`GlobalNoticeHost.tsx`, `NoticeCapsule.tsx`)
- **Doubao Realtime Tuning**: bidirectional streaming path and parameter tuning in `asr/realtime/doubao.rs`
- **Polishing Failure Feedback**: runtime `polishing_failed` hint path from normal pipeline to frontend
- **Connection Testing**: `test_llm_provider` command with latency measurement

<!-- gitnexus:start -->
# GitNexus — Code Intelligence

This project is indexed by GitNexus as **push-2-talk** (1832 symbols, 4477 relationships, 153 execution flows). Use the GitNexus MCP tools to understand code, assess impact, and navigate safely.

> If any GitNexus tool warns the index is stale, run `npx gitnexus analyze` in terminal first.

## Always Do

- **MUST run impact analysis before editing any symbol.** Before modifying a function, class, or method, run `gitnexus_impact({target: "symbolName", direction: "upstream"})` and report the blast radius (direct callers, affected processes, risk level) to the user.
- **MUST run `gitnexus_detect_changes()` before committing** to verify your changes only affect expected symbols and execution flows.
- **MUST warn the user** if impact analysis returns HIGH or CRITICAL risk before proceeding with edits.
- When exploring unfamiliar code, use `gitnexus_query({query: "concept"})` to find execution flows instead of grepping. It returns process-grouped results ranked by relevance.
- When you need full context on a specific symbol — callers, callees, which execution flows it participates in — use `gitnexus_context({name: "symbolName"})`.

## When Debugging

1. `gitnexus_query({query: "<error or symptom>"})` — find execution flows related to the issue
2. `gitnexus_context({name: "<suspect function>"})` — see all callers, callees, and process participation
3. `READ gitnexus://repo/push-2-talk/process/{processName}` — trace the full execution flow step by step
4. For regressions: `gitnexus_detect_changes({scope: "compare", base_ref: "main"})` — see what your branch changed

## When Refactoring

- **Renaming**: MUST use `gitnexus_rename({symbol_name: "old", new_name: "new", dry_run: true})` first. Review the preview — graph edits are safe, text_search edits need manual review. Then run with `dry_run: false`.
- **Extracting/Splitting**: MUST run `gitnexus_context({name: "target"})` to see all incoming/outgoing refs, then `gitnexus_impact({target: "target", direction: "upstream"})` to find all external callers before moving code.
- After any refactor: run `gitnexus_detect_changes({scope: "all"})` to verify only expected files changed.

## Never Do

- NEVER edit a function, class, or method without first running `gitnexus_impact` on it.
- NEVER ignore HIGH or CRITICAL risk warnings from impact analysis.
- NEVER rename symbols with find-and-replace — use `gitnexus_rename` which understands the call graph.
- NEVER commit changes without running `gitnexus_detect_changes()` to check affected scope.

## Tools Quick Reference

| Tool | When to use | Command |
|------|-------------|---------|
| `query` | Find code by concept | `gitnexus_query({query: "auth validation"})` |
| `context` | 360-degree view of one symbol | `gitnexus_context({name: "validateUser"})` |
| `impact` | Blast radius before editing | `gitnexus_impact({target: "X", direction: "upstream"})` |
| `detect_changes` | Pre-commit scope check | `gitnexus_detect_changes({scope: "staged"})` |
| `rename` | Safe multi-file rename | `gitnexus_rename({symbol_name: "old", new_name: "new", dry_run: true})` |
| `cypher` | Custom graph queries | `gitnexus_cypher({query: "MATCH ..."})` |

## Impact Risk Levels

| Depth | Meaning | Action |
|-------|---------|--------|
| d=1 | WILL BREAK — direct callers/importers | MUST update these |
| d=2 | LIKELY AFFECTED — indirect deps | Should test |
| d=3 | MAY NEED TESTING — transitive | Test if critical path |

## Resources

| Resource | Use for |
|----------|---------|
| `gitnexus://repo/push-2-talk/context` | Codebase overview, check index freshness |
| `gitnexus://repo/push-2-talk/clusters` | All functional areas |
| `gitnexus://repo/push-2-talk/processes` | All execution flows |
| `gitnexus://repo/push-2-talk/process/{name}` | Step-by-step execution trace |

## Self-Check Before Finishing

Before completing any code modification task, verify:
1. `gitnexus_impact` was run for all modified symbols
2. No HIGH/CRITICAL risk warnings were ignored
3. `gitnexus_detect_changes()` confirms changes match expected scope
4. All d=1 (WILL BREAK) dependents were updated

## Keeping the Index Fresh

After committing code changes, the GitNexus index becomes stale. Re-run analyze to update it:

```bash
npx gitnexus analyze
```

If the index previously included embeddings, preserve them by adding `--embeddings`:

```bash
npx gitnexus analyze --embeddings
```

To check whether embeddings exist, inspect `.gitnexus/meta.json` — the `stats.embeddings` field shows the count (0 means no embeddings). **Running analyze without `--embeddings` will delete any previously generated embeddings.**

> Claude Code users: A PostToolUse hook handles this automatically after `git commit` and `git merge`.

## CLI

| Task | Read this skill file |
|------|---------------------|
| Understand architecture / "How does X work?" | `.claude/skills/gitnexus/gitnexus-exploring/SKILL.md` |
| Blast radius / "What breaks if I change X?" | `.claude/skills/gitnexus/gitnexus-impact-analysis/SKILL.md` |
| Trace bugs / "Why is X failing?" | `.claude/skills/gitnexus/gitnexus-debugging/SKILL.md` |
| Rename / extract / split / refactor | `.claude/skills/gitnexus/gitnexus-refactoring/SKILL.md` |
| Tools, resources, schema reference | `.claude/skills/gitnexus/gitnexus-guide/SKILL.md` |
| Index, status, clean, wiki CLI commands | `.claude/skills/gitnexus/gitnexus-cli/SKILL.md` |

<!-- gitnexus:end -->

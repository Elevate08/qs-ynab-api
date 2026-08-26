# YNAB Pulse: Service & Bar-Widget Architectural Decoupling

## Problem Statement
How might we decouple background sync, subprocess orchestration, and IPC handling from UI rendering so YNAB Pulse operates as a resilient, multi-monitor-safe Omarchy service with a lightweight bar widget?

## Recommended Direction
Transform the plugin from a monolithic `bar-widget` into a dual `kinds: ["service", "bar-widget"]` plugin modeled after `omarchy.media`:

1. **`Service.qml` (Singleton Service)**:
   - Runs headlessly in `omarchy-shell`'s `serviceHost`.
   - Owns the single `IpcHandler` for `io.github.elevate08.ynab-glance`.
   - Manages the periodic background refresh `Timer` and subprocesses (`fetchProc`, `forceFetchProc`, `YnabAuth`).
   - Maintains reactive state (`overviewData`, `authenticated`, `loading`, `statusError`, `activeBudgetId`).

2. **`BarWidget.qml` (Presentation & Popup Anchor)**:
   - Extends `qs.Ui.BarWidget`.
   - Binds to `bar?.shell?.serviceFor("io.github.elevate08.ynab-glance")`.
   - Renders the status bar icon and Age of Money pill.
   - Anchors and opens the `PopupCard` containing the detailed multi-tab financial view.

3. **`manifest.json` & Settings Consolidation**:
   - Update `kinds: ["service", "bar-widget"]` with `keepLoaded: true`.
   - Remove obsolete `overlay` entry point; consolidate settings & auth workflows within the panel popup.

## Key Assumptions to Validate
- [ ] `shell.serviceFor("io.github.elevate08.ynab-glance")` resolves reliably across multi-monitor bar instances.
- [ ] IPC methods (`open`, `toggle`, `refresh`, `status`) work seamlessly via `omarchy-shell` whether the popup is open or closed.
- [ ] Background periodic timer triggers without leaking memory or accumulating subprocess zombies.

## MVP Scope
- Create `Service.qml` with centralized state, timer, processes, and IPC handlers.
- Refactor `BarWidget.qml` to consume the service singleton and host the popup panel.
- Clean `Panel.qml` of duplicate subprocesses, IPC handlers, and timer loops.
- Update `manifest.json` to declare `kinds: ["service", "bar-widget"]`.
- Verify `tests/run.sh` passes and test hot-reloading with `omarchy-shell shell rescanPlugins`.

## Not Doing (and Why)
- **External Rust Daemon / systemd Unit**: Unnecessary complexity; QML `Process` wrapping `bin/ynab-cli` is fast (<15ms) and sandboxed to the user session.
- **Multi-budget simultaneous polling**: Avoids potential YNAB API rate limits and memory inflation.
- **Standalone `overlay` window**: `Settings.qml` is redundant since the panel already provides an integrated settings and token manager.

## Open Questions
- None. Requirements, lifecycle contracts, and multi-monitor safety guarantees are fully defined.

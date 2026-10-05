# IPC Module (`@modules/ipc`)

Bridges and event listeners connecting the Tauri backend to Reveal's frontend state machines.

## Architecture

- `appEventListeners.js`: Unified listener registration for all Tauri backend events (system, filesystem, memory cards, culling, import, export, and publication).
- `devPanelBridge.js`: Two-way messaging bridge between the main window and the detached floating Develop panel.
- `settingsPanelBridge.js`: Messaging bridge with the detached Settings window.
- `menuBridge.js`: Application menu commands routing (exports, reset, shortcuts, developer tools).
- `index.js`: Barrel export exposing all registration functions.

## Test Coverage

- `appEventListeners.test.js`: Validates comprehensive listener setup and event dispatching.
- `bridges.test.js`: Validates detached panel and menu IPC communication contracts.

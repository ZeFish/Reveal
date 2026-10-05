# Shell Module (`@modules/shell`)

Application chrome, boot orchestrators, and root shell components.

## Architecture

- `appBoot.js`: Synchronous session initialization (`initSessionBoot`) and asynchronous startup sequence (`performAppBoot`).
- `WindowControls.svelte`: Window control buttons and titlebar drag handles.
- `FullscreenViewer.svelte`: Fullscreen media preview and pan/zoom container.
- `AppModals.svelte`: Root modal container for long-running operations and notifications.
- `index.js`: Barrel export exposing components and lifecycle helpers.

## Test Coverage

- `appBoot.test.js`: Validates synchronous session state restoration and async startup workflows.

# fin Android Client (MVP scaffold)

Independent Android client parallel to WebUI.
Runtime truth only via local WS.

## Modules
- connection: ws lifecycle + handshake/subscription state machine
- session: session list/filter/switch/restore
- input: input queue and dedupe key
- turn: turn render models (compact/debug)
- runtime: worker/project/daemon status projections
- ws: protocol contracts and ws client adapter
- scan: barcode payload parse/validate
- store: single-source app state stores
- ui: compose screens (home/session/runtime/connection)

## Build
(Android project scaffold only in this commit; Gradle wrapper and app wiring next step.)

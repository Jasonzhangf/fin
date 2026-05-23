# Manual Visual Evidence Checklist (for blocked device capture)

When ADB capture is black/unavailable, collect these manually from device UI:

1. `e2e-manual-waiting.png`
   - After send click, show pending/waiting status.
2. `e2e-manual-progress-tool-error.png`
   - During inference, show tool/error timeline rendered.
3. `e2e-manual-finished.png`
   - Completed turn with assistant + tool/error records.
4. `e2e-manual-restart-recovery.png`
   - Force stop + relaunch, history restored and can continue chat.

Place files into:
- `reports/session-kb-screenshots/`

Then update:
- `reports/session-kb-validation.md` (G25/I31/I32 => PASS with evidence paths)
- `reports/session-kb-objective-audit.md` (same)
- `reports/session-kb-completion-audit-2026-05-16.md` summary counts

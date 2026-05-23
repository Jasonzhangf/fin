# Bugfix Workflow (Mandatory)

## Rule
Every bugfix must follow **Red -> Fix -> Green**.

## Steps
1. Reproduce bug with an automated regression test first (RED).
2. Save RED evidence under `reports/regression/bug-repro/<bug-id>/red.log`.
3. Implement fix.
4. Re-run test and full local regression (GREEN).
5. Save GREEN evidence under `reports/regression/bug-repro/<bug-id>/green.log`.

## Gates
- Missing RED evidence => fail.
- Missing GREEN evidence => fail.
- Full regression not run => fail.

# QA documentation

This directory contains quality, diagnostics, and regression references for contributors.

| Document | Purpose |
|---|---|
| [`regression-matrix.md`](regression-matrix.md) | Regression coverage checklist across CLI, runtime, project, cache, and platform behavior. |
| [`bug-triage.md`](bug-triage.md) | Triage workflow and data to collect for bugs. |
| [`diagnostics.md`](diagnostics.md) | Support-facing guidance for collecting `envr doctor` and `envr diagnostics export` data. |
| [`diagnostics-repro.md`](diagnostics-repro.md) | How to reproduce and inspect diagnostic export behavior. |

Most documents here are maintainer-facing. Users filing support issues may be linked to [`diagnostics.md`](diagnostics.md) when maintainers need structured troubleshooting data.

End users should usually start with the root [`README.md`](../../README.md), [`../cli/recipes.md`](../cli/recipes.md), or [`../release/KNOWN-ISSUES.md`](../release/KNOWN-ISSUES.md).

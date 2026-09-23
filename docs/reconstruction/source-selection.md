# Baseline Selection

Selected provisional baseline: `/home/photoos/PhotoOS-recovery/reimplementation-v15-20260911-172343`

reimplementation-v15 is the most complete integrated recovery workspace and contains the broadest verified server/client structure. It remains provisional: per-file candidate overrides and missing modern frontend pages will be handled in later phases.

## Family scores

| Root | Score | Rust | TS/TSX | CSS | FE critical | BE critical | Known risk markers |
|---|---:|---:|---:|---:|---:|---:|---:|
| `/home/photoos/PhotoOS-recovery/reimplementation-v15-20260911-172343` | 458 | 87 | 31 | 8 | 11/11 | 7/7 | 5 |
| `/home/photoos/PhotoOS-recovery/source-recovered-20260910-010351` | 448 | 77 | 31 | 8 | 11/11 | 7/7 | 1 |
| `/home/photoos/PhotoOS-recovery/git-main-20260910-004803` | 336 | 41 | 15 | 2 | 5/11 | 1/7 | 1 |

## Selection rules

- Exact recovered source has priority over recreation.
- Larger file size alone is not evidence that a candidate is newer or correct.
- StoragePage has multiple historical variants and must be behavior-reviewed before override.
- Missing modern live pages are reconstruction targets, not silent omissions.
- Hard-coded JWT and recovery stubs remain explicit Phase 2 blockers.

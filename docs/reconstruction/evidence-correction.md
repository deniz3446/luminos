# Evidence Correction — Phase 1 Batch 2

Batch 2's initial live frontend chunk detector inspected only the direct
`client/dist` directory. Vite production chunks are stored recursively under
the dist tree, normally `client/dist/assets`.

Phase 1 Batch 3 corrected this by scanning `client/dist/**/*.js`.

The correction affects only the live-frontend missing-source evidence.
It does not invalidate:
- the backend candidate risk classification;
- the SHA256 source candidate index;
- the provisional `reimplementation-v15` baseline selection.

# PhotoOS Dev Bridge Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** PC üzerinde çalışan, ChatGPT'nin PhotoOS kaynak kodunu güvenli biçimde düzenleyip test ederek GitHub'a göndermesini sağlayan canonical geliştirme köprüsünü kurmak.

**Architecture:** `C:\Users\MSI\PhotoOS-Dev` GitHub `deniz3446/luminos` deposunun canonical çalışma kopyası olacak. Sunucudaki doğrulanmış commit `273a3fe401c2803afa6b072ed7a4d5b0331e3377` dosya içeriği mevcut GitHub geçmişinin üzerine normal bir commit olarak alınacak; eski main ayrıca archive branch ile korunacak. Remote Desktop Commander dosya/terminal köprüsü olacak; Windows'taki küçük PhotoOS Dev Bridge uygulaması bağlantı ve repo durumunu gösterecek.

**Tech Stack:** Git 2.55, Python 3.12 + tkinter/unittest, Node 24 + npm.cmd, Windows PowerShell/cmd, Paramiko SSH, Rust testleri için izole PhotoOS server worktree.

**Spec:** `C:\Users\MSI\PhotoOS-AI\PHOTOOS-DEV-BRIDGE-DESIGN.md`

## Global Constraints
- Normal geliştirme canlı PhotoOS deploy/restart yapmaz.
- Disk, network/firewall, reboot, kullanıcı/izin, update install ve production signing ayrı onay gerektirir.
- Force push yapılmaz.
- Secret, JWT, update token ve private signing key Git/log içine yazılmaz.
- Test kapıları geçmeden kod branch'i main'e gönderilmez.
- GitHub eski `luminos/main` geçmişi `archive/luminos-legacy-20260628` branch'i ile korunur.

## Review Focus
- Sunucu SSH bağlantısı yoksa uygulama çökmeden `offline` göstermeli.
- Repo yok/kirli olduğunda bootstrap mevcut dosyaları sessizce ezmemeli.
- npm PowerShell execution-policy sorunu nedeniyle `npm.cmd` kullanılmalı.
- Git push başarısızsa başarı raporu yazılmamalı.
- Canlı deploy komutları normal helper akışında bulunmamalı.

---### Task 1: Canonical repository bootstrap

**Files:**
- Create: `C:\Users\MSI\PhotoOS-Dev\` clone/worktree
- Create: `C:\Users\MSI\PhotoOS-AI\bootstrap_photoos_repo.py`
- Test: `C:\Users\MSI\PhotoOS-AI\tests\test_bootstrap.py`

**Interfaces:**
- Consumes: GitHub `deniz3446/luminos` main; server source tree at `/home/photoos/PhotoOS-worktrees/update-discovery-1.2.4-rebuild-20260922`.
- Produces: clean local `PhotoOS-Dev` main containing canonical source, plus `archive/luminos-legacy-20260628` remote branch.

- [ ] **Step 1: Write failing bootstrap safety tests**
  Test that dirty destination aborts, source HEAD must equal `273a3fe401c2803afa6b072ed7a4d5b0331e3377`, archive branch creation is idempotent, and excluded `.git`/build artifacts are never copied.
- [ ] **Step 2: Verify RED**
  Run: `python -m unittest C:\Users\MSI\PhotoOS-AI\tests\test_bootstrap.py -v`
  Expected: FAIL because bootstrap helpers do not exist.
- [ ] **Step 3: Implement bootstrap helper**
  Clone `https://github.com/deniz3446/luminos.git`, create/push archive branch if absent, fetch canonical files over Paramiko SFTP, preserve Git metadata, remove files absent from canonical tree, and create one normal import commit.
- [ ] **Step 4: Verify GREEN locally without push**
  Run test suite again and a dry-run against a temporary repo fixture.
  Expected: all tests PASS; no GitHub mutation in dry-run.
- [ ] **Step 5: Execute bootstrap and verify**
  Confirm clean local main, expected 332 tracked canonical files before helper files, and `git diff --check` passes.
- [ ] **Step 6: Commit/push canonical import**
  Push archive branch first, then fast-forward updated main; never force push.

---### Task 2: Dev Bridge workflow helper

**Files:**
- Create: `C:\Users\MSI\PhotoOS-AI\photoos_dev_bridge.py`
- Create: `C:\Users\MSI\PhotoOS-AI\tests\test_dev_bridge.py`
- Modify: `C:\Users\MSI\PhotoOS-AI\AGENTS.md`

**Interfaces:**
- Consumes: canonical repo from Task 1 and configured SSH key/known_hosts.
- Produces: commands/functions for status, branch creation, frontend verification, remote backend verification, commit and push; live deploy is intentionally absent.

- [ ] **Step 1: Write failing workflow tests**
  Cover branch-name sanitization, dirty-main refusal, `npm.cmd` selection, backend command targeting an isolated remote worktree, push refusal after failed tests, and absence of deploy/restart commands.
- [ ] **Step 2: Verify RED**
  Run: `python -m unittest C:\Users\MSI\PhotoOS-AI\tests\test_dev_bridge.py -v`
  Expected: FAIL because bridge module does not exist.
- [ ] **Step 3: Implement minimal workflow functions**
  Add `repo_status()`, `create_task_branch(name)`, `run_frontend_gates()`, `run_backend_gates()`, `commit_and_push(message)`, and structured JSON report output. Use subprocess argument arrays rather than shell interpolation where practical.
- [ ] **Step 4: Update AGENTS safety contract**
  Change normal successful workflow from automatic deploy to automatic Git commit/push; require separate explicit approval for all live deploy/restart/update-install operations.
- [ ] **Step 5: Verify GREEN**
  Run bridge unit tests, then `python -m unittest discover C:\Users\MSI\PhotoOS-AI\tests -v`.
  Expected: all tests PASS.

---

### Task 3: Windows status application

**Files:**
- Create: `C:\Users\MSI\PhotoOS-AI\PhotoOS-Dev-Bridge.pyw`
- Create: `C:\Users\MSI\PhotoOS-AI\Start-PhotoOS-Dev-Bridge.cmd`
- Create: desktop shortcut `PhotoOS Dev Bridge.lnk`
- Test: extend `tests\test_dev_bridge.py`.

**Interfaces:**
- Consumes: status/report functions from Task 2.
- Produces: visible Windows status screen and one-click Remote Desktop Commander launcher.

- [ ] **Step 1: Add failing presentation/launcher tests**
  Verify status model returns Remote Commander state, repo branch/cleanliness, last verification result and Git remote without exposing secrets.
- [ ] **Step 2: Verify RED**
  Run targeted unittest; expected FAIL because status model/launcher are missing.
- [ ] **Step 3: Implement tkinter status UI**
  Show Ready/Offline/Error, canonical branch, Git cleanliness, last test result, latest commit, and a button to start the Remote Desktop Commander using `npx.cmd @wonderwhy-er/desktop-commander@latest remote`.
- [ ] **Step 4: Create launcher and desktop shortcut**
  `Start-PhotoOS-Dev-Bridge.cmd` invokes `pythonw.exe` with the `.pyw` file. Shortcut points only to this launcher.
- [ ] **Step 5: Verify GREEN**
  Run tests and start UI once in non-destructive smoke mode; confirm process starts and status data loads.

---### Task 4: End-to-end verification and operator report

**Files:**
- Create: `C:\Users\MSI\PhotoOS-AI\REPORT-2026-09-24.md`
- Create/update: `C:\Users\MSI\PhotoOS-AI\last-status.json`

**Interfaces:**
- Consumes: Tasks 1-3.
- Produces: fresh evidence that the bridge is ready for future PhotoOS coding requests.

- [ ] **Step 1: Verify repository state**
  Run `git status --short --branch`, `git remote -v`, `git log -3 --oneline`, and compare GitHub main to local HEAD.
  Expected: main clean; origin is `deniz3446/luminos`; local and remote HEAD match.
- [ ] **Step 2: Run frontend baseline gates**
  In `C:\Users\MSI\PhotoOS-Dev\client`: `npm.cmd ci`, `npm.cmd run contract-check`, `npm.cmd run build`; run lint and record its exact result separately.
  Expected: dependency install, contract-check and production build PASS; any pre-existing lint debt is named rather than hidden.
- [ ] **Step 3: Run backend baseline gates remotely**
  Create/use an isolated test worktree from the canonical commit under `/home/photoos/PhotoOS-worktrees/ai-verify-*`; run `cargo test --workspace` and `cargo check --workspace` without touching `/opt/photoos/current` or systemd.
  Expected: commands complete; failures, if any, are reported verbatim and block automatic main integration for future tasks until classified.
- [ ] **Step 4: Run security scan**
  Search tracked files for private-key headers, update-token assignments, JWT secret material and known credential patterns.
  Expected: no secret material in Git.
- [ ] **Step 5: Generate final report**
  Write installed paths, repository/branch SHAs, tests, known limitations and the exact future workflow. Report explicitly that live deploy remains approval-gated.
- [ ] **Step 6: Final verification**
  Re-run bridge unit tests and inspect `last-status.json`; only then mark installation ready.

## Self-review
- Spec coverage: repository bootstrap, isolation, tests, commit/push, status UI, no automatic live deploy, secret protection and reporting are all assigned to tasks.
- Placeholder scan: no deferred implementation placeholders.
- Interface consistency: Task 2 consumes Task 1 canonical repo; Task 3 consumes Task 2 status functions; Task 4 verifies all outputs.
- Review focus: offline SSH, dirty repo, PowerShell npm policy, push failure and deploy exclusion each have a test or explicit verification step.

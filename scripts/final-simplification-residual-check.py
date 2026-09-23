from pathlib import Path
import sys

root = Path(__file__).resolve().parents[1]
failures = []

def fail(msg):
    failures.append(msg)

def text(rel):
    p = root / rel
    return p.read_text(errors="replace") if p.exists() else ""

checks = {
    "server/src/config/mod.rs": [
        "mirror_source_path",
        "mirror_target_root_path",
        "mirror_target_path",
    ],
    "scripts/photoos-notification-aggregator.sh": [
        '"name": "mirror"',
        "mirror-state.env",
        '"prefix": "MIRROR"',
    ],
    "server/src/handlers/setup.rs": [
        "google_drive",
        "googleDrive",
        "google_drive_enabled",
    ],
}

for rel, tokens in checks.items():
    body = text(rel)
    for token in tokens:
        if token in body:
            fail(f"{rel}: retired token remains: {token}")

legacy = [
    "client/AutomationPage.tsx",
    "client/AutomationPage.css",
    "client/api.ts",
    "client/components/RuleCard.tsx",
    "client/components/RuleEditor.tsx",
    "client/components/RunHistory.tsx",
    "client/types.ts",
    "client/utils.ts",
    "server/src/handlers/cloud_engine.rs.before-v15-borrow-fix",
    "server/src/config/mod.rs.before-corruption-fix",
    "smoke-v15/cloud-health.out",
]

for rel in legacy:
    if (root / rel).exists():
        fail(f"legacy retired artifact remains: {rel}")

migration = root / "server/migrations/20260723021550_cloud_engine_core.sql"
if not migration.is_file():
    fail("historical cloud migration was removed")

installer = text("installer/install-to-disk.sh")
if "for GROUP in sudo disk docker; do" not in installer:
    fail("Docker Engine installer policy was removed")

if failures:
    print("FINAL_SIMPLIFICATION_RESIDUAL_CONTRACT=FAIL")
    for item in sorted(set(failures)):
        print(f"- {item}")
    sys.exit(1)

print("mirror_residuals=PASS")
print("cloud_setup_residuals=PASS")
print("legacy_artifacts=PASS")
print("historical_migration=PASS")
print("docker_engine_policy=PASS")
print("FINAL_SIMPLIFICATION_RESIDUAL_CONTRACT=PASS")

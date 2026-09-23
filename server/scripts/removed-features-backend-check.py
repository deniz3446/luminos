from pathlib import Path
import sys

server = Path(__file__).resolve().parents[1]
src = server / "src"

failures = []

def fail(msg):
    failures.append(msg)

# Feature-owned source files that must disappear.
forbidden_files = [
    "src/handlers/automation.rs",
    "src/handlers/cloud_accounts.rs",
    "src/handlers/cloud_auth.rs",
    "src/handlers/cloud_engine.rs",
    "src/handlers/cloud_sync.rs",
    "src/handlers/docker.rs",
    "src/handlers/mirror.rs",
    "src/handlers/mirror_scheduler.rs",

    "src/routes/automation.rs",
    "src/routes/cloud_accounts.rs",
    "src/routes/cloud_auth.rs",
    "src/routes/cloud_sync.rs",
    "src/routes/docker.rs",
    "src/routes/mirror.rs",

    "automation.rs",
]

for rel in forbidden_files:
    if (server / rel).exists():
        fail(f"forbidden feature file still exists: server/{rel}")

# Compiled source must no longer expose removed API families.
forbidden_api_prefixes = [
    "/api/v1/mirror",
    "/api/v1/docker",
    "/api/v1/cloud-sync",
    "/api/v1/cloud/accounts",
    "/api/v1/automation",
]

# Module names / implementation identifiers that should disappear
# from active server/src after feature removal.
forbidden_identifiers = [
    "routes::mirror",
    "routes::docker",
    "routes::cloud_sync",
    "routes::cloud_auth",
    "routes::cloud_accounts",
    "routes::automation",
    "handlers::mirror",
    "handlers::mirror_scheduler",
    "handlers::docker",
    "handlers::cloud_sync",
    "handlers::cloud_accounts",
    "handlers::cloud_auth",
    "handlers::cloud_engine",
    "handlers::automation",
]

rust_files = sorted(src.rglob("*.rs"))

for file in rust_files:
    text = file.read_text(errors="replace")
    rel = file.relative_to(server)

    for prefix in forbidden_api_prefixes:
        if prefix in text:
            fail(f"server/{rel}: forbidden API reference {prefix}")

    for ident in forbidden_identifiers:
        if ident in text:
            fail(f"server/{rel}: forbidden implementation reference {ident}")

# Explicit module declarations.
for modfile in [
    src / "handlers" / "mod.rs",
    src / "routes" / "mod.rs",
]:
    if not modfile.exists():
        fail(f"required module file missing: {modfile.relative_to(server)}")
        continue

    text = modfile.read_text(errors="replace")

    for name in [
        "automation",
        "cloud_accounts",
        "cloud_auth",
        "cloud_engine",
        "cloud_sync",
        "docker",
        "mirror",
        "mirror_scheduler",
    ]:
        if f"mod {name};" in text or f"pub mod {name};" in text:
            fail(
                f"server/{modfile.relative_to(server)}: "
                f"removed module still declared: {name}"
            )

# Retained core backend must still exist.
required_files = [
    "src/routes/photos.rs",
    "src/routes/albums.rs",
    "src/routes/storage.rs",
    "src/routes/raid.rs",
    "src/routes/backups.rs",
    "src/routes/devices.rs",
    "src/routes/notifications.rs",
    "src/routes/logs.rs",
    "src/routes/setup.rs",
    "src/routes/updates.rs",
    "src/routes/user.rs",
    "src/handlers/pc_backups.rs",
]

for rel in required_files:
    if not (server / rel).exists():
        fail(f"retained backend file missing: server/{rel}")

if failures:
    print("REMOVED_FEATURES_BACKEND_CONTRACT=FAIL")
    for item in sorted(set(failures)):
        print(f"- {item}")
    sys.exit(1)

print("removed_backend_files=PASS")
print("removed_backend_routes=PASS")
print("removed_backend_modules=PASS")
print("retained_backend_core=PASS")
print("REMOVED_FEATURES_BACKEND_CONTRACT=PASS")

#!/usr/bin/env bash
set -Eeuo pipefail
ROOT=/home/photoos/PhotoOS-recovery/reimplementation-v15-20260911-172343
SANDBOX="$ROOT/smoke-v15"
BIN="$ROOT/target/release/server"
mount --bind "$SANDBOX/runtime" /var/lib/photoos/runtime
mount --bind "$SANDBOX/srv-photoos" /srv/photoos
export PHOTOOS_HOST=127.0.0.1 PHOTOOS_PORT=18080
export PHOTOOS_DATABASE_PATH=/var/lib/photoos/runtime/smoke.db
export PHOTOOS_RUNTIME_DIR=/var/lib/photoos/runtime
export PHOTOOS_TEMP_DIR=/var/lib/photoos/runtime/tmp
export PHOTOOS_CACHE_ROOT=/var/lib/photoos/runtime/cache
export PHOTOOS_LOG_ROOT=/var/lib/photoos/runtime/logs
export PHOTOOS_STORAGE_ROOT=/srv/photoos
export PHOTOOS_DISKS_ROOT=/srv/photoos/disks
export PHOTOOS_DISK1=/srv/photoos/disks/disk1
export PHOTOOS_DISK2=/srv/photoos/disks/disk2
export PHOTOOS_FRONTEND_DIR="$SANDBOX/frontend"
export PHOTOOS_FRONTEND_INDEX="$SANDBOX/frontend/index.html"
export PHOTOOS_VERSION=1.2.1-recovery-v15-smoke
export PHOTOOS_ENVIRONMENT=smoke
exec "$BIN"

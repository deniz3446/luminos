#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def read(rel: str) -> str:
    path = ROOT / rel
    if not path.is_file():
        raise AssertionError(f"missing required file: {rel}")
    return path.read_text(encoding="utf-8")


def require(text: str, needle: str, label: str) -> None:
    if needle not in text:
        raise AssertionError(f"{label}: missing {needle!r}")


def reject(text: str, needle: str, label: str) -> None:
    if needle in text:
        raise AssertionError(f"{label}: forbidden legacy text {needle!r}")


agent = read("update-agent/update_agent.py")
helper = read("update-agent/update_helper.py")
core = read("update-agent/agent_core.py")
agent_unit = read("systemd/photoos-update-agent.service")
helper_unit = read("systemd/photoos-update-helper.service")
install = read("installer/install.sh")
disk_install = read("installer/install-to-disk.sh")
build_installer = read("installer/build-installer.sh")

for rel, text in (
    ("update-agent/update_agent.py", agent),
    ("update-agent/update_helper.py", helper),
    ("update-agent/agent_core.py", core),
):
    require(text, "PhotoOS", rel)

require(agent, 'bind_host="127.0.0.1"', "agent source")
require(agent, 'TOKEN_HEADER = "X-PhotoOS-Update-Token"', "agent source")
require(helper, '["systemctl", "restart", "photoos.service"]', "helper source")
reject(helper, "shell=True", "helper source")

for needle in (
    "User=photoos",
    "Group=photoos",
    "ExecStart=/usr/bin/python3 /opt/photoos/update-agent/update_agent.py",
    "Environment=PHOTOOS_UPDATE_TOKEN_FILE=/etc/photoos/update-agent.env",
    "Wants=photoos-update-helper.service",
    "After=photoos-update-helper.service",
    "NoNewPrivileges=true",
    "ProtectSystem=strict",
    "ReadWritePaths=/var/lib/photoos/update",
):
    require(agent_unit, needle, "agent systemd unit")
reject(agent_unit, "User=root", "agent systemd unit")
reject(agent_unit, "/var/lib/photoos/runtime/token", "agent systemd unit")

for needle in (
    "User=root",
    "Group=root",
    "ExecStart=/usr/bin/python3 /opt/photoos/update-agent/update_helper.py",
    "RuntimeDirectory=photoos",
    "NoNewPrivileges=true",
    "ProtectSystem=strict",
    "ReadWritePaths=/opt/photoos /var/lib/photoos/update /run/photoos",
):
    require(helper_unit, needle, "helper systemd unit")

require(install, 'TOKEN_ENV_FILE="$ETC_ROOT/update-agent.env"', "installer/install.sh")
require(disk_install, "/etc/photoos/update-agent.env", "installer/install-to-disk.sh")
for label, text in (("installer/install.sh", install), ("installer/install-to-disk.sh", disk_install)):
    require(text, "secrets.token_hex(32)", label)
    require(text, "chmod 0640", label)
    require(text, "root:photoos", label)
    require(text, "photoos-update-agent.service", label)
    require(text, "photoos-update-helper.service", label)
    reject(text, "/var/lib/photoos/runtime/token", label)


require(build_installer, 'SOURCE_AGENT="$PROJECT_ROOT/update-agent"', "installer/build-installer.sh")
reject(build_installer, 'SOURCE_AGENT="/opt/photoos/update-agent"', "installer/build-installer.sh")
require(build_installer, "for file in update_agent.py update_helper.py agent_core.py", "installer/build-installer.sh")
require(build_installer, '$SOURCE_AGENT/$file', "installer/build-installer.sh")
require(build_installer, 'SOURCE_SYSTEMD="$PROJECT_ROOT/systemd"', "installer/build-installer.sh")
require(build_installer, "for unit in photoos-update-agent.service photoos-update-helper.service", "installer/build-installer.sh")
require(build_installer, '$SOURCE_SYSTEMD/$unit', "installer/build-installer.sh")
require(build_installer, '$PAYLOAD/systemd', "installer/build-installer.sh")
require(install, '$PAYLOAD_DIR/systemd/$unit', "installer/install.sh")
reject(install, 'cat > /etc/systemd/system/photoos-update-agent.service', "installer/install.sh")
reject(install, 'cat > /etc/systemd/system/photoos-update-helper.service', "installer/install.sh")

print("UPDATE_AGENT_SOURCE_CONTRACT=PASS")

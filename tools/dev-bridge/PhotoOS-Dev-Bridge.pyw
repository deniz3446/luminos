from __future__ import annotations

import os
import subprocess
import tkinter as tk
from tkinter import messagebox, ttk

from photoos_dev_bridge import (
    REPO,
    ROOT,
    load_status_file,
    remote_commander_command,
    repo_snapshot,
    status_lines,
)


class DevBridgeApp(tk.Tk):
    def __init__(self) -> None:
        super().__init__()
        self.title("PhotoOS Dev Bridge")
        self.geometry("720x470")
        self.minsize(640, 420)
        self.remote_process: subprocess.Popen | None = None

        self.status_var = tk.StringVar(value="Durum okunuyor...")
        self.remote_var = tk.StringVar(value="Remote Commander: bağlantı bekleniyor")
        self._build_ui()
        self.refresh_status()

    def _build_ui(self) -> None:
        root = ttk.Frame(self, padding=24)
        root.pack(fill="both", expand=True)

        ttk.Label(
            root,
            text="PhotoOS Dev Bridge",
            font=("Segoe UI", 22, "bold"),
        ).pack(anchor="w")
        ttk.Label(
            root,
            text="Kod değişikliği → test → Git commit/push. Canlı deploy ayrıca onay ister.",
            font=("Segoe UI", 10),
        ).pack(anchor="w", pady=(4, 18))

        status_box = ttk.LabelFrame(root, text="Geliştirme Durumu", padding=16)
        status_box.pack(fill="both", expand=True)
        ttk.Label(
            status_box,
            textvariable=self.status_var,
            justify="left",
            font=("Consolas", 11),
        ).pack(anchor="w", fill="x")

        ttk.Separator(root).pack(fill="x", pady=14)
        ttk.Label(root, textvariable=self.remote_var).pack(anchor="w")

        buttons = ttk.Frame(root)
        buttons.pack(fill="x", pady=(14, 0))
        ttk.Button(
            buttons,
            text="Remote Commander Başlat",
            command=self.start_remote_commander,
        ).pack(side="left")
        ttk.Button(
            buttons,
            text="Durumu Yenile",
            command=self.refresh_status,
        ).pack(side="left", padx=10)
        ttk.Button(
            buttons,
            text="PhotoOS-Dev Klasörünü Aç",
            command=self.open_repo,
        ).pack(side="left")

    def refresh_status(self) -> None:
        try:
            repo = repo_snapshot()
            last = load_status_file()
            self.status_var.set("\n".join(status_lines(repo, last)))
        except Exception as exc:
            self.status_var.set(f"Durum okunamadı:\n{exc}")

    def open_repo(self) -> None:
        if REPO.exists():
            os.startfile(REPO)
        else:
            messagebox.showerror("PhotoOS Dev Bridge", f"Repo bulunamadı: {REPO}")

    def start_remote_commander(self) -> None:
        try:
            flags = getattr(subprocess, "CREATE_NEW_CONSOLE", 0)
            self.remote_process = subprocess.Popen(
                remote_commander_command(),
                cwd=ROOT,
                creationflags=flags,
            )
            self.remote_var.set(
                f"Remote Commander: başlatıldı (PID {self.remote_process.pid})"
            )
        except Exception as exc:
            messagebox.showerror(
                "PhotoOS Dev Bridge",
                f"Remote Commander başlatılamadı:\n{exc}",
            )


def main() -> None:
    app = DevBridgeApp()
    app.mainloop()


if __name__ == "__main__":
    main()

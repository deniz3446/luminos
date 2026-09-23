import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();

const loginPath = path.join(root, "src/pages/LoginPage.tsx");
const cssPath = path.join(root, "src/pages/LoginPage.css");
const layoutPath = path.join(root, "src/layouts/DashboardLayout.tsx");

const failures = [];

if (!fs.existsSync(loginPath)) {
  failures.push("LoginPage.tsx missing");
}

if (!fs.existsSync(cssPath)) {
  failures.push("LoginPage.css missing");
}

const login = fs.existsSync(loginPath)
  ? fs.readFileSync(loginPath, "utf8")
  : "";

const css = fs.existsSync(cssPath)
  ? fs.readFileSync(cssPath, "utf8")
  : "";

const layout = fs.readFileSync(layoutPath, "utf8");

for (const token of [
  'import "./LoginPage.css"',
  'className="login-page"',
  'className="login-shell"',
  "Private photo cloud",
  "Fotoğraflarınız. Verileriniz. Sizin bulutunuz.",
  "GÜVENLİ ERİŞİM",
  "Hesabınıza Giriş Yapın",
  "PhotoOS’a Giriş",
  "YOUR PRIVATE CLOUD",
  "Kendi sunucunuzda. Kendi kurallarınızla.",
]) {
  if (!login.includes(token)) {
    failures.push(`login contract missing: ${token}`);
  }
}

for (const token of [
  ".login-page",
  ".login-shell",
  ".login-card",
  ".login-visual",
  ".cloud-diagram",
]) {
  if (!css.includes(token)) {
    failures.push(`login css missing: ${token}`);
  }
}

if (!layout.includes('apiJson<MeEnvelope>("/api/v1/me")')) {
  failures.push("DashboardLayout session endpoint changed unexpectedly");
}

if (login.includes('background: "#111827"')) {
  failures.push("legacy inline login design remains");
}

if (failures.length) {
  console.error("LOGIN_SESSION_FRONTEND_CONTRACT=FAIL");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("LOGIN_SESSION_FRONTEND_CONTRACT=PASS");

import fs from "node:fs";
import path from "node:path";
import process from "node:process";

const root = process.cwd();

const appPath = path.join(root, "src/App.tsx");
const pagePath = path.join(root, "src/pages/NotFoundPage.tsx");

const app = fs.readFileSync(appPath, "utf8");

const failures = [];

if (!fs.existsSync(pagePath)) {
  failures.push("NotFoundPage.tsx missing");
}

if (!app.includes('import NotFoundPage from "./pages/NotFoundPage";')) {
  failures.push("NotFoundPage import missing");
}

if (!/path="\*"\s+element=\{<NotFoundPage\s*\/>\}/s.test(app)) {
  failures.push("wildcard route does not render NotFoundPage");
}

if (/path="\*"\s+element=\{<Navigate\s+to="\/login"/s.test(app)) {
  failures.push("wildcard route still redirects to login");
}

if (failures.length) {
  console.error("NOT_FOUND_CONTRACT=FAIL");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

const page = fs.readFileSync(pagePath, "utf8");

for (const required of [
  "404",
  "Sayfa bulunamadı",
  'to="/dashboard"',
]) {
  if (!page.includes(required)) {
    failures.push(`NotFound page missing: ${required}`);
  }
}

if (failures.length) {
  console.error("NOT_FOUND_CONTRACT=FAIL");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exit(1);
}

console.log("NOT_FOUND_CONTRACT=PASS");

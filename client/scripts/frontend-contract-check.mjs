import fs from "node:fs";import path from "node:path";import process from "node:process";
const root=process.cwd();const app=fs.readFileSync(path.join(root,"src/App.tsx"),"utf8");const client=fs.readFileSync(path.join(root,"src/api/client.ts"),"utf8");
const routes=["/dashboard","/photos","/albums","/storage","/raid","/backup","/pc-backups","/notifications","/logs","/devices","/tv","/health","/settings"];
const missing=routes.filter(r=>!app.includes(`path=\"${r}\"`));if(missing.length){console.error("Missing routes:",missing.join(", "));process.exit(1)}
if(/192\.168\.|http:\/\/10\.|http:\/\/172\.(1[6-9]|2\d|3[01])\./.test(client)){console.error("Fixed private-network API URL found in client.ts");process.exit(1)}
if(!client.includes("window.location.origin")||!client.includes("VITE_API_URL")){console.error("Dynamic API base contract missing");process.exit(1)}
console.log(`FRONTEND_CONTRACT=PASS routes=${routes.length}`);

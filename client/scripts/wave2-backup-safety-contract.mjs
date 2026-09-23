import fs from "node:fs";

const backup = fs.readFileSync(new URL("../src/pages/BackupManagerPage.tsx", import.meta.url), "utf8");
const raid = fs.readFileSync(new URL("../src/pages/RaidManagerPage.tsx", import.meta.url), "utf8");
const backend = fs.readFileSync(new URL("../../server/src/handlers/backups.rs", import.meta.url), "utf8");

const failures = [];
const expect = (condition, message) => {
  if (!condition) failures.push(message);
};

expect(
  backend.includes("if source == target || target.starts_with(&source)"),
  "backend must reject a target equal to or nested under the source"
);
expect(
  backend.includes('("PHOTOOS_DATA1", "/srv/photoos/disks/disk1")') &&
    backend.includes('("PHOTOOS_DATA2", "/srv/photoos/disks/disk2")'),
  "backup target labels must use PHOTOOS_DATA1 / PHOTOOS_DATA2"
);
expect(
  backup.includes("function photoosDiskRoot") &&
    backup.includes("function samePhotoosDisk") &&
    backup.includes("function firstSafeTarget"),
  "frontend must define same-physical-PhotoOS-disk safety helpers"
);
expect(
  backup.includes("firstSafeTarget(selectedSource,nextTargets.targets)"),
  "initial/current target must be selected from a different PhotoOS disk"
);
expect(
  backup.includes("const changeSource=(nextSource:string)=>"),
  "changing source must re-evaluate the selected target"
);
expect(
  backup.includes("!samePhotoosDisk(source,target)"),
  "start button eligibility must reject same-disk source/target"
);
expect(
  backup.includes("disabled={!x.available||samePhotoosDisk(source,x.path)}"),
  "same-disk target options must be disabled"
);
expect(
  !raid.includes("PHOTOOS1 ve PHOTOOS2") &&
    raid.includes("PHOTOOS_DATA1 ve PHOTOOS_DATA2"),
  "RAID explanatory copy must use current data-disk names"
);

if (failures.length) {
  console.error("BACKUP_SAFETY_CONTRACT=FAIL");
  for (const failure of failures) console.error(`FAIL: ${failure}`);
  process.exit(1);
}

console.log("BACKUP_SAFETY_CONTRACT=PASS");

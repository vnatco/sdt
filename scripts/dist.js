// Copies the built installer to ShutDownTimer_<version>_x64-setup.exe, the
// name used for releases (Tauri names it after the product, with spaces).
import { copyFileSync, existsSync, readFileSync } from "node:fs";

const conf = JSON.parse(readFileSync("src-tauri/tauri.conf.json", "utf8"));
const dir = "src-tauri/target/release/bundle/nsis";
const from = `${dir}/${conf.productName}_${conf.version}_x64-setup.exe`;
const to = `${dir}/ShutDownTimer_${conf.version}_x64-setup.exe`;

if (!existsSync(from)) {
  console.error(`Installer not found: ${from}`);
  process.exit(1);
}
copyFileSync(from, to);
console.log(`Release installer: ${to}`);

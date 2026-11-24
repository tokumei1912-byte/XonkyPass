// Создаёт latest.json для автообновления (формат tauri-plugin-updater).
// Приложение скачивает его из последнего релиза на GitHub.
//
// Использование: node scripts/updater-manifest.mjs <версия> <папка с файлами релиза> <owner/repo>
// В папке должны лежать установщик `*-setup.exe` и его подпись `*-setup.exe.sig`.

import { readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';

const [version, dir, repo] = process.argv.slice(2);
if (!version || !dir || !repo) {
  console.error('Использование: node scripts/updater-manifest.mjs <версия> <папка> <owner/repo>');
  process.exit(1);
}

const installers = readdirSync(dir).filter((name) => name.endsWith('-setup.exe'));
if (installers.length !== 1) {
  console.error(`Ожидался один установщик *-setup.exe в ${dir}, найдено: ${installers.length}`);
  process.exit(1);
}
const installer = installers[0];
if (!installer.includes(`_${version}_`)) {
  console.error(`Версия установщика ${installer} не совпадает с ${version}`);
  process.exit(1);
}
const signature = readFileSync(join(dir, `${installer}.sig`), 'utf8').trim();

const manifest = {
  version,
  notes: `XonkyPass ${version}`,
  pub_date: new Date().toISOString().replace(/\.\d{3}Z$/, 'Z'),
  platforms: {
    'windows-x86_64': {
      signature,
      url: `https://github.com/${repo}/releases/download/v${version}/${encodeURIComponent(installer)}`,
    },
  },
};

writeFileSync(join(dir, 'latest.json'), JSON.stringify(manifest, null, 2) + '\n');
console.log(`latest.json: ${version} → ${manifest.platforms['windows-x86_64'].url}`);

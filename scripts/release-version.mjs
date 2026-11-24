// Версия релиза: MAJOR.MINOR из Cargo.toml + номер запуска CI как PATCH.
// Номер запуска только растёт, поэтому каждый релиз новее предыдущего.
// Записывает версию в Cargo.toml и печатает её.
//
// Использование: node scripts/release-version.mjs <номер запуска>

import { readFileSync, writeFileSync } from 'node:fs';

const CARGO_TOML = new URL('../Cargo.toml', import.meta.url);

const run = process.argv[2];
if (!/^\d+$/.test(run ?? '')) {
  console.error('Укажите номер запуска CI: node scripts/release-version.mjs <число>');
  process.exit(1);
}

const toml = readFileSync(CARGO_TOML, 'utf8');
const pattern = /(\[workspace\.package\][^[]*?\nversion = ")(\d+)\.(\d+)\.\d+(")/;
const match = toml.match(pattern);
if (!match) {
  console.error('В Cargo.toml не найдена версия в [workspace.package]');
  process.exit(1);
}

const version = `${match[2]}.${match[3]}.${Number(run)}`;
writeFileSync(CARGO_TOML, toml.replace(pattern, `$1${version}$4`));
console.log(version);

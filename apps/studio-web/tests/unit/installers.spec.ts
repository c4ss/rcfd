// F-820: the installer rules that keep a user's settings across an update (scripts/installer-config.mjs, ADR 0801
// Addendum 17). The settings live in Electron's userData folder, named after productName; an update keeps them as long
// as the name, the NSIS appId and the keep-on-uninstall flag stay put, and the installed app is the tested app folder.
import { expect, test } from '@playwright/test';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { APP_ID, builderConfig, PRODUCT_NAME, runtimePackage } from '../../scripts/installer-config.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');
const pkg = JSON.parse(readFileSync(path.join(root, 'package.json'), 'utf8'));
const cfg = builderConfig({ pkg, stage: 'out/stage', output: 'out/installers', sign: false });

test('the userData folder keeps its name: out/packaged and the installers both say productName "rcfd Studio"', () => {
  // Renaming it would leave every user's recent list, service state and Claude state behind in the old folder.
  expect(PRODUCT_NAME).toBe('rcfd Studio');
  expect(runtimePackage(pkg)['productName']).toBe(PRODUCT_NAME);
  expect(cfg['productName']).toBe(PRODUCT_NAME);
});

test('an NSIS update installs over the same copy and never deletes the settings', () => {
  expect(APP_ID).toBe('org.rcfd.studio');
  expect(cfg['appId']).toBe(APP_ID);
  expect(cfg['nsis'].deleteAppDataOnUninstall).toBe(false);
  expect(cfg['nsis'].perMachine).toBe(false);
});

test('the installers carry the tested app folder: no asar, only package.json and dist/, the pinned Electron', () => {
  expect(cfg['asar']).toBe(false);
  expect(cfg['files']).toEqual(['package.json', 'dist/**']);
  expect(cfg['electronVersion']).toBe(pkg.devDependencies.electron);
});

test('nothing is published and signing is off unless asked for', () => {
  expect(cfg['publish']).toBeNull();
  expect(cfg['win'].signExecutable).toBe(false);
  expect(builderConfig({ pkg, stage: 's', output: 'o', sign: true })['win'].signExecutable).toBe(true);
  expect(cfg['mac']).toBeUndefined();
});

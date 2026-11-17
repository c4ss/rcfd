// What the Studio's installers are made of (F-820, ADR 0801 Addendum 17). Pure: scripts/package-installers.mjs builds
// with it, scripts/package-dir.mjs shares runtimePackage() with it, and tests/unit/installers.spec.ts checks the rules
// that keep a user's settings across an update.
//
// Where the settings live. Electron's userData folder is named after productName, so every build that says
// productName "rcfd Studio" reads the same folder: %APPDATA%\rcfd Studio on Windows, ~/.config/rcfd Studio on Linux
// ($XDG_CONFIG_HOME when set). It holds recent-projects.json, service/ (a started service's address and log) and claude/
// (the Claude panel's state); everything else is in the project folder (G12). No installer writes there: NSIS keeps it
// on uninstall (deleteAppDataOnUninstall false) and on an upgrade in place (the same appId), a .deb never owns files in
// a home folder, an AppImage is one file replaced by the next.
//
// The G10 settings themselves (sandbox, context isolation, CSP, IPC allow-list) are in the code and its tests, not in
// any file an installer could change; the installers carry the same app folder as out/packaged (no asar, no fuses yet,
// see the ADR's open questions), so the tested build and the installed one run the same files.

/** The productName that names the userData folder. Changing it moves every user's settings: never without a migration. */
export const PRODUCT_NAME = 'rcfd Studio';
/** NSIS finds the installed copy it upgrades by this id. Changing it installs a second copy beside the first. */
export const APP_ID = 'org.rcfd.studio';
export const EXECUTABLE = 'rcfd-studio';

/**
 * What the installers' app package.json adds to runtimePackage(): electron-builder names them in the installer and the
 * .deb's control file. Metadata only; out/packaged does not carry it.
 */
export const INSTALLER_METADATA = {
  description: 'rcfd Studio, the desktop front end of the rcfd CFD engine',
  author: { name: 'rcfd contributors', url: 'https://github.com/c4ss/rcfd' },
  homepage: 'https://github.com/c4ss/rcfd',
};

/** The package.json of the packaged app (resources/app), from the repository's package.json. */
export function runtimePackage(pkg) {
  return {
    name: pkg.name,
    productName: PRODUCT_NAME,
    version: pkg.version,
    private: true,
    license: pkg.license,
    type: pkg.type,
    main: pkg.main,
  };
}

/**
 * The electron-builder configuration. `stage` is the folder holding app/ (runtimePackage() and dist/) and bin/ (the
 * engine: rcfd, rcfd-mcp, rcfd-runner), `output` where the installers go, `sign` whether Windows signing is on.
 */
export function builderConfig({ pkg, stage, output, sign }) {
  return {
    appId: APP_ID,
    productName: PRODUCT_NAME,
    executableName: EXECUTABLE,
    electronVersion: pkg.devDependencies.electron,
    copyright: 'rcfd contributors, AGPL-3.0-only',
    directories: { app: `${stage}/app`, output },
    // The same files as out/packaged (scripts/package-dir.mjs), which the tests run: a plain app folder, no
    // node_modules (the renderer is bundled by Vite, main and preload need only Electron and Node).
    asar: false,
    npmRebuild: false,
    nodeGypRebuild: false,
    files: ['package.json', 'dist/**'],
    // The engine beside the app: <resources>/bin. Where a packaged Studio looks for it is open (ADR 0801 Addendum 17,
    // Q-E); today the Studio still has to be told by RCFD_STUDIO_MCP / RCFD_STUDIO_RCFD.
    extraResources: [{ from: `${stage}/bin`, to: 'bin' }],
    // Never published from a build: artifacts only, the maintainer releases by hand (docs/releasing.md).
    publish: null,
    win: {
      target: [{ target: 'nsis', arch: ['x64'] }],
      // Signing is off unless RCFD_STUDIO_SIGN=1 and a certificate is given (CSC_LINK / WIN_CSC_LINK and
      // CSC_KEY_PASSWORD, electron-builder's own variables); package-installers.mjs checks both.
      signExecutable: sign,
    },
    nsis: {
      oneClick: false,
      perMachine: false,
      allowToChangeInstallationDirectory: true,
      deleteAppDataOnUninstall: false,
      artifactName: 'rcfd-studio-${version}-setup-${arch}.${ext}',
    },
    linux: {
      target: [
        { target: 'AppImage', arch: ['x64'] },
        { target: 'deb', arch: ['x64'] },
      ],
      category: 'Science',
      maintainer: 'rcfd contributors <https://github.com/c4ss/rcfd>',
      synopsis: 'The rcfd Studio, the desktop front end of the rcfd CFD engine',
      artifactName: 'rcfd-studio-${version}-${arch}.${ext}',
    },
    deb: { packageName: 'rcfd-studio' },
  };
}

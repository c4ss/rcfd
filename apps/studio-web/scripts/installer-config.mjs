// What the Studio's installers are made of (F-820, ADR 0801 Addendum 19). Pure: scripts/package-installers.mjs builds
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

// The identity is permanent (the maintainer, 2026-11-17, Q-F820-5): appId org.rcfd.studio, the install folder
// "rcfd Studio", the version from the git tag only (a manual build stays package.json's 0.0.0, so a test build never
// looks newer than a release).

/** The productName that names the userData folder. Changing it moves every user's settings: never without a migration. */
export const PRODUCT_NAME = 'rcfd Studio';
/** NSIS finds the installed copy it upgrades by this id. Changing it installs a second copy beside the first. */
export const APP_ID = 'org.rcfd.studio';
/**
 * The Linux executable (/opt/rcfd Studio/rcfd-studio, the .deb's and the AppImage's). Windows sets none: electron-builder
 * names the NSIS install folder after the executable when one is set, and the folder is "rcfd Studio"
 * (%LOCALAPPDATA%\Programs\rcfd Studio), so the Windows executable is "rcfd Studio.exe".
 */
export const EXECUTABLE = 'rcfd-studio';
/** The NSIS install folder (per user: %LOCALAPPDATA%\Programs\<this>) and the Windows executable's name. */
export const WINDOWS_INSTALL_FOLDER = PRODUCT_NAME;
/**
 * Linux: Electron sets the window's app_id / WM_CLASS from package.json's desktopName, and the desktop links a running
 * window to its menu entry by it; with linux.syncDesktopName the installed .desktop file has the same name.
 */
export const DESKTOP_NAME = `${EXECUTABLE}.desktop`;

/**
 * What the installers' app package.json adds to runtimePackage(): electron-builder names them in the installer and the
 * .deb's control file. Metadata only; out/packaged does not carry it.
 */
export const INSTALLER_METADATA = {
  description: 'rcfd Studio, the desktop front end of the rcfd CFD engine',
  author: { name: 'rcfd contributors', url: 'https://github.com/c4ss/rcfd' },
  homepage: 'https://github.com/c4ss/rcfd',
  desktopName: DESKTOP_NAME,
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
    electronVersion: pkg.devDependencies.electron,
    copyright: 'rcfd contributors, AGPL-3.0-only',
    directories: { app: `${stage}/app`, output },
    // The same files as out/packaged (scripts/package-dir.mjs), which the tests run: a plain app folder, no
    // node_modules (the renderer is bundled by Vite, main and preload need only Electron and Node).
    asar: false,
    npmRebuild: false,
    nodeGypRebuild: false,
    files: ['package.json', 'dist/**'],
    // The engine beside the app: <resources>/bin. A packaged Studio with neither RCFD_STUDIO_MCP nor RCFD_STUDIO_RCFD
    // set uses it (src/main/project/config.ts bundledBin; never PATH), and starts its rcfd serve with the first project.
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
      executableName: EXECUTABLE,
      category: 'Science',
      syncDesktopName: true,
      maintainer: 'rcfd contributors <https://github.com/c4ss/rcfd>',
      synopsis: 'The rcfd Studio, the desktop front end of the rcfd CFD engine',
      artifactName: 'rcfd-studio-${version}-${arch}.${ext}',
    },
    deb: { packageName: 'rcfd-studio' },
  };
}

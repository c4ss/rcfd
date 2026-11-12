// @ts-check
'use strict';
// Preload script of the spike (ADR 0350, F-447): hands the MessagePort of the main process to the page and exposes the
// renderer process's own memory figures (P9). Nothing else crosses the context bridge.

const { contextBridge, ipcRenderer } = require('electron');

ipcRenderer.on('port', (e) => {
  window.postMessage('port', '*', e.ports);
});

contextBridge.exposeInMainWorld('host', {
  memory: () => process.getProcessMemoryInfo(),
});

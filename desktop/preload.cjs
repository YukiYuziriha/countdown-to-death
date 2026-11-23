'use strict';

const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('countdownHost', {
  getState: () => ipcRenderer.invoke('countdown:get-state'),
  saveState: state => ipcRenderer.invoke('countdown:save-state', state),
  getStatus: () => ipcRenderer.invoke('countdown:get-status'),
});

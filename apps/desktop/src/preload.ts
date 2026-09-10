import { contextBridge, ipcRenderer } from 'electron';
import type { DesktopApi } from './api';
import type { OperationsApi } from './operations-api';
import type { PreferencesApi } from './language';
import type { SupportApi } from './support-api';
const api: DesktopApi = {
  handshake: () => ipcRenderer.invoke('core:handshake'),
  searchCatalog: (rarity, locale) => ipcRenderer.invoke('core:catalog', { rarity, locale }),
  resolveRecommendedLevel: displayedLevel => ipcRenderer.invoke('core:recommended-level', displayedLevel),
  startSearch: (params) => ipcRenderer.invoke('core:start', params),
  currentSearch: () => ipcRenderer.invoke('core:current'),
  snapshot: (jobId) => ipcRenderer.invoke('core:snapshot', jobId),
  cancelSearch: (jobId) => ipcRenderer.invoke('core:cancel', jobId),
  restartWorker: () => ipcRenderer.invoke('core:restart'),
};
contextBridge.exposeInMainWorld('nioh', api);
const operations: OperationsApi = {
  prepareCount: params => ipcRenderer.invoke('operations:prepare-count',params),
  selectSave: () => ipcRenderer.invoke('operations:select'),
  execute: command => ipcRenderer.invoke('operations:execute', command),
  snapshot: (role, jobId) => ipcRenderer.invoke('operations:snapshot', { role, jobId }),
  cancel: (role, jobId) => ipcRenderer.invoke('operations:cancel', { role, jobId }),
  current: role => ipcRenderer.invoke('operations:current', role),
  prepareInstall: params => ipcRenderer.invoke('operations:install', params),
  prepareLiveAdd: params => ipcRenderer.invoke('operations:live-add', params),
  generate: params => ipcRenderer.invoke('operations:generate', params),
  searchNative: params => ipcRenderer.invoke('operations:native-search', params),
  captureGrace: params => ipcRenderer.invoke('operations:capture-grace', params),
  bindCachedSearch: params => ipcRenderer.invoke('operations:bind-cache', params),
};
contextBridge.exposeInMainWorld('operations', operations);
const preferences: PreferencesApi = {
  getLocale: () => ipcRenderer.invoke('preferences:locale'),
  setLocale: locale => ipcRenderer.invoke('preferences:set-locale', locale),
};
contextBridge.exposeInMainWorld('preferences', preferences);
const support: SupportApi = {
  diagnostics: () => ipcRenderer.invoke('support:diagnostics'),
  exportDiagnostics: () => ipcRenderer.invoke('support:export'),
};
contextBridge.exposeInMainWorld('support', support);

const review: import('./review-api').ReviewApi = {
 favorites: params => ipcRenderer.invoke('review:favorites',params),
 update: params => ipcRenderer.invoke('review:update',params),
 auxiliary: params => ipcRenderer.invoke('review:auxiliary',params),
 dataDirectory: action => ipcRenderer.invoke('review:data-directory',action),
 openSaveFolder: params => ipcRenderer.invoke('review:save-folder',params),
 log: message => ipcRenderer.invoke('review:log',message),
 copyLog: () => ipcRenderer.invoke('review:copy-log'),
 windowAction: action => ipcRenderer.invoke('review:window',action),
 retain: params => ipcRenderer.invoke('review:retain',params),
 release: key => ipcRenderer.invoke('review:release',key),
 preview: params => ipcRenderer.invoke('review:preview',params),
 prepareCart: params => ipcRenderer.invoke('review:prepare-cart',params),
 openBackupFolder:()=>ipcRenderer.invoke('review:backup-folder'),
  openLink: name => ipcRenderer.invoke('review:link',name),
 copyText: text => ipcRenderer.invoke('review:copy',text),
};
contextBridge.exposeInMainWorld('review',review);

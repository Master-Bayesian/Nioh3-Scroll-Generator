import { contextBridge, ipcRenderer } from 'electron';
import type { DesktopApi } from './api';
import type { OperationsApi } from './operations-api';
import type { PreferencesApi } from './language';
import type { SupportApi } from './support-api';
import { createDiagnosticInvoker } from './invoke-with-diagnostics';
const call = createDiagnosticInvoker((channel, value = null) => ipcRenderer.invoke(channel, value)) as (channel: string, value?: unknown) => Promise<any>;
const api: DesktopApi = {
  handshake: () => call('core:handshake'),
  searchCatalog: (rarity, locale) => call('core:catalog', { rarity, locale }),
  resolveRecommendedLevel: displayedLevel => call('core:recommended-level', displayedLevel),
  startSearch: (params) => call('core:start', params),
  currentSearch: () => call('core:current'),
  snapshot: (jobId) => call('core:snapshot', jobId),
  cancelSearch: (jobId) => call('core:cancel', jobId),
  restartWorker: () => call('core:restart'),
};
contextBridge.exposeInMainWorld('nioh', api);
const operations: OperationsApi = {
  prepareCount: params => call('operations:prepare-count',params),
  selectSave: () => call('operations:select'),
  execute: command => call('operations:execute', command),
  snapshot: (role, jobId) => call('operations:snapshot', { role, jobId }),
  cancel: (role, jobId) => call('operations:cancel', { role, jobId }),
  current: role => call('operations:current', role),
  prepareInstall: params => call('operations:install', params),
  prepareLiveAdd: params => call('operations:live-add', params),
  generate: params => call('operations:generate', params),
  searchNative: params => call('operations:native-search', params),
  captureGrace: params => call('operations:capture-grace', params),
  bindCachedSearch: params => call('operations:bind-cache', params),
};
contextBridge.exposeInMainWorld('operations', operations);
const preferences: PreferencesApi = {
  getLocale: () => call('preferences:locale'),
  setLocale: locale => call('preferences:set-locale', locale),
};
contextBridge.exposeInMainWorld('preferences', preferences);
const support: SupportApi = {
  diagnostics: () => call('support:diagnostics'),
  exportDiagnostics: () => call('support:export'),
};
contextBridge.exposeInMainWorld('support', support);

const review: import('./review-api').ReviewApi = {
 favorites: params => call('review:favorites',params),
 update: params => call('review:update',params),
 auxiliary: params => call('review:auxiliary',params),
 dataDirectory: action => call('review:data-directory',action),
 openSaveFolder: params => call('review:save-folder',params),
 log: message => call('review:log',message),
 copyLog: () => call('review:copy-log'),
 windowAction: action => call('review:window',action),
 retain: params => call('review:retain',params),
 release: key => call('review:release',key),
 preview: params => call('review:preview',params),
 prepareCart: params => call('review:prepare-cart',params),
 openBackupFolder:()=>call('review:backup-folder'),
  openLink: name => call('review:link',name),
 copyText: text => call('review:copy',text),
};
contextBridge.exposeInMainWorld('review',review);

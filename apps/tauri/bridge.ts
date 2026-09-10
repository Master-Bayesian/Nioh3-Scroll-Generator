import { invoke } from '@tauri-apps/api/core';
const call = (channel: string, value: unknown = null): Promise<any> => invoke('desktop_request', { channel, value });
window.nioh = {
  handshake: () => call('core:handshake'), searchCatalog: (rarity, locale) => call('core:catalog', { rarity, locale }),
  resolveRecommendedLevel: v => call('core:recommended-level', v), startSearch: p => call('core:start', p),
  currentSearch: () => call('core:current'), snapshot: id => call('core:snapshot', id), cancelSearch: id => call('core:cancel', id), restartWorker: () => call('core:restart'),
};
window.operations = {
  prepareCount: p => call('operations:prepare-count', p), selectSave: () => call('operations:select'), execute: p => call('operations:execute', p),
  snapshot: (role, jobId) => call('operations:snapshot', {role, jobId}), cancel: (role, jobId) => call('operations:cancel', {role, jobId}), current: role => call('operations:current', role),
  prepareInstall: p => call('operations:install', p), prepareLiveAdd: p => call('operations:live-add', p), generate: p => call('operations:generate', p), searchNative: p => call('operations:native-search', p),
  captureGrace: p => call('operations:capture-grace', p), bindCachedSearch: p => call('operations:bind-cache', p),
};
window.preferences = { getLocale: () => call('preferences:locale'), setLocale: v => call('preferences:set-locale', v) };
window.support = { diagnostics: () => call('support:diagnostics'), exportDiagnostics: () => call('support:export') };
window.review = {
  favorites: p => call('review:favorites', p), update: p => call('review:update', p), auxiliary: p => call('review:auxiliary', p), dataDirectory: p => call('review:data-directory', p),
  openSaveFolder: p => call('review:save-folder', p), log: p => call('review:log', p), copyLog: () => call('review:copy-log'), windowAction: p => call('review:window', p),
  retain: p => call('review:retain', p), release: p => call('review:release', p), preview: p => call('review:preview', p), prepareCart: p => call('review:prepare-cart', p),
  openBackupFolder: () => call('review:backup-folder'), openLink: p => call('review:link', p), copyText: p => call('review:copy', p),
};
document.addEventListener('mousedown', e => {
  if (e.button === 0 && (e.target as Element).closest('.topbar') && !(e.target as Element).closest('button,input,a,select')) void call('review:window', 'drag');
});

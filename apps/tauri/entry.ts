import './bridge';
import { invoke } from '@tauri-apps/api/core';
if (!localStorage.getItem('nioh3-ui-locale')) {
  const locale = await window.preferences.getLocale();
  localStorage.setItem('nioh3-ui-locale', locale);
}
await import('../workshop/main');
const ready = () => {
  if (!document.querySelector('#root .shell')) return false;
  requestAnimationFrame(() => requestAnimationFrame(() => void invoke('desktop_request', {channel:'support:ready',value:null})));
  return true;
};
if (!ready()) {
  const observer = new MutationObserver(() => { if (ready()) observer.disconnect(); });
  observer.observe(document.getElementById('root')!, {childList:true});
}

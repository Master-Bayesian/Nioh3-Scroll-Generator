import {PortableUpdate} from './portable-update';
import {FavoritesStore} from './favorites-store';
import {spawn} from 'node:child_process';
import {createHash} from 'node:crypto';
import {RollingLog} from './rolling-log';
import { app, BrowserWindow, ipcMain, dialog, shell, clipboard } from 'electron';
import { resolve, dirname } from 'node:path';
import { pathToFileURL } from 'node:url';
import { CandidateRegistry } from './candidate-registry';
import type {ReviewApi} from './review-api';
import { WorkerClient } from './worker-client';
import { ProtectedClient } from './protected-client';
import { publicCurrentJob, requirePublicJob } from './public-operation-jobs';
import type { ProtectedParams, ProtectedMethod } from './protected-client';
import type { OperationsApi, PublicOperation } from './operations-api';
import type { CandidateTransfer } from '../../../packages/contracts/responses';
import { PreferencesStore } from './preferences';
import { translate } from './locales';
import { mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { writeFile } from 'node:fs/promises';
import { verifyPortable } from '../../../packages/packaging/integrity.mjs';
import { version } from '../../../package.json';
import type { DiagnosticReport } from './support-api';
import * as originalFilesystem from 'original-fs';

// Automated smoke runs must not overwrite the user's language or Chromium profile.
if (process.env.NIOH3_ELECTRON_TEST) {
  app.setPath('userData', mkdtempSync(resolve(tmpdir(), 'nioh3-v2-electron-')));
}

const root = app.isPackaged ? process.resourcesPath : resolve(__dirname, '../../..');
const primaryInstance = app.requestSingleInstanceLock();
if (!primaryInstance) app.quit();
const workerPath = app.isPackaged ? resolve(root, 'worker/nioh3-search-worker.exe') : (process.env.NIOH3_PYTHON || 'python');
let worker: WorkerClient;
let window: BrowserWindow;
let quitting = false;
let restarting = false;
let closing = false;
let preferences: PreferencesStore;
let runtimeLog:RollingLog;
let updater:PortableUpdate;
let applyUpdate=false;
let packageVerification: DiagnosticReport['packageVerification'] = null;
const protectedHosts = new Map<'save' | 'runtime', ProtectedClient>();
function host(role: 'save' | 'runtime') {
  if (role !== 'save' && role !== 'runtime') throw new Error('INVALID_ROLE');
  let client = protectedHosts.get(role);
  if (!client) {
    client = new ProtectedClient(root, app.isPackaged ? resolve(root, 'worker/nioh3-protected-worker.exe') : workerPath, role, app.isPackaged,message=>runtimeLog?.write(role+'-stderr',message));
    protectedHosts.set(role, client);
  }
  return client;
}
const reviewMode=process.env.NIOH3_REVIEW_UI==='1'||app.isPackaged&&process.env.NIOH3_LEGACY_WORKBENCH!=='1';
const entry = resolve(__dirname, reviewMode ? 'review.html' : 'index.html');
const cartRegistry=new CandidateRegistry();
const trustedUrl = pathToFileURL(entry).href;

app.enableSandbox();
app.on('second-instance', () => {
  if (window && !window.isDestroyed()) { if (window.isMinimized()) window.restore(); window.show(); window.focus(); }
});
app.whenReady().then(async () => {
  if (!primaryInstance) return;
  // Verify before any Python worker can own a save operation or native hook.
  if (app.isPackaged) {
    packageVerification = await verifyPortable(resolve(root, '..'), originalFilesystem);
    if (packageVerification.version !== version) throw new Error('PACKAGE_VERSION_MISMATCH');
  }
  preferences = new PreferencesStore(resolve(app.getPath('userData'), 'v2-preferences.json'), app.getLocale());
  const favorites = new FavoritesStore(resolve(app.getPath('userData'), 'favorites.json'));
  runtimeLog = new RollingLog(resolve(app.getPath('userData'),'logs'));
  runtimeLog.write('startup',version);
  updater=new PortableUpdate(resolve(app.getPath('userData'),'updates'),version,resolve(__dirname,'extract-update.ps1'));
  worker = new WorkerClient(root, workerPath, app.isPackaged,message=>runtimeLog?.write('search-stderr',message));
  window = new BrowserWindow({ width: reviewMode ? 1600 : 1100, height: reviewMode ? 1000 : 800, show: !process.env.NIOH3_ELECTRON_TEST, minWidth:1100, minHeight:740, frame:!reviewMode, icon:resolve(root,'assets/nioh3-scroll-generator-icon.png'),
    webPreferences: { preload: resolve(__dirname, 'preload.cjs'), contextIsolation: true, sandbox: true, nodeIntegration: false, webviewTag: false },
  });
  window.setMenu(null);
  window.webContents.on('did-start-loading',()=>cartRegistry.clear());
  window.webContents.setWindowOpenHandler(() => ({ action: 'deny' }));
  window.webContents.on('will-navigate', (event) => event.preventDefault());
  window.webContents.session.setPermissionRequestHandler((_wc, _permission, callback) => callback(false));
  window.webContents.session.setPermissionCheckHandler(() => false);
  const register = (channel: string, handler: (value: unknown) => unknown) => ipcMain.handle(channel, (event, value) => {
    if (event.sender !== window.webContents || event.senderFrame !== window.webContents.mainFrame || event.senderFrame.url !== trustedUrl) throw new Error('UNTRUSTED_SENDER');
    if (restarting) throw new Error('WORKER_RESTARTING');
    if (closing) throw new Error('APPLICATION_CLOSING');
    const quiet=/snapshot|current|catalog|retain|release/.test(channel)||(channel==='review:update'&&(value as {action?:string})?.action==='status');
    if(!quiet)runtimeLog.write('request',channel);
    return Promise.resolve().then(()=>handler(value)).then(result=>{if(!quiet)runtimeLog.write('complete',channel);return result},error=>{runtimeLog.write('error',channel+' '+String(error));throw error});
  });
  register('review:update', value=>{
    const params=value as {action:string;channel:'stable'|'beta'};
    if(!params||!['status','check','download','apply'].includes(params.action)||!['stable','beta'].includes(params.channel))throw Error('INVALID_UPDATE_ACTION');
    if(params.action==='check')void updater.check(params.channel);
    if(params.action==='download')void updater.download().catch(e=>runtimeLog.write('update',e));
    if(params.action==='apply'){
      if(!app.isPackaged)throw Error('源码启动不支持自动替换，请使用发行包测试更新。');
      if(updater.state.phase!=='ready'||!updater.stagedDirectory)throw Error('UPDATE_NOT_READY');
      applyUpdate=true;app.quit();
    }
    return {...updater.state,canApply:app.isPackaged};
  });
  register('review:window' , value => { if(value==='minimize')window.minimize(); else if(value==='maximize'){if(window.isMaximized())window.unmaximize();else window.maximize();}else if(value==='close')window.close();else throw Error('INVALID_WINDOW_ACTION'); });
  register('core:handshake', () => worker.handshake());
  register('preferences:locale', () => preferences.getLocale());
  register('preferences:set-locale', value => preferences.setLocale(value));
  const diagnostics = async (): Promise<DiagnosticReport> => ({
    schema: 'nioh3-v2-diagnostics/v1', version, packaged: app.isPackaged,
    locale: await preferences.getLocale(), platform: process.platform, arch: process.arch,
    runtimeVersions: { electron: process.versions.electron, node: process.versions.node, chrome: process.versions.chrome },
    packageVerification, workers: [worker.diagnostics(), ...[...protectedHosts.values()].map(client => client.diagnostics())],
  });
  register('support:diagnostics', diagnostics);
  register('support:export', async () => {
    const selected = await dialog.showSaveDialog(window, { title: translate(await preferences.getLocale(), 'exportDiagnostics'),
      defaultPath: 'nioh3-v2-diagnostics.json', filters: [{ name: 'JSON', extensions: ['json'] }] });
    if (selected.canceled || !selected.filePath) return { saved: false };
    await writeFile(selected.filePath, JSON.stringify(await diagnostics(), null, 2) + '\n', 'utf8');
    return { saved: true };
  });
  register('core:catalog', (value) => {
    if (!value || typeof value !== 'object') throw new Error('INVALID_REQUEST');
    const { rarity, locale } = value as { rarity: 3 | 4 | 5; locale: 'en-US' | 'zh-CN' | 'ja-JP' };
    return worker.catalog(rarity, locale);
  });
  register('core:start', (value) => worker.start(value as Parameters<WorkerClient['start']>[0]));
  register('core:current', () => worker.current());
  register('core:recommended-level', value => worker.resolveRecommendedLevel(value as number));
  register('core:snapshot', (value) => worker.snapshot(value as string));
  register('core:cancel', (value) => worker.cancel(value as string));
  register('core:restart', async () => {
    restarting = true;
    try { await worker.close(); worker = new WorkerClient(root, workerPath, app.isPackaged,message=>runtimeLog?.write('search-stderr',message)); return await worker.handshake(); }
    finally { restarting = false; }
  });
  const publicMethods = new Set(['runtime.count_execute','runtime.count_status','runtime.count_recover','save.recycle_backups', 'save.discover', 'save.inventory', 'save.prepare_edit', 'save.prepare_delete',
    'save.backups', 'save.prepare_restore', 'save.discard', 'save.commit', 'save.operation', 'save.operations',
    'runtime.status', 'runtime.start_override', 'runtime.stop_override',
    'runtime.live_batch_execute','runtime.live_batch_status','runtime.live_batch_cancel',
    'runtime.live_add_execute', 'runtime.live_add_status', 'runtime.live_add_recover', 'runtime.live_add_cancel']);
  register('operations:prepare-count',async value=>{
    const params=value as Parameters<OperationsApi['prepareCount']>[0];
    if(!params||!Number.isInteger(params.new_count)||params.new_count<0||params.new_count>7)throw Error('INVALID_COUNT');
    const source=await host('save').run('save.count_edit_source',{save_id:params.save_id,snapshot_id:params.snapshot_id,slot_index:params.slot_index});
    if(!source||!('count_source' in source))throw Error('COUNT_SOURCE_EXPECTED');
    return host('runtime').call('runtime.count_prepare',{source:source.count_source as ProtectedParams<'runtime.count_prepare'>['source'],new_count:params.new_count});
  });
  register('operations:execute', value => {
    const command = value as PublicOperation;
    if (!command || !publicMethods.has(command.method)) throw new Error('PRIVATE_OR_UNKNOWN_OPERATION');
    return host(command.method.startsWith('save.') ? 'save' : 'runtime').call(command.method, command.params);
  });
  register('operations:select', async () => {
    const result = await dialog.showOpenDialog(window, { title: translate(await preferences.getLocale(), 'selectSave'), properties: ['openFile'], filters: [{ name: 'SAVEDATA.BIN', extensions: ['BIN'] }] });
    return result.canceled ? null : host('save').call('save.register', { path: result.filePaths[0] });
  });
  register('operations:current', async value => {
    if (value !== 'save' && value !== 'runtime') throw new Error('INVALID_ROLE');
    const client = protectedHosts.get(value);
    if (!client) return { job: null, busy: false };
    const result = await client.call('job.current', {});
    return publicCurrentJob(result.job);
  });
  for (const action of ['snapshot', 'cancel'] as const) register(`operations:${action}`, async value => {
    const params = value as { role: 'save' | 'runtime'; jobId: string };
    const client = host(params.role);
    const job = requirePublicJob(await client.call('job.snapshot', { job_id: params.jobId }));
    return action === 'snapshot' ? job : requirePublicJob(await client.call('job.cancel', { job_id: params.jobId }));
  });
  register('review:retain',async value=>{const params=value as Parameters<ReviewApi['retain']>[0];return cartRegistry.retain(params.source==='runtime'?await host('runtime').run('runtime.export',{candidate_id:params.candidate_id}) as unknown as CandidateTransfer:await worker.exportCandidate(params.job_id,params.candidate_id))});
  register('review:favorites',async value=>{
    const params=value as Parameters<ReviewApi['favorites']>[0];
    if(!params||!['list','add','remove'].includes(params.action))throw Error('INVALID_FAVORITES_ACTION');
    const entries=params.action==='add'?await favorites.add(params.sample!,cartRegistry.resolve([params.reference_id!])[0]):params.action==='remove'?await favorites.remove(params.key!):await favorites.list();
    return entries.map(({sample,transfer})=>({...sample,backend:{candidateId:transfer.candidate_id,referenceId:cartRegistry.retain(transfer).reference_id,installable:true}}));
  });
  register('review:release',value=>{if(typeof value!=='string')throw new Error('INVALID_REFERENCE');cartRegistry.release(value)});
  register('review:preview',async value=>{const params=value as Parameters<ReviewApi['preview']>[0];const result=await worker.previewSeed({seed:params.seed,rarity:params.rarity,level:params.level});return {reference_id:params.retain===false?null:cartRegistry.retain(result.transfer).reference_id,candidate:result.candidate}});
  register('review:prepare-cart',async value=>{
    const params=value as Parameters<ReviewApi['prepareCart']>[0];if(!params||!['save','live'].includes(params.mode))throw new Error('INVALID_ADDITION_MODE');
    if(!Array.isArray(params.references)||params.references.length>50)throw Error('CART_CAPACITY_REACHED');
    const candidates=cartRegistry.resolve(params.references);
    const request={save_id:params.save_id,snapshot_id:params.snapshot_id,candidates,recommended_level:params.recommended_level,transfer_count:params.transfer_count} as ProtectedParams<'save.prepare_install_many'>;
    if(params.mode==='save')return host('save').call('save.prepare_install_many',request);
    const prepared=await host('save').run('save.materialize_live_many',request);
    if(!prepared||!('candidates' in prepared)||!('save_path' in prepared)||!Array.isArray(prepared.candidates)||typeof prepared.save_path!=='string')throw new Error('LIVE_MATERIALIZATION_EXPECTED');
    return host('runtime').call('runtime.live_batch_prepare',{candidates:prepared.candidates,save_path:prepared.save_path} as ProtectedParams<'runtime.live_batch_prepare'>);
  });
  register('review:auxiliary',async value=>{const p=value as {seed:number;playthrough:number};const result=await host('save').run('save.auxiliary_preview',p);if(!('auxiliary_json' in result)||typeof result.auxiliary_json!=='string')throw Error('AUXILIARY_EXPECTED');return JSON.parse(result.auxiliary_json)});
  register('review:data-directory',async value=>{
    if(!['inspect','set','reset','open'].includes(value as string))throw Error('INVALID_DIRECTORY_ACTION');
    let path:string|null=null;
    if(value==='set'){const selected=await dialog.showOpenDialog(window,{properties:['openDirectory','createDirectory']});if(selected.canceled)return null;path=selected.filePaths[0];}
    const result=await host('save').run('save.data_directory',{action:value==='open'?'inspect':value as 'inspect'|'set'|'reset',path});
    if(!('data_directory' in result)||typeof result.data_directory!=='string')throw Error('DIRECTORY_EXPECTED');
    if(value==='open'){const error=await shell.openPath(result.data_directory);if(error)throw Error(error);}
    return result;
  });
  register('review:save-folder',async value=>{const result=await host('save').run('save.live_add_source',value as {save_id:string;snapshot_id:string});if(!('save_path' in result)||typeof result.save_path!=='string')throw Error('SAVE_EXPECTED');const error=await shell.openPath(dirname(result.save_path));if(error)throw Error(error)});
  register('review:backup-folder',async()=>{const result=await host('save').run('save.backup_location',{});if(!('backup_directory' in result)||typeof result.backup_directory!=='string')throw new Error('INVALID_BACKUP_DIRECTORY');const error=await shell.openPath(result.backup_directory);if(error)throw new Error(error)});
  register('review:log',value=>{if(typeof value!=='string'||value.length>16384)throw Error('INVALID_LOG_MESSAGE');runtimeLog.write('ui',value)});
  register('review:copy-log',async()=>{clipboard.writeText(JSON.stringify(await diagnostics(),null,2)+'\n'+runtimeLog.tail()+'\n'+worker.stderr.join('\n'));});
  register('review:copy',value=>{if(typeof value!=='string'||value.length>200000)throw new Error('INVALID_CLIPBOARD_TEXT');clipboard.writeText(value)});
  register('review:link',value=>{
    const links={github:'https://github.com/Master-Bayesian/Nioh3-Scroll-Generator',updates:'https://github.com/Master-Bayesian/Nioh3-Scroll-Generator/releases/latest',qq:'https://qm.qq.com/cgi-bin/qm/qr?k=0qS7eJtELBBcN8_ne4B7qG-c63Ze6pIo&jump_from=webapi&authKey=OMNXRYe8Ns3exbv9xiDr6HOQca3C/F+f5dVguJS7d2NFCf5URf308buzPfPXaf2G'};
    if(typeof value!=='string'||!Object.prototype.hasOwnProperty.call(links,value))throw new Error('INVALID_EXTERNAL_LINK');return shell.openExternal(links[value as keyof typeof links]);
  });
  register('operations:install', async value => {
    const params = value as Parameters<OperationsApi['prepareInstall']>[0];
    if (!params || !['search', 'runtime'].includes(params.source)) throw new Error('INVALID_CANDIDATE_SOURCE');
    const candidate = params.source === 'search' ? await worker.exportCandidate(params.job_id, params.candidate_id)
      : await host('runtime').run('runtime.export', { candidate_id: params.candidate_id }) as unknown as CandidateTransfer;
    return host('save').call('save.prepare_install', { save_id: params.save_id, snapshot_id: params.snapshot_id,
      candidate, recommended_level: params.recommended_level, transfer_count: params.transfer_count });
  });
  register('operations:live-add', async value => {
    const params = value as Parameters<OperationsApi['prepareLiveAdd']>[0];
    if (!params || !['search', 'runtime'].includes(params.source)) throw new Error('INVALID_CANDIDATE_SOURCE');
    const candidate = params.source === 'search' ? await worker.exportCandidate(params.job_id, params.candidate_id)
      : await host('runtime').run('runtime.export', { candidate_id: params.candidate_id }) as unknown as CandidateTransfer;
    const source = await host('save').run('save.live_add_source', { save_id: params.save_id, snapshot_id: params.snapshot_id });
    if (!('save_path' in source) || typeof source.save_path !== 'string') throw new Error('INVALID_SAVE_SOURCE');
    return host('runtime').call('runtime.live_add_prepare', { candidate, save_path: source.save_path });
  });
  register('operations:generate', async value => {
    const params = value as Parameters<OperationsApi['generate']>[0];
    const template = await host('save').run('save.template', { save_id: params.save_id, snapshot_id: params.snapshot_id, playthrough: params.playthrough });
    return host('runtime').call('runtime.generate', { template: template as any, seed: params.seed,
      playthrough: params.playthrough, rarity: params.rarity, level: params.level,
      recommended_level: params.recommended_level, title_screen_confirmed: params.title_screen_confirmed });
  });
  register('operations:native-search', async value => {
    const params = value as Parameters<OperationsApi['searchNative']>[0];
    const template = await host('save').run('save.template', { save_id: params.save_id, snapshot_id: params.snapshot_id, playthrough: params.playthrough });
    return host('runtime').call('runtime.search', { template: template as any, seed: params.seed,
      playthrough: params.playthrough, rarity: params.rarity, level: params.level,
      recommended_level: params.recommended_level, title_screen_confirmed: params.title_screen_confirmed,
      criteria: params.criteria, max_seeds: params.max_seeds, after_trial:params.after_trial||0 });
  });
  register('operations:capture-grace', async value => {
    const params = value as Parameters<OperationsApi['captureGrace']>[0];
    const template = await host('save').run('save.template', { save_id: params.save_id, snapshot_id: params.snapshot_id, playthrough: params.playthrough });
    return host('runtime').call('runtime.capture_grace', { template: template as any,
      playthrough: params.playthrough, rarity: params.rarity, level: params.level,
      recommended_level: params.recommended_level, title_screen_confirmed: params.title_screen_confirmed });
  });
  register('operations:bind-cache', async value => {
    const result = await host('save').run('save.cached_grace', value as Parameters<OperationsApi['bindCachedSearch']>[0]);
    return worker.registerCache(result.cache_json as string);
  });
  window.on('close', event => { if (!quitting) { event.preventDefault(); app.quit(); } });
  await window.loadFile(entry);
}).catch(async error => {
  // No automatic relaunch or operation replay after a failed startup.
  if (process.env.NIOH3_ELECTRON_TEST) console.error('STARTUP_FAILED:', error);
  else await dialog.showMessageBox({ type: 'error', title: 'Nioh 3 V2 startup failed', message: String(error) });
  app.quit();
});
app.on('window-all-closed', () => app.quit());
app.on('before-quit', (event) => {
  if (!primaryInstance) return;
  if (quitting) return;
  event.preventDefault();
  if (closing) return;
  closing = true;
  void (async () => {
    for (const [role, client] of protectedHosts) {
      if (!await client.close()) {
        closing = false;
        const locale = await preferences.getLocale().catch(() => 'en-US' as const);
        await dialog.showMessageBox(window, { type: 'warning', title: translate(locale, 'shutdownTitle'),
          message: translate(locale, 'shutdownMessage') });
        return;
      }
      protectedHosts.delete(role);
    }
    await worker?.close();
    if(applyUpdate&&updater.stagedDirectory){
      try{const helper=resolve(app.getPath('userData'),'updates','apply-update.ps1');await writeFile(helper,await originalFilesystem.promises.readFile(resolve(__dirname,'apply-update.ps1')));
      const manifestHash=updater.manifestHash;if(!manifestHash)throw Error('UPDATE_VERIFICATION_MISSING');
      const child=spawn('powershell.exe',['-NoProfile','-NonInteractive','-ExecutionPolicy','Bypass','-File',helper,'-ProcessId',String(process.pid),'-Target',dirname(process.execPath),'-Staged',updater.stagedDirectory,'-ManifestHash',manifestHash],{windowsHide:true,detached:true,stdio:'ignore'});await new Promise<void>((resolve,reject)=>{child.once('spawn',resolve);child.once('error',reject)});child.unref();}
      catch(error){runtimeLog.write('update-apply-failed',error);applyUpdate=false;closing=false;await dialog.showMessageBox(window,{type:'error',message:'更新准备失败，请重新启动应用后重试。'});return;}
    }
    quitting = true; app.quit();
  })();
});

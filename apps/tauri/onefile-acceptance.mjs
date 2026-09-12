/** Shared, isolated acceptance helpers for the production one-file executable. */
import {chromium} from 'playwright';
import {readFile} from 'node:fs/promises';
import {createHash} from 'node:crypto';
import {createServer} from 'node:net';
import {execFileSync} from 'node:child_process';
import {join, resolve} from 'node:path';
import assert from 'node:assert/strict';

export const pause = ms => new Promise(resolve => setTimeout(resolve, ms));
export const digest = bytes => createHash('sha256').update(bytes).digest('hex');
export async function inspectOnefile(path) {
  const bytes = await readFile(path), footer = bytes.subarray(-56);
  assert.equal(footer.subarray(0, 16).toString('ascii'), 'NIOH3_ONEFILE_V1');
  const size = Number(footer.readBigUInt64LE(16));
  assert(size > 0 && size < bytes.length - 56);
  const payload = bytes.subarray(bytes.length - 56 - size, -56);
  assert.equal(digest(payload), footer.subarray(24).toString('hex'));
  return {bytes, payload, sha256: digest(bytes), payloadSha256: digest(payload)};
}
export function executable() {
  assert(process.env.NIOH3_ONEFILE_EXE, 'Set NIOH3_ONEFILE_EXE to the built install-free EXE');
  return resolve(process.env.NIOH3_ONEFILE_EXE);
}
export async function isolatedEnvironment(root) {
  const server = createServer();
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  const profile = join(root, 'profile');
  const env = {...process.env, LOCALAPPDATA: join(root, 'local'), NIOH3_STATE_ROOT: join(root, 'state'),
    NIOH3_TAURI_TEST_ROOT: profile, NIOH3_TAURI_TEST_DEBUG_PORT: String(port)};
  // The real launcher, rather than a test shell, must set its process context.
  for (const key of ['NIOH3_ONEFILE_EXE', 'NIOH3_ONEFILE_PID', 'NIOH3_ONEFILE_PAYLOAD_SHA256']) delete env[key];
  return {env, profile, port};
}
export async function connect(port, child) {
  for (let index = 0; index < 200; index++) {
    if (child?.exitCode !== undefined && child.exitCode !== null) throw Error(`Launcher exited ${child.exitCode}`);
    try {if ((await fetch(`http://127.0.0.1:${port}/json/version`)).ok) break;} catch {}
    await pause(300);
  }
  const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`);
  let page;
  for (let index = 0; index < 150; index++) {
    page = browser.contexts()[0]?.pages()[0];
    if (page) break;
    await pause(200);
  }
  assert(page, 'WebView2 opened without a page target');
  await page.getByText('后端已连接，请选择筛选条件。', {exact:true}).waitFor({timeout:45000});
  const diagnostics = await page.evaluate(() => window.support.diagnostics());
  assert.equal(diagnostics.packageVerification.ok, true, JSON.stringify(diagnostics.packageVerification));
  let ready;
  for (let index = 0; index < 100; index++) {
    ready = await page.evaluate(() => window.review.update({action:'status', channel:'stable'}));
    if (ready.canApply) break;
    await pause(100);
  }
  assert.equal(ready.canApply, true, 'Startup handshake and update cleanup did not finish');
  return {browser, page, diagnostics};
}
export async function closeSession(session, child) {
  if (session) {
    await session.page.evaluate(() => window.review.windowAction('close')).catch(() => {});
    await Promise.race([session.browser.close().catch(() => {}), pause(3000)]);
  }
  if (child) {
    for (let index = 0; index < 100 && child.exitCode === null; index++) await pause(100);
    assert.notEqual(child.exitCode, null, 'Launcher did not release its runtime lease after app exit');
    assert.equal(child.exitCode, 0);
  }
}
export function registrySnapshot() {
  // Read installation records in both views; never write or remove registry data.
  const script = `$ErrorActionPreference='Stop'; $rows=@();
    foreach($hive in @([Microsoft.Win32.RegistryHive]::CurrentUser,[Microsoft.Win32.RegistryHive]::LocalMachine)){
      foreach($view in @([Microsoft.Win32.RegistryView]::Registry64,[Microsoft.Win32.RegistryView]::Registry32)){
        $base=[Microsoft.Win32.RegistryKey]::OpenBaseKey($hive,$view)
        try{$key=$base.OpenSubKey('Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall')
          if($null -ne $key){try{foreach($name in $key.GetSubKeyNames()){
            $item=$key.OpenSubKey($name); if($null -eq $item){continue}
            try{$rows+=@{hive="$hive";view="$view";key=$name;name=$item.GetValue('DisplayName');path=$item.GetValue('InstallLocation');uninstall=$item.GetValue('UninstallString')}}finally{$item.Dispose()}
          }}finally{$key.Dispose()}}
        }finally{$base.Dispose()}
      }
    }; ConvertTo-Json -InputObject @($rows | Sort-Object hive,view,key) -Compress -Depth 4`;
  return digest(Buffer.from(execFileSync('powershell.exe', ['-NoProfile', '-NonInteractive', '-Command', script],
    {windowsHide:true, encoding:'utf8', timeout:15000}).trim()));
}

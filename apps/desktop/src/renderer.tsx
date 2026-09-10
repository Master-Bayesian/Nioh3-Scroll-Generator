import React, { useEffect, useState, useSyncExternalStore } from 'react';
import { createRoot } from 'react-dom/client';
import { LanguageProvider, useLanguage } from './language';
import { localizedError, type MessageKey } from './locales';
import { OperationsWorkbench } from './operations-workbench';
import { SearchController, terminal } from './search-controller';
import type { StartParams } from './worker-client';
import './api';
import './support-api';
import './renderer.css';

const controller = new SearchController(window.nioh);
const initialQuery: StartParams['query'] = {
  playthrough: 3, rarity: 4, level: 180,
  primary_effect_ids: [44634], required_secondary_ids: [], required_secondary_id_groups: [],
  grace_effect_id: null, minimum_roll_percent_by_effect_id: [],
  auxiliary: { required_terrain_effect_keys: [], required_terrain_effect_key_groups: [],
    required_special_rule_keys: [], required_special_rule_key_groups: [],
    required_enemy_lookup_keys: [], required_enemy_lookup_key_groups: [] },
};
function Workbench() {
  const { locale, setLocale, t, error: preferenceError } = useLanguage();
  const [names, setNames] = useState<Record<number, string>>({});
  const [catalogError, setCatalogError] = useState('');
  const state = useSyncExternalStore(controller.subscribe, controller.getSnapshot);
  const [draft, setDraft] = useState(JSON.stringify(initialQuery, null, 2));
  const [count, setCount] = useState(2);
  const [budget, setBudget] = useState(1000000);
  const [allowCpu, setAllowCpu] = useState(false);
  const [parseError, setParseError] = useState('');
  const [supportStatus, setSupportStatus] = useState('');
  useEffect(() => { void controller.connect(); return () => controller.dispose(); }, []);
  useEffect(() => {
    let current = true;
    if (state.handshake) void window.nioh.searchCatalog(state.submitted?.query.rarity ?? 4, locale).then(catalog => {
      if (current) { setNames(Object.fromEntries([...catalog.ordinary_effects, ...catalog.grace_effects].map(effect => [effect.effect_id, effect.name]))); setCatalogError(''); }
    }).catch(error => { if (current) { setNames({}); setCatalogError(String(error)); } });
    return () => { current = false; };
  }, [locale, state.handshake, state.submitted?.query.rarity]);
  const active = state.busy || !!(state.job && !terminal(state.job));
  async function search() {
    setParseError('');
    try {
      await controller.start({ query: JSON.parse(draft), context_digest: state.handshake!.context.context_digest,
        result_count: count, page_trials: 100000, job_trials: budget, allow_cpu_fallback: allowCpu, resume_token: null });
    } catch (error) { setParseError(String(error)); }
  }
  return <main>
    <h1>{t('title')}</h1>
    <p>{t('intro')}</p>
    <p>MasterBayesian &amp; Saber_Li</p>
    <button onClick={() => void window.support.exportDiagnostics().then(result =>
      setSupportStatus(t(result.saved ? 'diagnosticsSaved' : 'diagnosticsCancelled'))).catch(error => setSupportStatus(localizedError(locale, error)))}>{t('exportDiagnostics')}</button>
    {supportStatus && <p role="status">{supportStatus}</p>}
    <label htmlFor="language-select">{t('language')}</label>
    <select id="language-select" value={locale} onChange={event => void setLocale(event.target.value as typeof locale)}>
      <option value="zh-CN">简体中文</option><option value="en-US">English</option><option value="ja-JP">日本語</option>
    </select>
    {preferenceError && <p role="alert">{t('preferencesFailed')}</p>}
    {catalogError && <p role="alert">{t('catalogFailed')}</p>}
    <section aria-label={t('worker')}>
      <h2>{t('worker')}</h2>
      <p role="status">{state.handshake ? t('ready') : state.busy ? t('connecting') : t('disconnected')}</p>
      <button onClick={() => void controller.connect(true)} disabled={state.busy}>{t('restart')}</button>
      {state.handshake && <details><summary>{t('capabilities')}</summary><pre>{JSON.stringify(state.handshake, null, 2)}</pre></details>}
    </section>
    <section aria-label={t('queryTitle')}>
      <h2>{t('queryTitle')}</h2>
      <p>{t('queryHint')}</p>
      <label htmlFor="query">{t('queryJson')}</label>
      <textarea id="query" value={draft} onChange={event => setDraft(event.target.value)} spellCheck={false} />
      <div className="controls">
        <label>{t('results')} <input type="number" min="1" max="100" value={count} onChange={event => setCount(Number(event.target.value))} /></label>
        <label>{t('budget')} <input type="number" min="1" max="4294967296" value={budget} onChange={event => setBudget(Number(event.target.value))} /></label>
        <label><input type="checkbox" checked={allowCpu} onChange={event => setAllowCpu(event.target.checked)} />{t('cpu')}</label>
      </div>
      <div className="controls">
        <button onClick={() => void search()} disabled={active || !state.handshake}>{t('start')}</button>
        <button onClick={() => void controller.cancel()} disabled={!state.job || terminal(state.job)}>{t('cancel')}</button>
        <button onClick={() => void controller.resume()} disabled={active || !state.job?.resume_token}>{t('resume')}</button>
      </div>
      {(parseError || state.error) && <div role="alert"><p>{localizedError(locale, parseError || state.error)}</p><details><summary>{t('diagnostics')}</summary>{parseError || state.error}</details></div>}
    </section>
    <section aria-label={t('candidates')}>
      <h2>{t('candidates')}</h2>
      {state.job && <>
        <p data-testid="job-state" data-state={state.job.state} role="status">{t(state.job.state)} · {t((state.job.stop_reason ?? 'working') as MessageKey)} · {t('candidateCount', { count: state.job.candidates.length })}</p>
        <p>{t('cursor', { cursor: state.job.cursor, time: state.job.elapsed_ms })}</p>
        {state.job.state === 'cancel_requested' && <p>{t('cancelPending')}</p>}
        {state.job.error && <div role="alert"><p>{localizedError(locale, state.job.error.code)}</p><details><summary>{t('diagnostics')}</summary>{state.job.error.code}: {state.job.error.message}</details></div>}
        <details><summary>{t('progress')}</summary><pre>{JSON.stringify(state.job.progress, null, 2)}</pre></details>
        <table><thead><tr><th>{t('seed')}</th><th>{t('rarity')}</th><th>{t('stage')}</th><th>{t('effects')}</th></tr></thead>
          <tbody>{state.job.candidates.map(candidate => <tr key={candidate.candidate_id}><td>{candidate.seed}</td><td>{candidate.rarity}</td><td>{t(candidate.record_stage as MessageKey)}</td><td>{candidate.effects.map(effect => `${names[effect.effect_id] ?? ''} · ${effect.effect_id.toString(16).toUpperCase()} : ${new Intl.NumberFormat(locale).format(effect.value)}`).join(' / ')}</td></tr>)}</tbody>
        </table>
        <p>{t('evidence')}</p>
      </>}
    </section>
    <OperationsWorkbench />
  </main>;
}
createRoot(document.getElementById('root')!).render(<LanguageProvider><Workbench /></LanguageProvider>);

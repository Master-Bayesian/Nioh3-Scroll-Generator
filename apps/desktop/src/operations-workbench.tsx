import { useLanguage } from './language';
import { localizedError } from './locales';
import { useEffect, useMemo, useState, useSyncExternalStore } from 'react';
import type { PublicOperation } from './operations-api';
import { OperationController, type OperationRole } from './operation-controller';

/** Temporary controls consume the same framework-independent observer as final UI. */
export function OperationsWorkbench() {
  const { locale, t } = useLanguage();
  const controllers = useMemo(() => ({ save: new OperationController('save', window.operations),
    runtime: new OperationController('runtime', window.operations) }), []);
  const save = useSyncExternalStore(controllers.save.subscribe, controllers.save.getSnapshot);
  const runtime = useSyncExternalStore(controllers.runtime.subscribe, controllers.runtime.getSnapshot);
  const [role, setRole] = useState<OperationRole>('save');
  const [draft, setDraft] = useState(JSON.stringify({ method: 'save.discover', params: {} }, null, 2));
  const [parseError, setParseError] = useState('');
  const state = role === 'save' ? save : runtime;
  const controller = controllers[role];
  useEffect(() => {
    void controllers.save.connect(); void controllers.runtime.connect();
    return () => { controllers.save.dispose(); controllers.runtime.dispose(); };
  }, [controllers]);
  function execute() {
    setParseError('');
    try {
      const command = JSON.parse(draft) as PublicOperation;
      const target = command.method.startsWith('save.') ? 'save' : 'runtime';
      setRole(target);
      void controllers[target].start(() => window.operations.execute(command),
        command.method === 'save.commit' || command.method === 'save.operation' ? command.params.plan_id : null);
    } catch (failure) { setParseError(String(failure)); }
  }
  const error = parseError || state.error;
  return <section aria-label={t('operations')}>
    <h2>{t('operations')}</h2>
    <p>{t('saveHint')}</p><p>{t('nativeHint')}</p>
    <button disabled={!controllers.save.canStart()} onClick={() => {
      setRole('save'); void controllers.save.start(() => window.operations.selectSave());
    }}>{t('selectSave')}</button>
    <label htmlFor="operation-command">{t('command')}</label>
    <textarea id="operation-command" value={draft} onChange={event => setDraft(event.target.value)} spellCheck={false} />
    <button onClick={execute}>{t('execute')}</button>
    <button onClick={() => { setRole('save'); void controllers.save.recover(); }}>{t('recoverSave')}</button>
    <button onClick={() => { setRole('runtime'); void controllers.runtime.recover(); }}>{t('recoverRuntime')}</button>
    <button disabled={state.phase !== 'running' || !state.job?.cancellable} onClick={() => void controller.cancel()}>{t('cancelOperation')}</button>
    {state.operationId && <button onClick={() => void controller.inspectReceipt()}>{t('inspectReceipt')}</button>}
    {state.phase === 'interrupted' && <p role="status">{t('operationUnknown')}</p>}
    {error && <div role="alert"><p>{localizedError(locale, error)}</p><details><summary>{t('diagnostics')}</summary>{error}</details></div>}
    <pre aria-live="polite" data-testid="operation-state" data-phase={state.phase}>
      {JSON.stringify({ role, ...state }, null, 2)}
    </pre>
  </section>;
}

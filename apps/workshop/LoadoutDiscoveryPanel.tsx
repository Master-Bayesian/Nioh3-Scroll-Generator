import React from 'react';
import { Notice } from './Notice';
import type { LoadoutDiscoveryData, LoadoutDiscoveryState } from './loadout-discovery-state';

type PendingDiscovery = Exclude<LoadoutDiscoveryState<LoadoutDiscoveryData>, { kind: 'Ready' }>;

/** Render every discovery outcome explicitly;empty content never looks pending. */
export function LoadoutDiscoveryPanel({ state, retry }: { state: PendingDiscovery; retry: () => void }) {
  let outcome: React.ReactNode;
  switch (state.kind) {
    case 'Loading': outcome = <p role="status">正在载入配装资料…</p>; break;
    case 'Failed': outcome = <>
      <p>暂时无法读取配装资料。</p>
      <Notice text={state.message} tone="error" />
      <button className="primary" onClick={retry}>重新读取配装资料</button>
    </>; break;
    case 'Absent': outcome = <>
      <p role="status">当前安装中没有可用的配装资料。</p>
      <button className="primary" onClick={retry}>重新读取配装资料</button>
    </>; break;
    case 'Idle': outcome = <>
      <p role="status">尚未读取配装资料。</p>
      <button className="primary" onClick={retry}>重新读取配装资料</button>
    </>; break;
  }
  return <main className="equipment-page loadout-page" data-testid="loadout-discovery-panel" data-state={state.kind}>
    <section className="character-hint"><h2>配装优化</h2>{outcome}</section>
  </main>;
}

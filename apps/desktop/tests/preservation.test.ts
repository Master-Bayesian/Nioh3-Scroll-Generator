import { test } from 'node:test';
import assert from 'node:assert/strict';
import { execFile } from 'node:child_process';
import { promisify } from 'node:util';
import { mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve, join } from 'node:path';
import { ProtectedClient } from '../src/protected-client';
import { OperationController } from '../src/operation-controller';
import { SaveSession, saveGateway, editFromEntry } from '../src/save-session';
import { publicCurrentJob, requirePublicJob } from '../src/public-operation-jobs';
import type { OperationsApi } from '../src/operations-api';
import type { SaveReference } from '../../../packages/contracts/protected-responses';

const EMPTY_EFFECT_ID = 0xFFFFFFFF;

/** The seven slot identities as the shipped reader reports them. */
function effectIds(entry: { effects: { effect_id: number }[] }): number[] {
  return entry.effects.map(effect => effect.effect_id);
}

/** Every effect-slot field, which must not move during an unrelated edit. */
function effectRegion(entry: { effects: unknown }): string {
  return JSON.stringify(entry.effects);
}

test('extra-affix and over-bound records survive inspect and an unrelated edit', { timeout: 120000 }, async () => {
  const directory = await mkdtemp(join(tmpdir(), 'nioh3-preserve-'));
  const previous = process.env.NIOH3_STATE_ROOT;
  process.env.NIOH3_STATE_ROOT = join(directory, 'state');
  let client: ProtectedClient | undefined;
  let observer: OperationController | undefined;
  try {
    const fixture = await promisify(execFile)(process.env.NIOH3_PYTHON || 'python', [
      resolve('apps/desktop/tests/fixtures/create-preservation-save.py'), join(directory, 'fixture')], { windowsHide: true });
    const { path } = JSON.parse(fixture.stdout);
    const executable = process.env.NIOH3_PROTECTED_WORKER_EXE;
    client = new ProtectedClient(resolve('.'), executable || process.env.NIOH3_PYTHON || 'python', 'save', !!executable);
    const reference = await client.run('save.register', { path }) as unknown as SaveReference;
    const api: Pick<OperationsApi, 'execute' | 'current' | 'snapshot' | 'cancel' | 'prepareInstall'> = {
      execute: command => client!.call(command.method, command.params),
      current: async () => publicCurrentJob((await client!.call('job.current', {})).job),
      snapshot: async (_role, jobId) => requirePublicJob(await client!.call('job.snapshot', { job_id: jobId })),
      cancel: async (_role, jobId) => requirePublicJob(await client!.call('job.cancel', { job_id: jobId })),
      prepareInstall: async () => { throw new Error('Not used by the preservation workflow'); },
    };
    observer = new OperationController('save', api, 5);
    const session = new SaveSession(saveGateway(api as OperationsApi, observer));
    await session.select(reference);

    const inventory = session.getSnapshot().inventory!;
    assert.equal(inventory.entries.length, 2, 'the fixture holds exactly two mapped records');
    const legacy = inventory.entries[0];
    const sixEffect = inventory.entries[1];

    // Inspection reports both records without rewriting them. The stored raw is
    // 1400; the host canonicalizes only for the *display* value, so the derived
    // display reflects the current bound while `recommended_raw_level` and the
    // stored header keep the original 1400.
    assert.equal(legacy.header.recommended_level, 1400, 'the stored header is untouched');
    assert.equal(legacy.derived.recommended_raw_level, 1400, 'inspection reports the stored raw');
    assert.equal(legacy.derived.recommended_raw_was_clamped, true);
    assert.equal(legacy.derived.recommended_displayed_level, 356, 'display uses the current bound');
    assert.equal(sixEffect.header.rarity, 5);

    assert.equal(effectIds(sixEffect).filter(id => id !== EMPTY_EFFECT_ID).length, 6,
      'six populated effect slots plus the empty sentinel');
    assert.equal(effectIds(sixEffect).length, 7, 'the record still exposes all seven slots');
    const sixEffectsBefore = effectRegion(sixEffect);

    // An unrelated edit: only the six-effect record's own fields move.
    const edit = editFromEntry(sixEffect);
    edit.header.transfer_count = 7;
    const plan = await session.prepareEdit([edit as never]);
    assert.equal((await session.commit(plan.plan_id)).commit_status, 'committed');

    // The on-disk container changed only where the edit landed; the over-bound
    // record's slot keeps its bytes, and both records survive the round trip.
    await session.refresh();
    const after = session.getSnapshot().inventory!;
    const legacyAfter = after.entries[0];
    const sixAfter = after.entries[1];
    assert.equal(legacyAfter.header.recommended_level, 1400,
      'the over-bound raw survived the unrelated edit');
    assert.equal(legacyAfter.derived.recommended_raw_level, 1400);
    assert.deepEqual(effectIds(legacyAfter), effectIds(legacy), 'legacy effects unchanged');
    assert.equal(effectRegion(sixAfter), sixEffectsBefore,
      'every effect slot field is byte-identical after an unrelated header edit');
    assert.equal(effectIds(sixAfter).filter(id => id !== EMPTY_EFFECT_ID).length, 6);
    assert.equal(sixAfter.header.transfer_count, 7, 'the unrelated edit landed');
  } finally {
    if (previous === undefined) delete process.env.NIOH3_STATE_ROOT;
    else process.env.NIOH3_STATE_ROOT = previous;
    observer?.dispose();
    await client?.close().catch(() => {});
    await rm(directory, { recursive: true, force: true });
  }
});

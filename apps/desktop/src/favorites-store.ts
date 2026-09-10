import {readFile, open, stat, mkdir, rename} from 'node:fs/promises';
import {dirname} from 'node:path';
import type {CandidateTransfer} from '../../../packages/contracts/responses';
import type {Sample} from '../../workshop/model';

export const COLLECTION_LIMIT = 50;
export const sampleKey = (sample: Pick<Sample, 'seed' | 'rarity' | 'playthrough' | 'level'>) =>
  `${sample.playthrough ?? 3}:${sample.rarity}:${sample.level ?? 180}:${sample.seed}`;
type Entry = {sample: Sample; transfer: CandidateTransfer};

/** Private records stay in the broker. Public snapshots contain display data only. */
export class FavoritesStore {
  private queue: Promise<unknown> = Promise.resolve();
  constructor(private path: string) {}
  private async read(): Promise<Entry[]> {
    let raw: string;
    try { if((await stat(this.path)).size>4_000_000)throw Error('FAVORITES_FILE_INVALID');raw = await readFile(this.path, 'utf8'); }
    catch (error) { if ((error as NodeJS.ErrnoException).code === 'ENOENT') return []; throw error; }
    if (raw.length > 4_000_000) throw Error('FAVORITES_FILE_INVALID');
    const value = JSON.parse(raw);
    if (value.version !== 1 || !Array.isArray(value.entries) || value.entries.length > COLLECTION_LIMIT)
      throw Error('FAVORITES_FILE_INVALID');
    for (const entry of value.entries) this.validate(entry.sample, entry.transfer);
    return value.entries;
  }
  private validate(sample: Sample, transfer: CandidateTransfer) {
    if (!sample || !transfer || String(transfer.seed) !== sample.seed || transfer.rarity !== sample.rarity ||
        transfer.playthrough !== (sample.playthrough ?? 3) || transfer.level !== (sample.level ?? 180) ||
        typeof transfer.candidate_id !== 'string' || !Array.isArray(sample.effects) ||
        !Array.isArray(sample.rules) || !Array.isArray(sample.enemies) ||
        JSON.stringify({sample, transfer}).length > 70_000) throw Error('FAVORITE_INVALID');
  }
  async list() { await this.queue; return this.read(); }
  private mutate(change: (entries: Entry[]) => Entry[]) {
    const work = this.queue.then(async () => {
      const entries = change(await this.read());
      await mkdir(dirname(this.path), {recursive: true});
      const file=await open(this.path + '.tmp','w');
      try {await file.writeFile(JSON.stringify({version: 1, entries}),'utf8');await file.sync()}
      finally {await file.close()}
      await rename(this.path + '.tmp', this.path);
      return entries;
    });
    this.queue = work.catch(() => {});
    return work;
  }
  add(sample: Sample, transfer: CandidateTransfer) {
    this.validate(sample, transfer);
    const publicSample = structuredClone(sample);
    delete publicSample.backend;
    delete publicSample.saveEntry;
    return this.mutate(entries => {
      const key = sampleKey(publicSample), existing = entries.findIndex(e => sampleKey(e.sample) === key);
      if (existing < 0 && entries.length >= COLLECTION_LIMIT) throw Error('FAVORITES_CAPACITY_REACHED');
      const entry = {sample: publicSample, transfer: structuredClone(transfer)};
      if (existing >= 0) entries[existing] = entry; else entries.push(entry);
      return entries;
    });
  }
  remove(key: string) { return this.mutate(entries => entries.filter(e => sampleKey(e.sample) !== key)); }
}

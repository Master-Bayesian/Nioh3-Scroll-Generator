import { mkdir, readFile, rename, writeFile } from 'node:fs/promises';
import { dirname } from 'node:path';
import { isLocale, normalizeLocale, type Locale } from './locales';

/** Small stable user-data file; independent of the installed app directory. */
export class PreferencesStore {
  private pending: Promise<unknown> = Promise.resolve();
  constructor(private path: string, private systemLocale: string) {}
  async getLocale(): Promise<Locale> {
    await this.pending.catch(() => undefined);
    try {
      const value = JSON.parse(await readFile(this.path, 'utf8'));
      return value.schema === 1 && isLocale(value.locale) ? value.locale : normalizeLocale(this.systemLocale);
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT' || error instanceof SyntaxError) return normalizeLocale(this.systemLocale);
      throw error;
    }
  }
  setLocale(locale: unknown): Promise<Locale> {
    if (!isLocale(locale)) return Promise.reject(new Error('INVALID_REQUEST: unsupported locale'));
    const operation = this.pending.catch(() => undefined).then(async () => {
      await mkdir(dirname(this.path), { recursive: true });
      const temporary = `${this.path}.${process.pid}.tmp`;
      await writeFile(temporary, JSON.stringify({ schema: 1, locale }) + '\n', 'utf8');
      await rename(temporary, this.path);
      return locale;
    });
    this.pending = operation;
    return operation;
  }
}

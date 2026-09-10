import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from 'react';
import { normalizeLocale, translate, type Locale, type MessageKey } from './locales';
export interface PreferencesApi { getLocale(): Promise<Locale>; setLocale(locale: Locale): Promise<Locale> }
declare global { interface Window { preferences: PreferencesApi } }
const LanguageContext = createContext({ locale: 'en-US' as Locale, setLocale: async (_locale: Locale) => {}, error: '' });
export function LanguageProvider({ children }: { children: ReactNode }) {
  const [locale, setValue] = useState<Locale>(normalizeLocale(navigator.language));
  const [error, setError] = useState('');
  const revision = useRef(0);
  useEffect(() => {
    let active = true;
    const initialRevision = revision.current;
    void window.preferences.getLocale().then(value => {
      if (active && revision.current === initialRevision) setValue(value);
    }).catch(error => {
      if (active && revision.current === initialRevision) setError(String(error));
    });
    return () => { active = false; };
  }, []);
  useEffect(() => { document.documentElement.lang = locale; }, [locale]);
  async function setLocale(value: Locale) {
    const current = ++revision.current;
    try {
      const saved = await window.preferences.setLocale(value);
      if (revision.current === current) { setValue(saved); setError(''); }
    } catch (error) { if (revision.current === current) setError(String(error)); }
  }
  return <LanguageContext.Provider value={{ locale, setLocale, error }}>{children}</LanguageContext.Provider>;
}
export function useLanguage() {
  const language = useContext(LanguageContext);
  return { ...language, t: (key: MessageKey, values?: Record<string, string | number>) => translate(language.locale, key, values) };
}

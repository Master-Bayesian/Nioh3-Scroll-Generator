import React, { useEffect, useRef, useState } from "react";
import type {
  LocalNameCatalogSource,
} from "../../packages/contracts/protected-responses";
import { currentLocale, type UiLocale } from "./presentation";
import { desktop } from "./desktop-bridge";

/** The protected adapter's hard input limit. Keep this in sync with the
 * contract boundary; the renderer must reject before reading the file. */
export const LOCAL_CATALOG_MAX_BYTES = 2 * 1024 * 1024;
const LOCAL_CATALOG_STORAGE_KEY = "nioh3-equipment-local-name-catalog-v1";
const LOCAL_CATALOG_STORAGE_MAX_CHARS = 512 * 1024;
const LOCAL_CATALOG_SCHEMA = "nioh3-local-name-catalog-v1";
const SAVE_ITEM_NAMESPACE = "save_item_u16_le_bytes";
const CATALOG_ROLE = "save_active_items";
const LOCALES: UiLocale[] = ["zh-CN", "en-US", "ja-JP"];

export interface ActiveLocalCatalog {
  source: LocalNameCatalogSource;
  entries: ReadonlyMap<number, string>;
  displayRows: number;
  claimBoundary: string;
}

interface PersistedCatalog {
  schema: typeof LOCAL_CATALOG_SCHEMA;
  status: "ok";
  source: LocalNameCatalogSource;
  entries: [number, string][];
  displayRows: number;
  claimBoundary: string;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

function boundedText(value: unknown, max = 256): value is string {
  return (
    typeof value === "string" &&
    value.length > 0 &&
    value.length <= max &&
    !/[\u0000-\u001f\u007f]/.test(value)
  );
}

function isLocale(value: unknown): value is UiLocale {
  return LOCALES.includes(value as UiLocale);
}

function isSource(value: unknown): value is LocalNameCatalogSource {
  if (!isRecord(value)) return false;
  return (
    value.role === CATALOG_ROLE &&
    value.namespace === SAVE_ITEM_NAMESPACE &&
    boundedText(value.source_label) &&
    boundedText(value.declared_version, 128) &&
    isLocale(value.locale) &&
    Number.isSafeInteger(value.bytes) &&
    (value.bytes as number) >= 0 &&
    (value.bytes as number) <= LOCAL_CATALOG_MAX_BYTES &&
    typeof value.sha256 === "string" &&
    /^[0-9a-f]{64}$/i.test(value.sha256)
  );
}

function isSafeEntry(row: unknown, source: LocalNameCatalogSource): row is {
  display_id: number;
  id: number;
  name: string;
} {
  if (!isRecord(row)) return false;
  return (
    row.displayable === true &&
    row.source_role === source.role &&
    row.namespace === source.namespace &&
    row.state === "accepted" &&
    row.quarantine_reason === null &&
    row.high_word === null &&
    Number.isInteger(row.id) &&
    Number.isInteger(row.display_id) &&
    row.id === row.display_id &&
    (row.display_id as number) > 0 &&
    (row.display_id as number) <= 0xffff &&
    typeof row.name === "string" &&
    row.name.trim().length > 0 &&
    row.name.length <= 512
  );
}

function conflictIds(value: unknown): Set<number> {
  const ids = new Set<number>();
  if (!Array.isArray(value)) return ids;
  for (const conflict of value) {
    if (isRecord(conflict) && Number.isInteger(conflict.id))
      ids.add(conflict.id as number);
  }
  return ids;
}

/**
 * Turn the backend result into a display-only map. Every condition is
 * intentional: no sentinel, malformed, high-word, namespace, or conflict row
 * can label a runtime item. The full result remains available to the caller's
 * import receipt, while the UI keeps only this safe projection.
 */
export function activeCatalogFromResult(value: unknown): ActiveLocalCatalog {
  if (!isRecord(value) || value.schema !== LOCAL_CATALOG_SCHEMA || value.status !== "ok")
    throw new Error("CATALOG_UNEXPECTED_RESPONSE");
  if (!isSource(value.source)) throw new Error("CATALOG_INVALID_SOURCE");
  if (!Array.isArray(value.rows) || value.rows.length > 10000)
    throw new Error("CATALOG_INVALID_ROWS");
  const source = value.source;
  const blocked = conflictIds(value.conflicts);
  const entries = new Map<number, string>();
  const duplicateIds = new Set<number>();
  for (const row of value.rows) {
    if (!isSafeEntry(row, source)) continue;
    const id = row.display_id;
    if (blocked.has(id)) continue;
    const previous = entries.get(id);
    if (previous !== undefined && previous !== row.name) {
      duplicateIds.add(id);
      continue;
    }
    entries.set(id, row.name);
  }
  for (const id of duplicateIds) entries.delete(id);
  // Report only the entries that survived the renderer's independent safety
  // projection; backend counts are provenance, not permission to display.
  const displayRows = entries.size;
  const claimBoundary = boundedText(value.claim_boundary, 2048)
    ? value.claim_boundary
    : "Local names only; this does not establish game compatibility or legitimacy.";
  return { source, entries, displayRows, claimBoundary };
}

function persistedCatalog(value: ActiveLocalCatalog): PersistedCatalog {
  return {
    schema: LOCAL_CATALOG_SCHEMA,
    status: "ok",
    source: value.source,
    entries: [...value.entries.entries()],
    displayRows: value.displayRows,
    claimBoundary: value.claimBoundary,
  };
}

function isPersisted(value: unknown): value is PersistedCatalog {
  if (!isRecord(value) || value.schema !== LOCAL_CATALOG_SCHEMA || value.status !== "ok")
    return false;
  if (!isSource(value.source) || !Array.isArray(value.entries) || value.entries.length > 10000)
    return false;
  if (!Number.isSafeInteger(value.displayRows) || (value.displayRows as number) < 0)
    return false;
  if (!boundedText(value.claimBoundary, 2048)) return false;
  const seen = new Set<number>();
  for (const entry of value.entries) {
    if (
      !Array.isArray(entry) ||
      entry.length !== 2 ||
      !Number.isInteger(entry[0]) ||
      entry[0] <= 0 ||
      entry[0] > 0xffff ||
      typeof entry[1] !== "string" ||
      entry[1].trim().length === 0 ||
      entry[1].length > 512 ||
      seen.has(entry[0])
    )
      return false;
    seen.add(entry[0]);
  }
  return true;
}

export function loadStoredLocalCatalog(): ActiveLocalCatalog | null {
  try {
    const raw = localStorage.getItem(LOCAL_CATALOG_STORAGE_KEY);
    if (!raw) return null;
    const parsed: unknown = JSON.parse(raw);
    if (!isPersisted(parsed)) {
      localStorage.removeItem(LOCAL_CATALOG_STORAGE_KEY);
      return null;
    }
    return {
      source: parsed.source,
      entries: new Map(parsed.entries),
      displayRows: parsed.displayRows,
      claimBoundary: parsed.claimBoundary,
    };
  } catch {
    // A corrupt or unavailable browser store must never affect raw-id display.
    return null;
  }
}

function persistLocalCatalog(value: ActiveLocalCatalog): boolean {
  try {
    const encoded = JSON.stringify(persistedCatalog(value));
    if (encoded.length > LOCAL_CATALOG_STORAGE_MAX_CHARS) return false;
    localStorage.setItem(LOCAL_CATALOG_STORAGE_KEY, encoded);
    return true;
  } catch {
    return false;
  }
}

function removeStoredLocalCatalog() {
  try {
    localStorage.removeItem(LOCAL_CATALOG_STORAGE_KEY);
  } catch {
    // Storage can be disabled or full; in-memory removal still succeeds.
  }
}

function bytesToBase64(bytes: Uint8Array): string {
  let text = "";
  const chunk = 0x8000;
  for (let index = 0; index < bytes.length; index += chunk)
    text += String.fromCharCode(...bytes.subarray(index, index + chunk));
  return btoa(text);
}

function sourceLabel(fileName: string): string {
  const parts = fileName.split(/[\\/]/g);
  return parts[parts.length - 1] || "selected-file.json";
}

function localeLabel(locale: UiLocale): string {
  return locale === "zh-CN" ? "简体中文" : locale === "en-US" ? "English" : "日本語";
}

interface LocalCatalogImportProps {
  onCatalogChange: (catalog: ActiveLocalCatalog | null) => void;
}

/** Optional, user-selected local-name read. It never opens a path or writes a save. */
export function LocalCatalogImport({ onCatalogChange }: LocalCatalogImportProps) {
  const [catalog, setCatalog] = useState<ActiveLocalCatalog | null>(() =>
    loadStoredLocalCatalog(),
  );
  const [declaredVersion, setDeclaredVersion] = useState("unknown");
  const [sourceLocale, setSourceLocale] = useState<UiLocale>(() => currentLocale());
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [error, setError] = useState("");
  const request = useRef(0);
  const live = useRef(true);

  useEffect(() => {
    onCatalogChange(catalog);
  }, [catalog, onCatalogChange]);
  useEffect(
    () => () => {
      live.current = false;
      request.current += 1;
    },
    [],
  );

  async function importFile(file: File | undefined, input: HTMLInputElement) {
    input.value = "";
    if (!file) return;
    const token = ++request.current;
    setBusy(true);
    setMessage("");
    setError("");
    try {
      if (!file.name.toLowerCase().endsWith(".json"))
        throw new Error("CATALOG_FILE_TYPE_UNSUPPORTED");
      if (file.size > LOCAL_CATALOG_MAX_BYTES)
        throw new Error("CATALOG_INPUT_TOO_LARGE");
      const bytes = new Uint8Array(await file.arrayBuffer());
      if (bytes.byteLength > LOCAL_CATALOG_MAX_BYTES)
        throw new Error("CATALOG_INPUT_TOO_LARGE");
      if (!desktop || !window.operations)
        throw new Error("DESKTOP_REQUIRED");
      const result = await window.operations.execute({
        method: "catalog.import_names",
        params: {
          role: "save_active_items",
          content_base64: bytesToBase64(bytes),
          source_label: sourceLabel(file.name),
          declared_version: declaredVersion.trim() || "unknown",
          locale: sourceLocale,
        },
      });
      if (token !== request.current || !live.current) return;
      const next = activeCatalogFromResult(result);
      const persisted = persistLocalCatalog(next);
      setCatalog(next);
      setMessage(
        persisted
          ? "本地名称目录已加载。"
          : "本地名称目录已加载，但浏览器存储不可用；本次会话仍可使用。",
      );
    } catch (reason) {
      if (token !== request.current || !live.current) return;
      // Keep the previous catalog on every rejection, including malformed
      // response and storage failures.
      setError(String(reason));
    } finally {
      if (token === request.current && live.current) setBusy(false);
    }
  }

  function removeCatalog() {
    request.current += 1;
    removeStoredLocalCatalog();
    setCatalog(null);
    setBusy(false);
    setError("");
    setMessage("本地名称目录已移除。仅名称显示已恢复为数字 ID。 ");
  }

  return (
    <fieldset className="equipment-catalog-import">
      <legend>本地名称目录（可选）</legend>
      <div className="equipment-catalog-controls">
        <label>
          声明版本
          <input
            className="equipment-catalog-version"
            value={declaredVersion}
            maxLength={128}
            onChange={(event) => setDeclaredVersion(event.target.value)}
          />
        </label>
        <label>
          名称语言
          <select
            value={sourceLocale}
            onChange={(event) => setSourceLocale(event.target.value as UiLocale)}
          >
            {LOCALES.map((locale) => (
              <option value={locale} key={locale}>
                {localeLabel(locale)}
              </option>
            ))}
          </select>
        </label>
        <label className="equipment-catalog-file">
          选择 items_little_endian.json
          <input
            type="file"
            accept=".json,application/json"
            disabled={busy || !desktop}
            onChange={(event) => void importFile(event.currentTarget.files?.[0], event.currentTarget)}
          />
        </label>
        {catalog && (
          <button type="button" onClick={removeCatalog}>
            移除本地目录
          </button>
        )}
      </div>
      <p className="equipment-catalog-help">
        仅读取你选择的 items_little_endian.json（上限 2 MiB）；不读取路径、不写入游戏或存档。
      </p>
      {catalog && (
        <p className="equipment-catalog-disclosure" role="status">
          名称来源：
          {React.createElement(
            "span",
            { className: "equipment-catalog-source-value" },
            catalog.source.source_label,
          )}
          {" · 声明版本 "}
          {React.createElement(
            "span",
            { className: "equipment-catalog-source-value" },
            catalog.source.declared_version,
          )}
          {" · "}
          {localeLabel(catalog.source.locale)}
          {" · "}
          {catalog.displayRows}
          {" 个可显示名称。仅名称显示，不代表游戏兼容性或合法性。"}
        </p>
      )}
      {message && <p className="equipment-status" role="status">{message}</p>}
      {error && <p className="equipment-error">导入失败：{error}</p>}
    </fieldset>
  );
}

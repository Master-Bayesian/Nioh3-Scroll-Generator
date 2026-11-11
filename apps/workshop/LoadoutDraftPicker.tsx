import React, { useEffect, useRef, useState } from "react";
import { PickerDialog } from "./LoadoutPickers";

export function LoadoutDraftPicker({ drafts, activeId, onSelect, onCreate, onRename }: {
  drafts: { id: number; name: string }[];
  activeId: number;
  onSelect: (id: number) => void;
  onCreate: (name: string) => void;
  onRename: (name: string) => void;
}) {
  const [mode, setMode] = useState<"create" | "rename" | null>(null);
  const [name, setName] = useState("");
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => { if (mode) { input.current?.focus(); input.current?.select(); } }, [mode]);
  const duplicate = drafts.some(draft => draft.name === name.trim() && (mode !== "rename" || draft.id !== activeId));
  return <div className="loadout-draft-picker">
    <select aria-label="当前配装方案" title={drafts.find(draft => draft.id === activeId)?.name} value={activeId} onChange={event => onSelect(Number(event.target.value))}>
      {drafts.map(draft => <option key={draft.id} value={draft.id}>{draft.name}</option>)}
    </select>
    <button type="button" onClick={() => { setName(drafts.find(draft => draft.id === activeId)?.name ?? ""); setMode("rename"); }}>重命名</button>
    <button type="button" onClick={() => { setName(`配装${drafts.length + 1}`); setMode("create"); }}>+ 新建配装</button>
    {mode && <PickerDialog title={mode === "rename" ? "重命名配装" : "新建配装"} close={() => setMode(null)}>
      <form className="loadout-new-draft" onSubmit={event => {
        event.preventDefault();
        if (!name.trim() || duplicate) return;
        (mode === "rename" ? onRename : onCreate)(name.trim()); setMode(null);
      }}>
        <label>配装名称<input ref={input} aria-label="配装名称" autoFocus maxLength={60} value={name} onChange={event => setName(event.target.value)} /></label>
        {mode === "create" && <p>创建空白配装，等级从 1 开始。当前配装会保留，可通过顶部列表切回。</p>}
        <p className="loadout-source-note">配装自动保存到本机，刷新或重新打开工具后可继续使用。</p>
        {duplicate && <p role="alert" className="loadout-invalid">已有同名配装，请换一个名称。</p>}
        <footer className="loadout-picker-footer"><button type="button" onClick={() => setMode(null)}>取消</button><button type="submit" disabled={!name.trim() || duplicate}>{mode === "rename" ? "保存名称" : "创建配装"}</button></footer>
      </form>
    </PickerDialog>}
  </div>;
}

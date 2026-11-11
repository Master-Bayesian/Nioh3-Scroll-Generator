import React, {useState} from "react";
import { PrestigeEditor } from "./PrestigeEditor";
import {PlayerResourcesEditor,type ResourceSection} from "./PlayerResourcesEditor";
import "./player-editor.css";

/** Player resources have a separate entry point from equipment and item records. */
export function PlayerEditor({ onBusy }: { onBusy?: (value: boolean) => void }) {
  const [section,setSection]=useState<"prestige"|ResourceSection>("prestige"),[busy,setBusy]=useState(false);
  const activity=(active:boolean)=>{setBusy(active);onBusy?.(active);};
  return <main className="player-editor" aria-label="人物编辑">
    <header className="player-editor-heading">
      <h2>人物编辑</h2>
      <p>修改角色的恩惠点数、储存精华、武功、武家阶级与常世的残思。</p>
    </header>
    <div className="player-editor-sections" role="tablist" aria-label="人物编辑功能">
      <button id="player-prestige-tab" role="tab" aria-selected={section==="prestige"} disabled={busy}
        onClick={()=>setSection("prestige")} aria-controls="player-prestige-panel" className={section==="prestige"?"active":""}>
        恩惠点数<small>九类称号恩惠点数</small>
      </button>
      <button id="player-ingot-tab" role="tab" aria-selected={section==="ingot"} disabled={busy}
        onClick={()=>setSection("ingot")} aria-controls="player-ingot-panel" className={section==="ingot"?"active":""}>
        灵银地金<small>储存的精华</small>
      </button>
      <button id="player-glory-tab" role="tab" aria-selected={section==="glory"} disabled={busy}
        onClick={()=>setSection("glory")} aria-controls="player-glory-panel" className={section==="glory"?"active":""}>
        武功与武家<small>武功余额与武家阶级</small>
      </button>
      <button id="player-fragments-tab" role="tab" aria-selected={section==="fragments"} disabled={busy}
        onClick={()=>setSection("fragments")} aria-controls="player-fragments-panel" className={section==="fragments"?"active":""}>
        常世的残思<small>魂核与守护灵觉醒资源</small>
      </button>
    </div>
    {section==="prestige"?<div id="player-prestige-panel" role="tabpanel" aria-labelledby="player-prestige-tab">
      <PrestigeEditor onBusy={activity} /></div>
      :<div id={"player-"+section+"-panel"} role="tabpanel" aria-labelledby={"player-"+section+"-tab"}>
        <PlayerResourcesEditor section={section} onBusy={activity}/></div>}
  </main>;
}

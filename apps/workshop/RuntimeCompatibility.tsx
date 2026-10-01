import React,{useEffect,useState} from "react";
import {Notice} from "./Notice";
import {errorText} from "./public-errors";
import {desktop} from "./desktop-bridge";
import "../desktop/src/operations-api";
type Report={present?:boolean;warning?:boolean;accepted?:boolean;game_version?:string;executable?:string;backup?:{attempted:boolean;verified:boolean;paths:string[];error?:string};features?:{live_character:boolean;live_equipment_add:boolean}};
/** Consent is stored by the native host and bound to this process lifetime. */
export function RuntimeCompatibility(){
 const [report,setReport]=useState<Report|null>(null),[open,setOpen]=useState(false),[busy,setBusy]=useState(false),[message,setMessage]=useState("");
 const [risk,setRisk]=useState(false),[backed,setBacked]=useState(false);
 async function action(kind:"inspect"|"prepare"|"accept"){
  setBusy(true);setMessage("");
  try{
   const result=await window.operations.execute({method:"runtime.compatibility",params:{action:kind,...(kind==="accept"?{confirmed:risk,backup_confirmed:backed}:{})}});
   if(!("compatibility" in result))throw Error("UNEXPECTED_COMPATIBILITY_RESULT");
   const next=result.compatibility as Report;setReport(next);
   if(kind==="accept"){setOpen(false);window.dispatchEvent(new Event("nioh3:compatibility-accepted"));}
  }catch(error){setMessage(errorText(error));}finally{setBusy(false);}
 }
 useEffect(()=>{
  if(!desktop)return;
  void action("inspect");
  const show=()=>{setOpen(true);setRisk(false);setBacked(false);void action("prepare");};
  const detected=(event:Event)=>setReport((event as CustomEvent<Report>).detail);
  window.addEventListener("nioh3:compatibility-required",show);
  window.addEventListener("nioh3:compatibility-detected",detected);
  return()=>{window.removeEventListener("nioh3:compatibility-required",show);window.removeEventListener("nioh3:compatibility-detected",detected);};
 },[]);
 if(!desktop)return null;
 return <>
  {report?.warning&&!report.accepted&&<aside className="runtime-compatibility-banner">
   <span>检测到旧版本或不同的游戏程序。请核对兼容范围与存档备份，再继续实时操作。</span>
   <button onClick={()=>{setOpen(true);setRisk(false);setBacked(false);void action("prepare");}}>查看兼容提示</button>
  </aside>}
  {open&&<dialog open className="compatibility-dialog">
   <header><h2>游戏版本兼容提示</h2><button onClick={()=>setOpen(false)}>关闭</button></header>
   <div className="compatibility-body">
    <p>这份游戏程序与已验证的版本不完全相同，继续操作可能出错。请先备份存档，并确认备份可用。</p>
    {report?.game_version&&<p>游戏版本：{report.game_version}</p>}
    {report?.executable&&<code>{report.executable}</code>}
    {report?.backup?.verified?<><p>自动备份已完成，并已回读核对。请确认下面的备份包含你的当前存档。</p>{report.backup.paths.map(path=><code key={path}>{path}</code>)}</>:<>
     <p>自动备份没有完成。请先手动备份存档，再确认继续。</p>
     {report?.backup?.error&&<code>{report.backup.error}</code>}
    </>}
    {report?.features&&!report.features.live_equipment_add&&<p>此旧版本可以使用已有的绘卷搜索和存档功能；实时添加装备尚未完成版本核对。</p>}
    <label><input data-action="compatibility-risk" type="checkbox" checked={risk} onChange={event=>setRisk(event.target.checked)}/>我已了解版本差异和可能出错的风险</label>
    <label><input data-action="compatibility-backup" type="checkbox" checked={backed} onChange={event=>setBacked(event.target.checked)}/>我已确认当前存档有可用备份</label>
    <Notice text={message}/>
   </div>
   <footer><button onClick={()=>void action("prepare")} disabled={busy}>重试自动备份</button><button data-action="compatibility-accept" onClick={()=>void action("accept")} disabled={busy||!risk||!backed||!report?.present}>确认使用兼容模式</button></footer>
  </dialog>}
 </>;
}

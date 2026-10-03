import React, {useEffect, useMemo, useRef, useState} from "react";
import type {EquipmentAddition} from "../../packages/contracts/protected-responses";
import {ADD_CATALOG,ADD_TYPES,FacetFilter,MAJOR_ORDER} from "./equipment-facets";
import {data} from "./model";
import {plainGameText} from "./game-text";
import {Notice} from "./Notice";
import {errorText} from "./public-errors";
import {runtimeObserver} from "./save-workspace";
import "../desktop/src/operations-api";

type Addition=EquipmentAddition["equipment_add"];
const STORAGE="nioh3-live-equipment-add";
const catalog=ADD_CATALOG;
const effectName=(id:number)=>data.editorEffects.find(row=>row.id===String(id))?.name||`0x${id.toString(16).toUpperCase()}`;
// This catalog has Chinese item names only. Keep exact game names instead of
// translating their substrings as unrelated interface terms.
const itemName=(id:number)=>React.createElement("span",{lang:"zh-CN"},plainGameText(catalog.find(row=>row.id===id)?.name||String(id)));
function storedOperation():string|null {
  const value=localStorage.getItem(STORAGE);
  if(!value)return null;
  const parsed=JSON.parse(value) as {operation_id?:string};
  if(!parsed.operation_id || !/^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(parsed.operation_id))throw Error("INVALID_EQUIPMENT_OPERATION");
  return parsed.operation_id;
}
function integer(value:string,min:number,max:number):number|null {
  if(!/^\d+$/.test(value))return null;
  const number=Number(value);return Number.isInteger(number)&&number>=min&&number<=max?number:null;
}

/** Durable UI ownership starts before prepare. Restoring never inserts again. */
export function LiveEquipmentAdd({onBusy,onAdded}:{onBusy?:(busy:boolean)=>void;onAdded?:()=>void}) {
  const [query,setQuery]=useState(""),[kind,setKind]=useState(""),[type,setType]=useState(""),[school,setSchool]=useState("");
  const [item,setItem]=useState<number|null>(null);
  const [level,setLevel]=useState("180"),[plus,setPlus]=useState("20"),[rarity,setRarity]=useState("4"),[seed,setSeed]=useState("0");
  const [backupPath,setBackupPath]=useState("");
  const [recoveryMessage,setRecoveryMessage]=useState("");
  const [operation,setOperation]=useState<string|null>(null),[result,setResult]=useState<Addition|null>(null);
  const [busy,setBusy]=useState(false),[confirmed,setConfirmed]=useState(false),[message,setMessage]=useState("");
  const mounted=useRef(false),running=useRef(false),current=useRef<string|null>(null);
  const callbacks=useRef({onBusy,onAdded});callbacks.current={onBusy,onAdded};
  const rows=useMemo(()=>catalog.filter(row=>(!kind||row.major===kind)&&(!type||row.type===type)&&(!school||row.school===school)&&(!query||(`${row.name} ${row.id} ${row.major} ${row.type}`).toLowerCase().includes(query.toLowerCase()))),[query,kind,type,school]);
  const numbers={level:integer(level,1,65535),plus:integer(plus,0,65535),rarity:integer(rarity,0,5),seed:integer(seed,0,65535)};
  const valid=item!==null&&Object.values(numbers).every(value=>value!==null);
  const locked=busy||operation!==null;
  const terminal=result&&["verified","cancelled","rejected_before_dispatch","rejected_before_insertion"].includes(result.state);

  async function request(action:"prepare"|"execute"|"status"|"recover"|"cancel",id:string) {
    if(running.current)return;
    running.current=true;setBusy(true);callbacks.current.onBusy?.(true);setMessage("");setRecoveryMessage("");
    try {
      const submit=()=>action==="prepare" ? window.operations.execute({method:"runtime.equipment_add_prepare",params:{operation_id:id,item_id:item!,level:numbers.level!,plus:numbers.plus!,rarity:numbers.rarity!,seed:numbers.seed!,...(backupPath.trim()?{save_path:backupPath.trim()}:{})}})
        : action==="execute" ? window.operations.execute({method:"runtime.equipment_add_execute",params:{operation_id:id,plan_digest:result!.plan_digest!,confirmed:true}})
        : window.operations.execute({method:action==="status"?"runtime.equipment_add_status":action==="recover"?"runtime.equipment_add_recover":"runtime.equipment_add_cancel",params:{operation_id:id}});
      // The shared observer handles jobs; this component owns its durable
      // equipment UUID and explicitly queries its equipment receipt on recovery.
      const reply=runtimeObserver?await runtimeObserver.run(submit):await submit();
      if(!reply||!("equipment_add" in reply)||reply.equipment_add.operation_id!==id)throw Error("EQUIPMENT_ADDITION_RESULT_EXPECTED");
      if(!mounted.current||current.current!==id)return;
      setResult(reply.equipment_add);setConfirmed(false);
      if(reply.equipment_add.error)setMessage(reply.equipment_add.error);
      if(action==="execute"&&reply.equipment_add.state==="verified")callbacks.current.onAdded?.();
    }catch(error){
      if(mounted.current&&current.current===id){
        setConfirmed(false);setResult(previous=>({...previous,operation_id:id,state:"uncertain",plan_digest:previous?.plan_digest??null,process_id:previous?.process_id??0,slot_index:previous?.slot_index??null,preview:previous?.preview??null,error:null}));
        setMessage(errorText(error));
        if(action!=="status"){
          // Resolve the same UUID once by reading its receipt. Never replay a
          // prepare or execute request whose response failed.
          try{
            const inspect=()=>window.operations.execute({method:"runtime.equipment_add_status",params:{operation_id:id}});
            const checked=runtimeObserver?await runtimeObserver.run(inspect):await inspect();
            if(!checked||!("equipment_add" in checked)||checked.equipment_add.operation_id!==id)throw Error("EQUIPMENT_ADDITION_RESULT_EXPECTED");
            if(!mounted.current||current.current!==id)return;
            setResult(checked.equipment_add);
            setRecoveryMessage(checked.equipment_add.error||"已读取本次操作状态，未重新执行添加。");
            if(action==="execute"&&checked.equipment_add.state==="verified")callbacks.current.onAdded?.();
          }catch(statusError){
            if(mounted.current&&current.current===id)setRecoveryMessage("状态核对失败："+errorText(statusError));
          }
        }
      }
    }finally{
      running.current=false;if(mounted.current)setBusy(false);callbacks.current.onBusy?.(false);
    }
  }
  useEffect(()=>{
    mounted.current=true;
    try {const id=storedOperation();if(id){current.current=id;setOperation(id);void request("status",id);}}
    catch(error){setMessage(errorText(error));}
    return ()=>{mounted.current=false;callbacks.current.onBusy?.(false);};
  },[]);
  function prepare(){
    if(!valid||locked)return;
    const id=crypto.randomUUID();
    try{localStorage.setItem(STORAGE,JSON.stringify({operation_id:id}));}
    catch(error){setMessage(errorText(error));return;}
    current.current=id;setOperation(id);setResult(null);setConfirmed(false);void request("prepare",id);
  }
  function newItem(){
    if(!terminal||busy)return;
    localStorage.removeItem(STORAGE);current.current=null;setOperation(null);setResult(null);setConfirmed(false);setMessage("");setRecoveryMessage("");
  }
  const preview=result?.preview;
  return <section className="live-equipment-add">
    <h2>实时添加装备</h2>
    <p>先由游戏按种子生成预览，确认后再加入背包。难度、进度和已习得信息由当前游戏角色决定。</p>
    <p className="equipment-notes">请先读档进入角色，操作时让游戏停在菜单。添加后到神社存档即可保存，无需关闭游戏。</p>
    <div className="live-equipment-layout">
      <div className="live-equipment-picker">
        <label><span>搜索装备</span><input value={query} onChange={event=>setQuery(event.target.value)} disabled={locked}/></label>
        <fieldset className="live-equipment-filters" disabled={locked}><FacetFilter majors={MAJOR_ORDER.map(value=>[value,null])} major={kind} onMajor={value=>{setKind(value);setType("");setSchool("");}} types={(ADD_TYPES.get(kind)??[]).map(value=>[value,null])} type={type} onType={setType} school={school} onSchool={kind==="防具"?setSchool:null}/></fieldset>
        <ul className="live-equipment-items">{rows.map(row=><li key={row.id}><button data-action="pick-equipment" aria-pressed={item===row.id} className={item===row.id?"active":""} disabled={locked} onClick={()=>setItem(row.id)}>{itemName(row.id)}<small>{[row.type||row.major,row.school].filter(Boolean).join(" · ")}</small></button></li>)}</ul>
      </div>
      <div className="live-equipment-form">
        <h3>{item===null?"请选择装备":itemName(item)}</h3>
        <div className="live-equipment-fields">{([["level","等级",level,setLevel],["plus","强化值",plus,setPlus],["rarity","稀有度",rarity,setRarity],["seed","生成种子",seed,setSeed]] as const).map(([key,label,value,setter])=><label key={key}><span>{label}</span><input data-field={key} inputMode="numeric" value={value} disabled={locked} onChange={event=>setter(event.target.value)}/></label>)}</div>
        <div className="live-equipment-fields"><label style={{gridColumn:"1 / -1"}}><span>要备份的存档路径</span><input data-field="backup-path" value={backupPath} disabled={locked} aria-describedby="equipment-backup-hint" onChange={event=>setBackupPath(event.target.value)}/></label></div>
        <p id="equipment-backup-hint" className="equipment-notes">留空时自动选择唯一存档；多个存档时填写当前角色的 SAVEDATA.BIN 路径。</p>
        <p className="equipment-notes">种子范围为 0–65535。不同种子会得到不同词条；预览没有想要的结果时，可取消后换种子。</p>
        <button data-action="prepare-equipment" className="primary" disabled={!valid||locked} onClick={prepare}>生成实时预览</button>
        <Notice text={message}/>
        <Notice text={recoveryMessage}/>
        {result&&<div data-state={result.state}>
          {preview&&<div className="live-equipment-preview"><h3>游戏生成的装备</h3><p>{itemName(preview.item_id)} · {preview.level} +{preview.plus} · <span>稀有度</span> {preview.rarity}</p><ul>{preview.effects.filter(effect=>effect.effect_id!==0xffffffff).map((effect,index)=><li key={index}><span>{effect.star?"✦ ":""}{effectName(effect.effect_id)}</span><b>{effect.value}</b></li>)}</ul></div>}
          {result.state==="prepared"&&<>
            <label className="live-equipment-confirm"><input data-action="confirm-equipment" type="checkbox" checked={confirmed} disabled={busy} onChange={event=>setConfirmed(event.target.checked)}/>我已核对预览，确认添加这一件装备</label>
            <div className="live-equipment-actions"><button data-action="execute-equipment" className="primary" disabled={busy||!confirmed||!result.plan_digest} onClick={()=>operation&&void request("execute",operation)}>确认加入背包</button><button data-action="cancel-equipment" disabled={busy} onClick={()=>operation&&void request("cancel",operation)}>取消本次添加</button></div>
          </>}
          {result.state==="uncertain"&&<p>结果尚未确认。请读取状态或恢复核对，程序不会再次执行添加。</p>}
          {result.state==="verified"&&<Notice tone="success" text="装备已加入背包并回读核对。到神社存档后即可保存。"/>}
          {result.state==="cancelled"&&<p>本次添加已取消。</p>}
          {result.state==="rejected_before_dispatch"&&<p>本次添加未执行。可重新选择装备或种子。</p>}
          {result.state==="rejected_before_insertion"&&<p>构造结果与预览不一致，装备没有加入背包。已核对背包未变，可重新准备。</p>}
        </div>}
        {operation&&<div className="live-equipment-actions"><button data-action="status-equipment" disabled={busy} onClick={()=>void request("status",operation)}>读取添加状态</button><button data-action="recover-equipment" disabled={busy} onClick={()=>void request("recover",operation)}>恢复核对</button>{terminal&&<button data-action="new-equipment" disabled={busy} onClick={newItem}>准备另一件装备</button>}</div>}
        {operation&&<details className="equipment-notes"><summary>操作记录</summary><code>{operation}</code><p>切换页面或重启工具后可继续核对此记录，无需重新添加。</p></details>}
      </div>
    </div>
  </section>;
}

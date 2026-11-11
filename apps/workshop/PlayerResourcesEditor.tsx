import React, {useEffect,useRef,useState} from "react";
import type {PlayerResources,PlayerEdit} from "../../packages/contracts/protected-responses";
import {runtimeObserver} from "./save-workspace";
import {Notice} from "./Notice";
import {localize} from "./presentation";
import catalog from "./player-clans.json";

export type ResourceSection="ingot"|"glory"|"fragments";
type Snapshot=PlayerResources["player_resources"];
type Result=PlayerEdit["player_edit"];
type Field=NonNullable<Result["field"]>;
const titles:Record<Field,string>={ingot:"灵银地金储存的精华",fragments:"常世的残思",glory:"持有武功",clan_rank:"武家阶级"};
const key="nioh3-player-resource-operation-v1";
const reference=()=>{try{const s=localStorage.getItem(key);return s&&/^[0-9a-f]{8}(-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(s)?s:null;}catch{return null;}};
const settled=(r:Result)=>r.state!=="uncertain";
const clanName=(id:number)=>catalog.clans.find(c=>c.id===id)?.name??"未隶属武家";
const rankName=(rank:number)=>catalog.ranks.find(r=>r.index===rank)?.name??"—";
type Review={snapshot:Snapshot;field:Field;clan:number;before:string;after:string;stored?:string};

/** Decimal strings stay exact from input through review, transport and receipts. */
export function PlayerResourcesEditor({section,onBusy}:{section:ResourceSection;onBusy?:(busy:boolean)=>void}){
  const [snapshot,setSnapshot]=useState<Snapshot|null>(null),[message,setMessage]=useState("");
  const [pending,setPending]=useState<string|null>(reference),[blocked,setBlocked]=useState(false);
  const [review,setReview]=useState<Review|null>(null),[busy,setBusy]=useState(false);
  const [draft,setDraft]=useState(""),[gloryDraft,setGloryDraft]=useState(""),[clan,setClan]=useState(0),[targetRank,setTargetRank]=useState("0");
  const mounted=useRef(false),running=useRef(false),callbacks=useRef(onBusy),heading=useRef<HTMLHeadingElement>(null);
  const clanSelection=useRef(0);
  callbacks.current=onBusy;
  useEffect(()=>{mounted.current=true;return()=>{mounted.current=false;};},[]);
  useEffect(()=>{setReview(null);setMessage("");},[section]);
  useEffect(()=>{setDraft(snapshot?.[section]??"");},[section,snapshot]);
  useEffect(()=>{
    if(!review)return;heading.current?.focus();
    const close=(e:KeyboardEvent)=>{if(e.key==="Escape"&&!running.current)setReview(null);};
    document.addEventListener("keydown",close);return()=>document.removeEventListener("keydown",close);
  },[review]);
  async function run(task:()=>Promise<void>){
    if(running.current)return;running.current=true;setBusy(true);callbacks.current?.(true);
    try{await task();}
    catch{if(mounted.current)setMessage(reference()?"操作结果尚未确认，请先核对上次操作，不要重复写入。":"未能读取或修改人物数据。请确认已进入支持的 PC v2.02 游戏后重新读取。");}
    finally{running.current=false;if(mounted.current)setBusy(false);callbacks.current?.(false);}
  }
  async function read(){
    const reply=await window.operations.execute({method:"runtime.player_resources",params:{}});
    if(!reply||!("player_resources" in reply))throw Error("PLAYER_RESOURCES_EXPECTED");
    if(!mounted.current)return;
    setSnapshot(reply.player_resources);setGloryDraft(reply.player_resources.glory);setReview(null);
    const id=reply.player_resources.clans.some(c=>c.clan_id===clanSelection.current)?clanSelection.current
      :reply.player_resources.current_clan_id||reply.player_resources.clans[0]?.clan_id||0;
    clanSelection.current=id;
    setClan(id);setTargetRank(String(reply.player_resources.clans.find(c=>c.clan_id===id)?.rank??0));
  }
  function adopt(reply:unknown,id:string){
    if(!reply||typeof reply!=="object"||!("player_edit" in reply))throw Error("PLAYER_RESULT_EXPECTED");
    const result=(reply as PlayerEdit).player_edit;
    if(result.operation_id!==id)throw Error("PLAYER_RECEIPT_MISMATCH");
    if(settled(result))try{if(localStorage.getItem(key)===id)localStorage.removeItem(key);}catch{/* Status remains read-only. */}
    if(!mounted.current)return result;
    setReview(null);setPending(settled(result)?null:id);
    setMessage(result.state==="verified"?"人物数据已写入并回读核对。请在游戏内确认效果，并正常存档。"
      :result.state==="uncertain"?"操作结果尚未确认，请先核对上次操作，不要重复写入。"
      :result.state==="expired"?"原游戏进程已结束。请重新读取当前角色并核对点数。"
      :result.state==="not_applied"?"当前仍为修改前的数据，未重新执行写入。":"本次未执行新的写入，请重新读取数据后核对。");
    return result;
  }
  async function check(id:string,recover=false){
    const invoke=()=>window.operations.execute({method:recover?"runtime.player_recover":"runtime.player_status",params:{operation_id:id}});
    const reply=runtimeObserver?await runtimeObserver.run(invoke):await invoke();
    const result=adopt(reply,id);
    if(mounted.current&&result&&settled(result)){setSnapshot(null);}
  }
  useEffect(()=>{void run(async()=>{
    const id=reference();
    const reply=await window.operations.execute({method:"runtime.player_status",params:id?{operation_id:id}:{}});
    if(!reply||!("player_edit" in reply))throw Error("PLAYER_RESULT_EXPECTED");
    const found=reply.player_edit.operation_id;
    if(found&&reply.player_edit.state==="uncertain"){
      // Set memory ownership before attempting the convenience browser reference.
      if(mounted.current)setPending(found);
      try{localStorage.setItem(key,found);}catch{/* Durable backend discovery survives storage loss. */}
      adopt(reply,found);return;
    }
    if(found)adopt(reply,found);
    const other=await window.operations.execute({method:"runtime.prestige_status",params:{}});
    if(other&&"prestige_edit" in other&&other.prestige_edit.state==="uncertain"){
      if(mounted.current){setBlocked(true);setMessage("请先到恩惠点数分区核对上次操作。");}
    }
    await read();
  });},[]);
  function prepare(field:Field,text:string,clanId=0){
    if(!snapshot||busy||pending||blocked)return;
    if(field==="glory"&&!snapshot.glory_editable)return;
    if(!/^(0|[1-9][0-9]{0,19})$/.test(text)){setMessage("请输入有效的非负整数。");return;}
    const n=BigInt(text),row=snapshot.clans.find(c=>c.clan_id===clanId);
    const cap=field==="clan_rank"?9n:BigInt(snapshot[(field+"_max") as "ingot_max"|"fragments_max"|"glory_max"]);
    if(n>cap){setMessage("目标数值超过游戏支持的上限。");return;}
    const before=field==="clan_rank"?String(row?.rank??0):snapshot[field];
    if(text===before){setMessage("没有需要修改的内容。");return;}
    let stored:string|undefined;
    if(field==="clan_rank"){
      if(!row){setMessage("该武家尚无可修改的贡献记录。");return;}
      const threshold=BigInt(catalog.ranks[Number(n)].minimum),tenure=BigInt(row.tenure);
      const next=threshold>tenure?threshold-tenure:0n;
      if(n<9n&&next+tenure>=BigInt(catalog.ranks[Number(n)+1].minimum)){setMessage("隶属时间贡献使该目标阶级不可用。");return;}
      stored=String(next);
    }
    setMessage("");setReview({snapshot,field,clan:clanId,before,after:text,stored});
  }
  async function write(){
    if(!review||pending||blocked||running.current)return;const selected=review;
    await run(async()=>{
      const id=crypto.randomUUID();
      localStorage.setItem(key,id);setPending(id);setReview(null);
      const invoke=()=>window.operations.execute({method:"runtime.player_edit",params:{
        operation_id:id,process_id:selected.snapshot.process_id,process_birth:selected.snapshot.process_birth,
        snapshot_id:selected.snapshot.snapshot_id,field:selected.field,clan_id:selected.clan,value:selected.after,confirmed:true,
      }});
      const reply=runtimeObserver?await runtimeObserver.run(invoke):await invoke();
      const result=adopt(reply,id);
      if(result?.state==="verified"&&mounted.current)try{await read();}catch{setSnapshot(null);}
    });
  }
  const disabled=busy||!!pending||blocked;
  const selectedClan=snapshot?.clans.find(c=>c.clan_id===clan);
  return <section className="player-resources" data-resource-section={section}>
    <div className="character-toolbar"><h2>{section==="glory"?"武功与武家":titles[section]}</h2>
      <button disabled={busy||!!pending} onClick={()=>void run(read)}>重新读取人物数据</button></div>
    <p className="character-hint">{section==="ingot"?"修改灵银地金内储存的精华，随身精华和道具数量保持不变。"
      :section==="fragments"?"修改供养魂核获得、用于魂核和守护灵觉醒等操作的常世的残思。"
      :"武功余额和各武家的阶级分别修改。武家阶级会结合该武家的贡献与隶属时间计算。"}</p>
    <Notice tone="warning" text="此功能尚未完成实机验证。写入后请在游戏内核对，保存和重载效果仍待确认。"/>
    {section==="glory"&&<Notice text="武功余额属于账号共用数据；联网同步后的保留情况仍待验证。"/>}
    <Notice text={message}/>
    {pending&&<section className="character-plan player-recovery"><h3>有待核对的人物操作</h3>
      <p>重新读取操作记录或核对当前游戏，只会检查结果，不会重复写入。</p>
      <div className="character-actions"><button disabled={busy} onClick={()=>void run(()=>check(pending))}>读取操作记录</button>
        <button disabled={busy} onClick={()=>void run(()=>check(pending,true))}>恢复核对</button></div></section>}
    {review&&<section className="character-plan player-review">
      <h3 tabIndex={-1} ref={heading}>确认人物数据修改</h3>
      <dl><div><dt>修改项目</dt><dd>{titles[review.field]}</dd></div>
        {review.field==="clan_rank"&&<div><dt>武家</dt><dd>{clanName(review.clan)}</dd></div>}
        <div><dt>修改前 → 修改后</dt><dd>{review.field==="clan_rank"
          ?localize(rankName(Number(review.before)))+" → "+localize(rankName(Number(review.after)))
          :review.before+" → "+review.after}</dd></div>
        {review.stored!==undefined&&<div><dt>修改后的武家贡献</dt><dd>{review.stored}</dd></div>}</dl>
      <div className="character-actions"><button className="primary" disabled={busy} onClick={()=>void write()}>确认写入人物数据</button>
        <button disabled={busy} onClick={()=>setReview(null)}>取消</button></div></section>}
    {snapshot&&<div className="player-resource-grid">
      <section className="player-resource-card" data-resource-field={section}>
        <h3>{titles[section]}</h3>
        <dl><div><dt>当前数量</dt><dd>{snapshot[section]}</dd></div>
          <div><dt>游戏上限</dt><dd>{snapshot[(section+"_max") as "ingot_max"|"fragments_max"|"glory_max"]}</dd></div></dl>
        <label>目标数量<input inputMode="numeric" aria-label="目标数量" disabled={disabled||(section==="ingot"&&!snapshot.ingot_owned)||(section==="glory"&&!snapshot.glory_editable)}
          value={section==="glory"?gloryDraft:draft} onChange={e=>{section==="glory"?setGloryDraft(e.target.value):setDraft(e.target.value);setReview(null);}}/></label>
        <button disabled={disabled||(section==="ingot"&&!snapshot.ingot_owned)||(section==="glory"&&!snapshot.glory_editable)} onClick={()=>prepare(section,section==="glory"?gloryDraft:draft)}>核对修改</button>
        {section==="ingot"&&!snapshot.ingot_owned&&<p className="character-hint">当前角色尚未持有灵银地金，不能修改储存精华。</p>}
        {section==="glory"&&!snapshot.glory_editable&&<p className="character-hint">账号与角色的武功同步标记不同；请在游戏内正常获取或消费武功后重新读取。</p>}
      </section>
      {section==="glory"&&<section className="player-resource-card" data-resource-field="clan_rank">
        <h3>武家阶级</h3><label>武家<select aria-label="武家" disabled={disabled} value={clan} onChange={e=>{
          const id=Number(e.target.value);clanSelection.current=id;setClan(id);setReview(null);setTargetRank(String(snapshot.clans.find(c=>c.clan_id===id)?.rank??0));
        }}><option value={0}>请选择武家</option>{catalog.clans.map(c=><option key={c.id} value={c.id} disabled={!snapshot.clans.some(r=>r.clan_id===c.id)}>{c.name}</option>)}</select></label>
        <dl><div><dt>当前阶级</dt><dd>{selectedClan?rankName(selectedClan.rank):"—"}</dd></div>
          <div><dt>武家贡献</dt><dd>{selectedClan?.stored??"—"}</dd></div>
          <div><dt>隶属时间贡献</dt><dd>{selectedClan?.tenure??"—"}</dd></div></dl>
        <label>目标阶级<select aria-label="目标阶级" disabled={disabled||!selectedClan} value={targetRank} onChange={e=>{setTargetRank(e.target.value);setReview(null);}}>
          {catalog.ranks.map(r=><option key={r.index} value={r.index}>{r.name}</option>)}</select></label>
        <button disabled={disabled||!selectedClan} onClick={()=>prepare("clan_rank",targetRank,clan)}>核对修改</button>
        <p className="character-hint">仅修改所选武家的贡献，不改变隶属武家、庇佑选择或势力战得分。</p>
      </section>}
    </div>}
  </section>;
}

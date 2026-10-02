import React,{useEffect,useRef,useState} from "react";
import {Notice} from "./Notice";
import {errorText} from "./public-errors";
import {desktop} from "./desktop-bridge";
import {useUiLocale} from "./presentation";
import "../desktop/src/operations-api";

type Report={
 present?:boolean;warning?:boolean;accepted?:boolean;reference_match?:boolean;game_version?:string;executable?:string;
 backup?:{attempted:boolean;verified:boolean;paths:string[];files?:{source:string;copy:string;bytes:number;sha256:string}[];error?:string|null}|null;
 operation_scoped_features?:string[];
 differences?:{code:string;expected:string;actual:string}[];
 hard_blocks?:{code:string;detail:string}[];
 plan?:{plan_id:string;bypassed_checks:string[];allowed_features:string[];required_checks:string[];audit_path:string|null}|null;
 probe?:{status:"structure_only";outcome:"unique"|"multiple"|"none"|"unavailable"|"not_needed";candidates:{profile_version:string;matched:boolean;error:string|null}[];note:string};
};
type Action="inspect"|"prepare"|"accept"|"cancel";
const technical=(text:string)=>React.createElement("code",null,text);
const labels:Record<string,string>={
 executable_sha256:"游戏程序指纹",game_version:"游戏版本",character_layout_evidence:"角色结构验证范围",
 unsupported_version:"此版本没有受支持的运行配置",backup_required:"需要重新创建并验证备份",backup_unverified:"备份未通过验证",consent_audit_failed:"确认记录未能安全保存",
 live_character:"实时角色读取与编辑",live_scroll_add:"实时添加绘卷",live_equipment_add:"实时添加装备",live_count_edit:"实时修改绘卷次数",native_generation:"原生绘卷生成、搜索与恩宠查询",temporary_override:"临时辅助参数覆盖",challenge_capacity_override:"临时挑战容量覆盖",
 process_identity:"当前游戏进程身份",code_and_layout:"原生代码位置与数据结构",ownership_and_bounds:"目标归属、范围与唯一性",verified_backup:"本次准备时的已验证备份快照",single_writer:"单一写入者与并发保护",recovery_receipts:"操作凭据与未知结果恢复",readback:"写入后的精确回读",
 verified_reference:"已验证基准",experimental_version_selected:"按版本选择的实验性结构",
};
const probeNames={unique:"发现一个结构候选",multiple:"发现多个结构候选，无法唯一判断",none:"没有匹配的结构候选",unavailable:"结构检查不可用",not_needed:"无需结构检查"};
const describe=(code:string)=>labels[code]||code;
const impacts:Record<string,string>={executable_sha256:"需要本次风险确认",game_version:"按此版本限制可用功能",character_layout_evidence:"角色功能仍属实验性"};
const remedies:Record<string,string>={unsupported_version:"请在设置中检查已选择的游戏程序。使用受支持版本后，重新连接并准备。",backup_required:"请确认存档位置可读取、备份目录可写入，然后重试备份。",backup_unverified:"请确认存档位置可读取、备份目录可写入，然后重试备份。",consent_audit_failed:"请确认工作室数据目录可写入，然后重新准备。"};

/** Native consent is bound to the process, fresh verified backup and exact plan. */
export function RuntimeCompatibility(){
 useUiLocale();
 const [report,setReport]=useState<Report|null>(null),[open,setOpen]=useState(false),[busy,setBusy]=useState(false),[message,setMessage]=useState("");
 const [risk,setRisk]=useState(false),[backed,setBacked]=useState(false);
 const epoch=useRef(0),queue=useRef(Promise.resolve()),mounted=useRef(true),dialog=useRef<HTMLDialogElement>(null),visible=useRef(false),pending=useRef(false);
 const canAccept=!!report?.present&&!!report.backup?.verified&&!!report.plan?.plan_id&&!report.hard_blocks?.length;
 const referenceMatched=!busy&&report?.reference_match===true&&!report.warning&&!report.hard_blocks?.length;

 function action(kind:Action){
  if(kind==="accept"&&(pending.current||!risk||!backed||!canAccept))return;
  const planId=kind==="accept"?report?.plan?.plan_id:undefined;
  const request=++epoch.current;
  pending.current=true;setBusy(true);setMessage("");setRisk(false);setBacked(false);
  if(kind!=="accept")setReport(previous=>previous?{...previous,accepted:false,reference_match:false,plan:null,backup:null}:previous);
  // Closing invalidates the renderer immediately. The host cancel follows any
  // already-running prepare, so that its late result cannot restore consent.
  queue.current=queue.current.then(async()=>{
   if(kind!=="cancel"&&request!==epoch.current)return;
   try{
    const result=await window.operations.execute({method:"runtime.compatibility",params:{action:kind,...(kind==="accept"?{plan_id:planId,confirmed:true,backup_confirmed:true}:{})}});
    if(!mounted.current||request!==epoch.current)return;
    if(!("compatibility" in result))throw Error("UNEXPECTED_COMPATIBILITY_RESULT");
    const next=result.compatibility as Report;
    setReport(next);
    if(kind==="accept"){
     if(!next.accepted)throw Error("COMPATIBILITY_CONFIRMATION_REQUIRED");
     visible.current=false;setOpen(false);window.dispatchEvent(new Event("nioh3:compatibility-accepted"));
    }
   }catch(error){
    if(mounted.current&&request===epoch.current){setReport(previous=>previous?{...previous,accepted:false,plan:null,backup:null}:previous);setMessage(errorText(error));}
   }finally{
    if(mounted.current&&request===epoch.current){pending.current=false;setBusy(false);}
   }
  });
 }
 function show(){
  if(visible.current&&pending.current)return;
  visible.current=true;setOpen(true);action("prepare");
 }
 function dismiss(){visible.current=false;setOpen(false);action("cancel");}
 useEffect(()=>{
  if(!desktop)return;
  mounted.current=true;action("inspect");
  const detected=(event:Event)=>{if(!visible.current&&!pending.current)setReport((event as CustomEvent<Report>).detail);};
  window.addEventListener("nioh3:compatibility-required",show);
  window.addEventListener("nioh3:compatibility-detected",detected);
  return()=>{mounted.current=false;++epoch.current;window.removeEventListener("nioh3:compatibility-required",show);window.removeEventListener("nioh3:compatibility-detected",detected);};
 },[]);
 useEffect(()=>{if(open&&!dialog.current?.open)dialog.current?.showModal();},[open]);
 if(!desktop)return null;
 return <>
  {report?.warning&&!report.accepted&&<aside className="runtime-compatibility-banner">
   <span>检测到版本或程序差异。请查看支持范围、备份结果和本次兼容计划。</span>
   <button onClick={show}>查看兼容提示</button>
  </aside>}
  {!open&&message&&<Notice text={message}/>}
  {open&&<dialog ref={dialog} className="compatibility-dialog" aria-labelledby="compatibility-title" onCancel={event=>{event.preventDefault();dismiss();}} onClick={event=>{
   if(event.target!==event.currentTarget)return;
   const rect=event.currentTarget.getBoundingClientRect();
   if(event.clientX<rect.left||event.clientX>rect.right||event.clientY<rect.top||event.clientY>rect.bottom)dismiss();
  }}>
   <header><h2 id="compatibility-title">游戏版本兼容提示</h2><button data-action="compatibility-close" onClick={dismiss}>关闭</button></header>
   <div className="compatibility-body" aria-busy={busy}>
    {referenceMatched?<Notice tone="success" text="当前程序与已验证版本一致，无需兼容确认；返回原操作即可继续。"/>:<p>兼容模式只允许跳过下方列出的差异检查。未适配功能和必要安全检查仍会阻止操作。</p>}
    {report?.game_version&&<p><span>实际文件版本</span>：{technical(report.game_version)}</p>}
    {report?.executable&&technical(report.executable)}
    {report?.present===false&&<Notice tone="warning" text="尚未连接到游戏。请启动游戏或等待游戏重启完成，然后点击重新连接并准备。"/>}
    {!!report?.hard_blocks?.length&&<section className="compatibility-blocks"><h3>本兼容计划暂不可确认</h3>
     <p>这些问题会阻止确认本兼容计划。请解决后重新准备；实时添加按各自操作的检查结果处理。</p>
     <ul>{report.hard_blocks.map((block,index)=><li key={block.code+index}><strong>{describe(block.code)}</strong><p>{remedies[block.code]||"请核对检测详情，解决问题后重新准备。"}</p><details><summary>检测详情</summary>{technical(block.detail)}</details></li>)}</ul>
    </section>}
    {!busy&&!referenceMatched&&report&&!report.backup?.verified&&<section className="compatibility-blocks"><h3>备份尚未通过验证</h3><p>本兼容计划的备份尚未通过验证。请解决备份问题后重试；勾选确认不能跳过此限制。</p>{report.backup?.error&&technical(report.backup.error)}</section>}
    {!!report?.operation_scoped_features?.length&&<section data-section="operation-scoped-features"><h3>单独核验的实时添加</h3>
     <p>以下添加在各自操作中核验目标和备份，无需先确认本兼容计划。请返回添加页面准备；不受支持的目标仍不会执行。</p>
     <ul>{report.operation_scoped_features.map(feature=><li key={feature}>{describe(feature)}</li>)}</ul>
    </section>}
    {!!report?.differences?.length&&<section><h3>检测到的差异</h3>
     <table className="compatibility-feature-table"><thead><tr><th>检查项目</th><th>已验证基准</th><th>当前检测</th><th>影响</th></tr></thead><tbody>
      {report.differences.map((difference,index)=><tr key={difference.code+index}><td>{describe(difference.code)}</td><td>{labels[difference.expected]?describe(difference.expected):technical(difference.expected)}</td><td>{labels[difference.actual]?describe(difference.actual):technical(difference.actual)}</td><td>{impacts[difference.code]||"请核对检测详情"}</td></tr>)}
     </tbody></table>
    </section>}
    {report?.probe&&<section><h3>只读结构检查</h3><p>{probeNames[report.probe.outcome]}</p>
     <p>结构匹配仅供诊断，不代表版本已适配，也不会启用原生调用或写入。</p>
     <ul>{report.probe.candidates.map(candidate=><li key={candidate.profile_version}>{candidate.profile_version} — {candidate.matched?"结构匹配":"结构未匹配"}</li>)}</ul>
     <details><summary>检测详情</summary>{technical(report.probe.note)}</details>
    </section>}
    {!referenceMatched&&(busy||report?.backup?.verified)&&<section><h3>已验证备份</h3>
     {busy?<p role="status">正在核对兼容计划和备份，请稍候。</p>:report?.backup?.verified?<>
      <p>已创建并回读核对本次准备时的存档备份。确认计划时会再次核对源存档；后续操作不会自动更新这份快照。</p>
      {report.backup.files?.length?report.backup.files.map(file=><dl className="game-install-profile" key={file.source}><dt>源存档</dt><dd>{technical(file.source)}</dd><dt>备份副本</dt><dd>{technical(file.copy)}</dd></dl>):report.backup.paths.map(path=><React.Fragment key={path}>{technical(path)}</React.Fragment>)}
     </>:<><p>自动备份尚未通过验证，不能继续。请解决备份问题后重试；勾选确认不能跳过此限制。</p>{report?.backup?.error&&technical(report.backup.error)}</>}
    </section>}
    {report?.plan&&<section className="compatibility-plan"><h3>本次兼容计划</h3>
     <h4>本次允许的功能</h4><ul>{report.plan.allowed_features.map(feature=><li key={feature}>{describe(feature)}</li>)}</ul>
     <h4>确认后跳过的差异检查</h4><ul>{report.plan.bypassed_checks.map(check=><li key={check}>{describe(check)}</li>)}</ul>
     <h4>仍然必须通过的检查</h4><ul>{report.plan.required_checks.map(check=><li key={check}>{describe(check)}</li>)}</ul>
     <p>确认只对当前游戏进程和本次计划有效。它不会自动执行写入，也不保证未验证版本的行为正确。</p>
     <details><summary>计划标识</summary>{technical(report.plan.plan_id)}</details>
    </section>}
    {!referenceMatched&&<><label><input data-action="compatibility-risk" type="checkbox" checked={risk} disabled={busy||!canAccept} onChange={event=>setRisk(event.target.checked)}/>我已核对本次计划，了解跳过检查的范围和风险</label>
    <label><input data-action="compatibility-backup" type="checkbox" checked={backed} disabled={busy||!canAccept} onChange={event=>setBacked(event.target.checked)}/>我已确认上方备份对应本次准备时的存档快照</label></>}
    <Notice text={message}/>
   </div>
   <footer><button data-action="compatibility-prepare" onClick={()=>action("prepare")} disabled={busy}>{report?.hard_blocks?.some(block=>block.code.startsWith("backup_"))?"修复后重试备份":"重新连接并准备"}</button>{referenceMatched?<button data-action="compatibility-return" onClick={dismiss}>返回原操作</button>:<button data-action="compatibility-accept" onClick={()=>action("accept")} disabled={busy||!risk||!backed||!canAccept}>确认本次兼容计划</button>}</footer>
  </dialog>}
 </>;
}

import React,{useEffect,useState} from "react";
import {errorText} from "./public-errors";
import {Notice} from "./Notice";
import {useUiLocale} from "./presentation";
import "../desktop/src/review-api";
type Install=NonNullable<Awaited<ReturnType<typeof window.review.gameInstallation>>>;
const pathText=(text:string)=>React.createElement("code",null,text);
const featureNames={offline_scroll_generation:"离线绘卷生成",character_read_edit:"角色读取与编辑",native_scroll_add:"实时添加绘卷",native_equipment_add:"实时添加装备"};
const supportNames={supported:"已支持",experimental:"实验性支持",unsupported:"未支持",unavailable:"不可用"};
/** Native-picked path, not a renderer-supplied executable/version override. */
export function GameInstallation({compact=false}:{compact?:boolean}){
 useUiLocale();
 const [install,setInstall]=useState<Install|null>(null),[busy,setBusy]=useState(false),[message,setMessage]=useState("");
 async function action(kind:"inspect"|"select"|"reset"){
  setBusy(true);setMessage("");
  try{const result=await window.review.gameInstallation(kind);if(result){setInstall(result);if(result.restart_required)setMessage("游戏路径已记录。请重新打开工作室后使用，游戏无需关闭。");}}
  catch(error){setMessage(errorText(error))}finally{setBusy(false)}
 }
 useEffect(()=>{void action("inspect")},[]);
 return <div className={"game-install"+(compact?" game-install-compact":"")}>
  {!compact&&<><h3>游戏程序位置</h3><p>自动查找 Steam 安装；其他安装方式可手动选择实际的 Nioh3.exe。</p></>}
  {install&&<>
   <p><span>检测来源</span>：{install.source==="selected"?"手动选择":"自动发现"}</p>
   {install.executable&&<p>{pathText(install.executable)}</p>}
   <p><span>实际文件版本</span>：<strong data-field="installed-file-version">{install.file_version||"无法读取"}</strong></p>
   {install.identity_error&&<Notice tone="warning" text={install.identity_error.message} installationRecovery={false}/>}
   <section className="game-install-compatibility" data-status={install.compatibility.status} aria-label="版本兼容范围">
    <h4>{install.compatibility.status==="known"?"已识别版本，请按功能查看支持范围":install.compatibility.status==="unknown"?"尚未适配此版本":"无法判断版本兼容性"}</h4>
    {install.compatibility.status==="unknown"&&<p>检测到了实际版本，但当前没有匹配的数据和运行时配置。不会借用其他版本继续操作。</p>}
    {install.compatibility.status==="unknown"&&<><p><span>已登记的文件版本</span>：{pathText("2.0.0.2 / 2.0.1.0 / 2.0.2.0")}</p><p>请核对是否选中了当前运行的 Nioh3.exe。新版本需要专属数据与运行配置，不能通过风险确认替代。</p></>}
    {install.compatibility.display_version&&<p><span>版本名称</span>：{install.compatibility.display_version}</p>}
    <dl className="game-install-profile">
     <dt>数据版本</dt><dd>{install.compatibility.data_version||"无匹配配置"}</dd>
     <dt>资源目录</dt><dd>{install.compatibility.resource_directory?pathText(install.compatibility.resource_directory):"无匹配配置"}</dd>
     <dt>运行时配置</dt><dd>{install.compatibility.runtime_profile?pathText(install.compatibility.runtime_profile):"无匹配配置"}</dd>
    </dl>
    <table className="compatibility-feature-table"><thead><tr><th>功能</th><th>支持状态</th></tr></thead><tbody>
     {(Object.keys(featureNames) as (keyof typeof featureNames)[]).map(key=><tr key={key} data-feature={key}><td>{featureNames[key]}</td><td data-support={install.compatibility.features[key]}>{supportNames[install.compatibility.features[key]]}</td></tr>)}
    </tbody></table>
    <p className="settings-note">实验性支持仍需风险确认；支持状态不代表当前游戏已通过现场验证。</p>
    <details><summary>检测依据</summary>{pathText(install.compatibility.reason)}</details>
   </section>
  </>}
  <div className="game-install-actions"><button data-action="inspect-game-executable" disabled={busy} onClick={()=>void action("inspect")}>重新检测安装与版本</button><button data-action="select-game-executable" disabled={busy} onClick={()=>void action("select")}>选择游戏程序</button>{!compact&&<button data-action="reset-game-executable" disabled={busy} onClick={()=>void action("reset")}>恢复自动查找</button>}</div>
  {!compact&&<p className="settings-note">版本从选中的文件读取。实时功能仍会检查正在运行的游戏是否匹配。</p>}
  <Notice text={message} installationRecovery={false}/>
 </div>;
}

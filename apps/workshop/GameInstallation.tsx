import React,{useEffect,useState} from "react";
import {errorText} from "./public-errors";
import {Notice} from "./Notice";
import {useUiLocale} from "./presentation";
import "../desktop/src/review-api";
type Install=NonNullable<Awaited<ReturnType<typeof window.review.gameInstallation>>>;
const pathText=(text:string)=>React.createElement("code",null,text);
const featureNames={offline_scroll_generation:"离线绘卷生成",character_read_edit:"角色读取与编辑",native_scroll_add:"实时添加绘卷",native_equipment_add:"实时添加装备"};
const supportNames={supported:"已支持",experimental:"实验性支持",unsupported:"未支持",unavailable:"不可用"};
/** The detected version for the settings menu row, e.g. "PC v2.02". */
export function GameVersionHint(){
 useUiLocale();
 const [text,setText]=useState("");
 useEffect(()=>{let live=true;window.review.gameInstallation("inspect").then(result=>{if(!live||!result)return;const status=result.compatibility.status;setText(status==="known"?result.compatibility.display_version||result.file_version||"":status==="unknown"?"尚未适配此版本":"未找到游戏");}).catch(()=>{if(live)setText("未找到游戏")});return()=>{live=false}},[]);
 return <small className="menu-hint">{text}</small>;
}
/**
 * Where the game is and which version it is: one summary line, the path, the
 * actions, and the per-feature detail folded away. Native-picked path, not a
 * renderer-supplied executable/version override.
 */
export function GameInstallation({compact=false}:{compact?:boolean}){
 useUiLocale();
 const [install,setInstall]=useState<Install|null>(null),[busy,setBusy]=useState(false),[message,setMessage]=useState("");
 async function action(kind:"inspect"|"select"|"reset"){
  setBusy(true);setMessage("");
  try{const result=await window.review.gameInstallation(kind);if(result){setInstall(result);if(result.restart_required)setMessage("游戏路径已记录。请重新打开工作室后使用，游戏无需关闭。");}}
  catch(error){setMessage(errorText(error))}finally{setBusy(false)}
 }
 useEffect(()=>{void action("inspect")},[]);
 const status=install?.compatibility.status;
 const features=install?(Object.keys(featureNames) as (keyof typeof featureNames)[]):[];
 const limited=features.filter(key=>install!.compatibility.features[key]!=="supported");
 return <div className={"game-install"+(compact?" game-install-compact":"")}>
  {install&&<>
   <div className="game-install-summary">
    <strong>{status==="known"?install.compatibility.display_version||install.file_version:status==="unknown"?"尚未适配此版本":"未找到游戏"}</strong>
    <span className="game-install-badge" data-badge={status}>{status==="known"?(limited.length?"部分功能受限":"已识别"):status==="unknown"?"未适配":"无法判断"}</span>
    <span className="game-install-source">{install.source==="selected"?"手动选择":"自动发现"}</span>
   </div>
   {install.executable&&<p className="game-install-path" title={install.executable}>{pathText(install.executable)}</p>}
   {install.identity_error&&<Notice tone="warning" text={install.identity_error.message} installationRecovery={false}/>}
   {status==="unknown"&&<p className="settings-note">检测到了实际版本，但当前没有匹配的数据和运行时配置，不会借用其他版本继续操作。请核对是否选中了当前运行的 Nioh3.exe，或等待工具更新。</p>}
   {status==="known"&&limited.length>0&&<p className="settings-note">{limited.map(key=>featureNames[key]+"："+supportNames[install.compatibility.features[key]]).join("；")}</p>}
  </>}
  <div className="game-install-actions"><button data-action="inspect-game-executable" disabled={busy} onClick={()=>void action("inspect")}>重新检测</button><button data-action="select-game-executable" disabled={busy} onClick={()=>void action("select")}>选择游戏程序</button>{!compact&&install?.source==="selected"&&<button data-action="reset-game-executable" disabled={busy} onClick={()=>void action("reset")}>恢复自动查找</button>}</div>
  {!compact&&!install&&<p className="settings-note">自动查找 Steam 安装；其他安装方式可手动选择实际的 Nioh3.exe。</p>}
  <Notice text={message} installationRecovery={false}/>
  {install&&<details className="game-install-details" open={status!=="known"}>
   <summary>版本详情</summary>
   <section className="game-install-compatibility" data-status={status} aria-label="版本兼容范围">
    <dl className="game-install-profile">
     <dt>实际文件版本</dt><dd><strong data-field="installed-file-version">{install.file_version||"无法读取"}</strong></dd>
     {status==="unknown"&&<><dt>已登记版本</dt><dd>{pathText("2.0.0.2 / 2.0.1.0 / 2.0.2.0")}</dd></>}
     <dt>数据版本</dt><dd>{install.compatibility.data_version||"无匹配配置"}</dd>
     <dt>资源目录</dt><dd>{install.compatibility.resource_directory?pathText(install.compatibility.resource_directory):"无匹配配置"}</dd>
     <dt>运行时配置</dt><dd>{install.compatibility.runtime_profile?pathText(install.compatibility.runtime_profile):"无匹配配置"}</dd>
    </dl>
    <table className="compatibility-feature-table"><thead><tr><th>功能</th><th>支持状态</th></tr></thead><tbody>
     {features.map(key=><tr key={key} data-feature={key}><td>{featureNames[key]}</td><td data-support={install.compatibility.features[key]}>{supportNames[install.compatibility.features[key]]}</td></tr>)}
    </tbody></table>
    <p className="settings-note">实验性支持仍需风险确认；支持状态不代表当前游戏已通过现场验证。实时功能仍会检查正在运行的游戏是否匹配。</p>
    <p className="settings-note">{pathText(install.compatibility.reason)}</p>
   </section>
  </details>}
 </div>;
}

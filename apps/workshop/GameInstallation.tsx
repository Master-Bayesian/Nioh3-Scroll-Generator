import React,{useEffect,useState} from "react";
import {errorText} from "./public-errors";
import {Notice} from "./Notice";
import "../desktop/src/review-api";
type Install=NonNullable<Awaited<ReturnType<typeof window.review.gameInstallation>>>;
const pathText=(text:string)=>React.createElement("code",null,text);
/** Native-picked path, not a renderer-supplied executable/version override. */
export function GameInstallation({compact=false}:{compact?:boolean}){
 const [install,setInstall]=useState<Install|null>(null),[busy,setBusy]=useState(false),[message,setMessage]=useState("");
 async function action(kind:"inspect"|"select"|"reset"){
  setBusy(true);setMessage("");
  try{const result=await window.review.gameInstallation(kind);if(result){setInstall(result);if(result.restart_required)setMessage("游戏路径已记录。请重新打开工作室后使用，游戏无需关闭。");}}
  catch(error){setMessage(errorText(error))}finally{setBusy(false)}
 }
 useEffect(()=>{void action("inspect")},[]);
 return <div className={"game-install"+(compact?" game-install-compact":"")}>
  {!compact&&<><h3>游戏程序位置</h3><p>自动查找 Steam 安装；其他安装方式可手动选择实际的 Nioh3.exe。</p></>}
  {install?.executable&&<p>{pathText(install.executable)}{install.file_version&&<> · <span>文件版本</span> {install.file_version}</>}</p>}
  <div className="game-install-actions"><button data-action="select-game-executable" disabled={busy} onClick={()=>void action("select")}>选择游戏程序</button>{!compact&&<button data-action="reset-game-executable" disabled={busy} onClick={()=>void action("reset")}>恢复自动查找</button>}</div>
  {!compact&&<p className="settings-note">版本从选中的文件读取。实时功能仍会检查正在运行的游戏是否匹配。</p>}
  <Notice text={message}/>
 </div>;
}

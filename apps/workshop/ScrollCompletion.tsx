import React, {useEffect, useRef, useState} from "react";
import type {ScrollCompletionPrediction} from "../../packages/contracts/protected-responses";
import {data} from "./model";
import {Notice} from "./Notice";
import {errorText} from "./public-errors";
import "../desktop/src/api";
import "../desktop/src/operations-api";

type Prediction = ScrollCompletionPrediction["completion_prediction"];
type Effect = Prediction["candidates"][number];
const effectName=(id:number)=>data.editorEffects.find(e=>e.id===String(id))?.name || `0x${id.toString(16).toUpperCase()}`;
function EffectResult({effect}:{effect:Effect}) {
  return <><b>{effectName(effect.effect_id)}</b><small>原始数值</small><span>{effect.value}</span><small>分位</small><small>{effect.roll}</small></>;
}

/** Read-only ordinary completion simulation; never returns an install action. */
export function ScrollCompletion({recordHex,identity,disabled=false}:{recordHex?:string;identity:string;disabled?:boolean}) {
  const scope=JSON.stringify([identity,recordHex,disabled]);
  const latest=useRef(scope);latest.current=scope;
  const request=useRef(0);
  const [result,setResult]=useState<{scope:string;prediction:Prediction;round:number}|null>(null);
  const [choice,setChoice]=useState<number|null>(null);
  const [busy,setBusy]=useState(false);
  const [message,setMessage]=useState("");
  useEffect(()=>{
    request.current++;setResult(null);setChoice(null);setBusy(false);setMessage("");
  },[scope]);
  useEffect(()=>()=>{request.current++;},[]);
  const shown=result?.scope===scope && !disabled ? result : null;
  const branch=shown?.prediction.branches.find(b=>b.choice===choice);
  async function predict(record=recordHex,round=1) {
    if(!record || disabled)return;
    const token=++request.current,started=scope;
    setBusy(true);setMessage("");
    try {
      const handshake=await window.nioh.handshake();
      const response=await window.operations.execute({method:"runtime.scroll_completion_predict",params:{record_hex:record,context_digest:handshake.context.context_digest}});
      if(token!==request.current || latest.current!==started)return;
      if(!response || !("completion_prediction" in response))throw Error("COMPLETION_RESULT_EXPECTED");
      setResult({scope:started,prediction:response.completion_prediction,round});setChoice(null);
    } catch(error) {
      if(token===request.current && latest.current===started) {
        setResult(null);
        const raw=String(error);
        setMessage(/matching PC|UnsupportedContext/.test(raw) ? "目前仅支持当前游戏版本的三周目稀有度 4 绘卷。" :
          /NoAttempts/.test(raw) ? "这张绘卷已没有挑战次数。" :
          /UnknownRecordSemantics|UnknownEffect|CounterOverflow|EmptyPool/.test(raw) ? "这张绘卷的数据暂不能可靠预测。" :errorText(error));
      }
    } finally {if(token===request.current && latest.current===started)setBusy(false);}
  }
  return <details className="module blue editor-section scroll-completion" name="editor-section">
    <summary>洗词条与添画预测</summary>
    <div className="editor-section-body">
      <p>预测下次普通通关可替换的词条，以及作出选择后的添画结果。</p>
      <p className="completion-boundary">首次通关可能是揭秘；揭秘条件与自动替换位置尚未确认，本面板只预测普通通关。</p>
      {disabled ? <p>请先应用或撤销绘卷修改，再读取预测。</p> : !recordHex ? <p>请先读取存档并选择一张绘卷。</p> : null}
      <button data-action="predict" className="primary" disabled={disabled || !recordHex || busy} onClick={()=>void predict()}>
        {busy ? "正在预测…" : "预测下次普通通关"}
      </button>
      {message && <Notice text={message}/>}
      {shown && <div className="completion-results">
        <p className="completion-round" data-round={shown.round}><b>模拟轮次</b> {shown.round}<span>剩余挑战次数</span> {shown.prediction.attempts}</p>
        <p>选择本轮操作，查看对应的添画，再继续预测下一轮。这里的选择只用于模拟。</p>
        <div className="completion-choices">
          <button className={choice===null ? "chosen" : ""} disabled={busy} onClick={()=>setChoice(null)}>不替换词条</button>
          {shown.prediction.candidates.map(effect=><button key={effect.slot} className={choice===effect.slot ? "chosen" : ""} disabled={busy} onClick={()=>setChoice(effect.slot)}>
            <span>替换位置</span><span>{effect.slot+1}</span><EffectResult effect={effect}/>
          </button>)}
        </div>
        <div className="completion-painting">
          <b>本轮添画</b>
          {!shown.prediction.painting.eligible ? <p>当前绘卷不满足添画条件。</p> : !shown.prediction.painting.success ? <p>本轮不会添画。</p> : branch?.painting_effect ? <p><EffectResult effect={branch.painting_effect}/></p> : null}
          <details><summary>预测依据</summary><p>抽签值 <span>{shown.prediction.painting.draw}</span> · 触发门槛 <span>{shown.prediction.painting.threshold}</span></p></details>
        </div>
        <div className="completion-actions">
          <button data-action="next" disabled={busy || !branch || shown.prediction.attempts<=1 || shown.round>=5} onClick={()=>branch && void predict(branch.record_hex,shown.round+1)}>按此选择预测下一轮</button>
          <button disabled={busy} onClick={()=>void predict()}>从当前存档重新预测</button>
        </div>
        {shown.round>=5 && <p>已模拟五轮；可从当前存档重新开始。</p>}
      </div>}
    </div>
  </details>;
}

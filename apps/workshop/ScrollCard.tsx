import React from "react";
import { data, type Sample } from "./model";
import { copyText } from "./desktop-bridge";
const hex = (id: string) =>
  "0x" + Number(id).toString(16).toUpperCase().padStart(4, "0");
export function ScrollCard({
  sample,
  level,
  showIds = false,
}: {
  sample: Sample;
  level: number;
  showIds?: boolean;
}) {
  return (
    <article
      className={"scroll rarity-" + sample.rarity}
      aria-label={"绘卷 " + sample.seed}
    >
      <header>
        <div>
          <small>
            Lv.{sample.level || 180} <span>稀有度 {sample.rarity}</span>
          </small>
          <h2>
            {(sample.playthrough || 3) === 3
              ? "百境百怪绘卷 · 顿悟"
              : `${sample.playthrough} 周目战绘卷`}
          </h2>
        </div>
        <span className="rarity-mark">R{sample.rarity}</span>
      </header>
      <div className="scroll-hero">
        <div>
          <p>
            可挑战次数上限<strong>{sample.capacity}</strong>
          </p>
          <p>
            推荐等级<strong>{level}</strong>
          </p>
        </div>
      </div>
      <section>
        <h3>特殊效果</h3>
        {Array.from({ length: 6 }, (_, i) => sample.effects[i]).map((e, i) =>
          e ? (
            <div
              className={
                "effect-line " + (e.role === "恩宠" ? "grace-line" : "")
              }
              key={i}
            >
              <span className="slot-mark">{i === 0 ? "◆" : "◇"}</span>
              <div>
                <span>{e.name}</span>
                {showIds && (
                  <small>
                    {hex(e.id)} · {e.role}
                  </small>
                )}
              </div>
              <strong title="数值评分：100 为最高档。同名词条分数越高，数值档位越高；不是实际加成百分比。">
                {e.role === "恩宠"
                  ? "恩宠"
                  : e.role === "成长词条"
                    ? "成长"
                    : sample.playthrough && sample.playthrough !== 3
                      ? "原始数值 " + e.raw
                      : "评分 " + e.roll + "/100"}
              </strong>
            </div>
          ) : (
            <div className="effect-line empty-line" key={i}>
              　
            </div>
          ),
        )}
      </section>
      <section>
        <h3>出现敌人</h3>
        <div className="enemy-lines">
          {sample.enemies.map((n, i) => (
            <span key={i}>◇ {n}</span>
          ))}
        </div>
      </section>
      <section>
        <h3>特殊规则</h3>
        {Array.from({ length: 3 }, (_, i) => sample.rules[i]).map((r, i) =>
          r ? (
            <p className="scroll-rule" key={i}>
              <span>{r.name}</span>
              <strong>{r.value}</strong>
            </p>
          ) : (
            <p className="scroll-rule empty-line" key={i}>
              　
            </p>
          ),
        )}
      </section>
      <div className="scroll-terrain">
        <span>地形影响</span>
        <strong>
          {data.terrains.find(
            (t) =>
              !t.aggregate &&
              t.effect_keys.length === sample.terrainKeys.length &&
              t.effect_keys.every((k) => sample.terrainKeys.includes(k)),
          )?.name || "未解析"}
        </strong>
      </div>
      <footer>
        <span>绘卷 ID</span>
        <strong>{sample.seed}</strong>
        <button
          aria-label="复制绘卷ID"
          onClick={() => void copyText(sample.seed)}
        >
          复制
        </button>
      </footer>
    </article>
  );
}

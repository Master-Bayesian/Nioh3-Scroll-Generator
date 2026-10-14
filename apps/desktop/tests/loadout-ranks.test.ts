import assert from "node:assert/strict";
import { test } from "node:test";
import { agilityRank, toughnessRank } from "../../workshop/loadout-ranks";

test("native agility compares the truncated ratio and supports five labels only", () => {
  for (const [weight, rank] of [[0,"A"],[200,"A"],[300,"A"],[301,"B"],[700,"B"],[701,"C"],[1000,"C"],[1001,"D"]] as const)
    assert.equal(agilityRank(weight,1000),rank);
  assert.equal(agilityRank(200,1000,true),"AA");
  assert.equal(agilityRank(201,1000,true),"A");
  assert.equal(agilityRank(2009,10000,true),"AA");
  assert.equal(agilityRank(2010,10000,true),"A");
  assert.equal(agilityRank(3009,10000),"A");
  assert.equal(agilityRank(3010,10000),"B");
  assert.equal(agilityRank(7009,10000),"B");
  assert.equal(agilityRank(7010,10000),"C");
});

test("Red Demon promotion caps at A or unlocked AA and never repairs overload", () => {
  assert.equal(agilityRank(700,1000,false,true),"A");
  assert.equal(agilityRank(701,1000,false,true),"B");
  assert.equal(agilityRank(300,1000,true,true),"AA");
  assert.equal(agilityRank(301,1000,true,true),"A");
  assert.equal(agilityRank(701,1000,true,true),"B");
  assert.equal(agilityRank(1001,1000,true,true),"D");
});

test("native toughness is 130/260/320 with a separate ultimate-stamina gate", () => {
  for (const [value,ordinary,ultimate] of [[0,"C","C"],[129,"C","C"],[130,"B","B"],[259,"B","B"],[260,"A","A"],[319,"A","A"],[320,"A","AA"],[999,"A","AA"]] as const) {
    assert.equal(toughnessRank(value),ordinary);
    assert.equal(toughnessRank(value,true),ultimate);
  }
});

test("unknown or unsupported numeric inputs do not fabricate a rank", () => {
  assert.equal(agilityRank(1,0),null);
  assert.equal(agilityRank(-1,10),null);
  assert.equal(agilityRank(1.1,10),null);
  assert.equal(agilityRank(5000000,10000000),null);
  assert.equal(toughnessRank(NaN),null);
  assert.equal(toughnessRank(-1),null);
  assert.equal(toughnessRank(320.5),null);
});

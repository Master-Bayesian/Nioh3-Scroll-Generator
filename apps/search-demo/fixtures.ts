export type Condition = { id: string; label: string; group: string; value?: string };
export const conditions: Condition[] = [
  { id: 'hp', label: '体力', value: '+313', group: 'Primary' },
  { id: 'toughness', label: '坚忍度', value: '+15', group: 'Primary' },
  { id: 'power', label: '武技精力', value: '−4.8%', group: 'Primary' },
  { id: 'ki', label: '精力回复速度', value: '+7.0%', group: 'Secondary' },
  { id: 'luck', label: '幸运', value: '+31', group: 'Secondary' },
  { id: 'damage', label: '伤害反映（忍术威力）', value: 'AA', group: 'Secondary' },
  { id: 'drop', label: '道具掉落率', value: '+6.0%', group: 'Secondary' },
  { id: 'kuzuryu', label: '九头龙的恩宠', group: 'Grace' },
  { id: 'magatsuhi', label: '祸津日的恩宠', group: 'Grace' },
  { id: 'fukurokuju', label: '寿老人之恩宠', group: 'Grace' },
  { id: 'oni', label: '独眼鬼', group: 'Enemies' },
  { id: 'ghost', label: '幽鬼', group: 'Enemies' },
  { id: 'yoki', label: '妖鬼', group: 'Enemies' },
  { id: 'yamagata', label: '山县昌景', group: 'Enemies' },
  { id: 'soldier', label: '幕府兵', group: 'Enemies' },
  { id: 'alchemy', label: '炼金术师', group: 'Enemies' },
];
export const rules: Record<string, string[]> = {
  'Automatic activation': ['神箭符 · 30 sec', '养身符 · 60 sec'],
  'Increased drops': ['九头龙的恩宠 · 30%', '神器 · 30%', '魂核 · 30%'],
  'Adversity': ['足部防具 · 65%', '头部防具 · 65%'],
  'Damage increase': ['近距离攻击 · 10%', '远距离攻击 · 10%'],
};
export interface ScrollFixture {
  id: string; seed: string; level: number; ng: number; rarity: number;
  capacity: number; conditions: string[]; rules: string[]; terrain: string;
}
// Deliberately illustrative combinations, not native-generated acceptance vectors.
export const fixtures: ScrollFixture[] = [
  { id:'sample-01', seed:'10030565', level:170, ng:3, rarity:4, capacity:5,
    conditions:['hp','ki','luck','damage','kuzuryu','oni','ghost','yoki'],
    rules:['Automatic activation: 神箭符 · 30 sec','Increased drops: 九头龙的恩宠 · 30%'], terrain:'Shrine' },
  { id:'sample-02', seed:'43723117', level:180, ng:3, rarity:4, capacity:7,
    conditions:['toughness','ki','luck','drop','magatsuhi','yamagata','soldier','alchemy'],
    rules:['Adversity: 足部防具 · 65%','Automatic activation: 养身符 · 60 sec'], terrain:'Battlefield' },
  { id:'sample-03', seed:'36526331', level:170, ng:2, rarity:3, capacity:4,
    conditions:['power','ki','luck','damage','fukurokuju','oni','yoki'],
    rules:['Damage increase: 近距离攻击 · 10%'], terrain:'Cave' },
];
export type Query = { selected: string[]; rules: string[]; terrain: string; capacity: string; ng: string; rarity: string; recommended: number; limit: number };
export const initialQuery: Query = {selected:[],rules:[],terrain:'Any',capacity:'Any',ng:'Any',rarity:'Any',recommended:350,limit:3};
export function matchFixtures(query: Query): ScrollFixture[] {
  return fixtures.filter(item => query.selected.every(id => item.conditions.includes(id))
    && query.rules.every(rule => item.rules.includes(rule))
    && (query.terrain === 'Any' || item.terrain === query.terrain)
    && (query.capacity === 'Any' || item.capacity === Number(query.capacity))
    && (query.ng === 'Any' || item.ng === Number(query.ng))
    && (query.rarity === 'Any' || item.rarity === Number(query.rarity)))
    .sort((a,b) => b.capacity-a.capacity).slice(0,query.limit);
}

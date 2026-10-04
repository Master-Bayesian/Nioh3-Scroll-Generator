/**
 * Loadout codes (配装码, #16): one or more equipment pieces as a short text a
 * player can paste anywhere. A code is "N3E1-" followed by base64url bytes:
 *
 *   count, then per piece: item_id, level, level_before_forge, plus, rarity,
 *   hell (0, or hell_skill + 1), effect count, then per effect
 *   (effect_id + 1 mod 2^32) * 2 + star, so an empty slot (0xFFFFFFFF) is one
 *   byte, and value; and a 16-bit checksum of everything before it.
 *
 * Numbers are unsigned LEB128 varints. The codec knows nothing about the item
 * table; callers check that the items and effects exist.
 */

export interface LoadoutPiece {
  item_id: number;
  level: number;
  level_before_forge: number;
  plus: number;
  rarity: number;
  hell?: boolean;
  hell_skill?: number;
  effects: { effect_id: number; value: number; star?: boolean }[];
}

/** A decoded piece always says whether it is a hell item. */
export type DecodedPiece = LoadoutPiece & { hell: boolean; hell_skill: number };

export const LOADOUT_PREFIX = "N3E1-";
const MAX_PIECES = 64;
const MAX_EFFECTS = 7;

function checksum(bytes: number[]): number {
  let hash = 0x811c9dc5;
  for (const byte of bytes) hash = Math.imul(hash ^ byte, 0x01000193) >>> 0;
  return (hash ^ (hash >>> 16)) & 0xffff;
}

function pushVarint(out: number[], value: number) {
  if (!Number.isSafeInteger(value) || value < 0) throw new Error("装备数据里有无法写入配装码的数值。");
  do {
    let byte = value % 128;
    value = Math.floor(value / 128);
    if (value > 0) byte |= 0x80;
    out.push(byte);
  } while (value > 0);
}

const INVALID = "配装码无效：可能复制不完整或被改动过。请重新完整复制以 " + LOADOUT_PREFIX + " 开头的整段文字。";

class Reader {
  offset = 0;
  constructor(private bytes: Uint8Array) {}
  varint(limit = 0xffffffff): number {
    let value = 0;
    let scale = 1;
    for (;;) {
      if (this.offset >= this.bytes.length || scale > 2 ** 35) throw new Error(INVALID);
      const byte = this.bytes[this.offset++];
      value += (byte & 0x7f) * scale;
      if (!(byte & 0x80)) break;
      scale *= 128;
    }
    if (value > limit) throw new Error(INVALID);
    return value;
  }
}

function toBase64Url(bytes: number[]): string {
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary).replaceAll("+", "-").replaceAll("/", "_").replace(/=+$/, "");
}

function fromBase64Url(text: string): Uint8Array {
  if (!/^[A-Za-z0-9_-]*$/.test(text)) throw new Error(INVALID);
  let binary: string;
  try {
    binary = atob(text.replaceAll("-", "+").replaceAll("_", "/") + "===".slice((text.length + 3) % 4));
  } catch {
    throw new Error(INVALID);
  }
  return Uint8Array.from(binary, char => char.charCodeAt(0));
}

export function encodeLoadout(pieces: LoadoutPiece[]): string {
  if (!pieces.length) throw new Error("没有可以生成配装码的装备。");
  if (pieces.length > MAX_PIECES) throw new Error("一个配装码最多包含 " + MAX_PIECES + " 件装备。");
  const out: number[] = [];
  pushVarint(out, pieces.length);
  for (const piece of pieces) {
    pushVarint(out, piece.item_id);
    pushVarint(out, piece.level);
    pushVarint(out, piece.level_before_forge);
    pushVarint(out, piece.plus);
    pushVarint(out, piece.rarity);
    pushVarint(out, piece.hell ? (piece.hell_skill ?? 0) + 1 : 0);
    const effects = piece.effects.slice(0, MAX_EFFECTS);
    pushVarint(out, effects.length);
    for (const effect of effects) {
      pushVarint(out, ((effect.effect_id + 1) % 2 ** 32) * 2 + (effect.star ? 1 : 0));
      pushVarint(out, effect.value);
    }
  }
  const sum = checksum(out);
  out.push(sum & 0xff, sum >> 8);
  return LOADOUT_PREFIX + toBase64Url(out);
}

/** Decode a code; whitespace and line breaks a chat app may add are ignored. */
export function decodeLoadout(code: string): DecodedPiece[] {
  const text = code.replace(/\s+/g, "");
  if (!text) throw new Error("请先粘贴配装码。");
  if (!text.toUpperCase().startsWith(LOADOUT_PREFIX)) {
    if (/^N3E\d+-/i.test(text)) throw new Error("这个配装码来自更新的版本，请先更新工具再导入。");
    throw new Error("这不是配装码。配装码以 " + LOADOUT_PREFIX + " 开头，请检查复制的内容。");
  }
  const bytes = fromBase64Url(text.slice(LOADOUT_PREFIX.length));
  if (bytes.length < 3) throw new Error(INVALID);
  const body = Array.from(bytes.subarray(0, bytes.length - 2));
  const sum = bytes[bytes.length - 2] | (bytes[bytes.length - 1] << 8);
  if (checksum(body) !== sum) throw new Error(INVALID);
  const reader = new Reader(bytes.subarray(0, bytes.length - 2));
  const count = reader.varint(MAX_PIECES);
  if (!count) throw new Error(INVALID);
  const pieces: DecodedPiece[] = [];
  for (let index = 0; index < count; index += 1) {
    const item_id = reader.varint(0xffff);
    const level = reader.varint(0xffff);
    const level_before_forge = reader.varint(0xffff);
    const plus = reader.varint(0xffff);
    const rarity = reader.varint(0xff);
    const hell = reader.varint(0x10000);
    const effectCount = reader.varint(MAX_EFFECTS);
    const effects: LoadoutPiece["effects"] = [];
    for (let slot = 0; slot < effectCount; slot += 1) {
      const id = reader.varint(0x1ffffffff);
      const value = reader.varint();
      effects.push({ effect_id: (Math.floor(id / 2) + 2 ** 32 - 1) % 2 ** 32, value, ...(id % 2 ? { star: true } : {}) });
    }
    pieces.push({ item_id, level, level_before_forge, plus, rarity, hell: hell > 0, hell_skill: hell > 0 ? hell - 1 : 0, effects });
  }
  if (reader.offset !== bytes.length - 2) throw new Error(INVALID);
  return pieces;
}

import { useEffect, useRef, useState } from "react";
import { desktop, retainSample } from "./desktop-bridge";
import type { Sample } from "./model";

export const collectionKey = (s: Sample) =>
  `${s.playthrough ?? 3}:${s.rarity}:${s.level ?? 180}:${s.seed}`;
export function useCollections(notify: (message: string) => void) {
  const [cart, setCart] = useState<Sample[]>([]);
  const [favorites, setFavorites] = useState<Sample[]>([]);
  const [cartPending, setCartPending] = useState<string[]>([]);
  const pending = useRef(new Set<string>());
  const cartRef = useRef(cart);
  cartRef.current = cart;
  useEffect(() => {
    if (desktop)
      void window.review
        .favorites({ action: "list" })
        .then(setFavorites)
        .catch((e) => notify(String(e)));
    else
      try {
        setFavorites(
          JSON.parse(localStorage.getItem("nioh3-favorites") || "[]").slice(
            0,
            50,
          ),
        );
      } catch {
        notify("收藏夹读取失败。");
      }
  }, []);
  async function addCart(sample: Sample) {
    const key = collectionKey(sample);
    if (
      cartRef.current.some((s) => collectionKey(s) === key) ||
      pending.current.has(key)
    )
      return;
    if (cartRef.current.length + pending.current.size >= 50) {
      notify("购物车最多保存 50 张绘卷。");
      return;
    }
    pending.current.add(key);
    setCartPending(items=>[...items,key]);
    try {
      const retained = desktop ? await retainSample(sample) : sample;
      cartRef.current = [...cartRef.current, retained];
      setCart(cartRef.current);
    } catch (e) {
      notify(String(e));
    } finally {
      pending.current.delete(key);
      setCartPending(items=>items.filter(value=>value!==key));
    }
  }
  function removeCart(sample: Sample) {
    cartRef.current = cartRef.current.filter(
      (s) => collectionKey(s) !== collectionKey(sample),
    );
    setCart(cartRef.current);
  }
  async function toggleFavorite(sample: Sample) {
    const key = collectionKey(sample),
      existing = favorites.some((s) => collectionKey(s) === key);
    if (pending.current.has("favorite:" + key)) return;
    if (!existing && favorites.length >= 50) {
      notify("收藏夹最多保存 50 张绘卷。");
      return;
    }
    pending.current.add("favorite:" + key);
    try {
      if (desktop) {
        const retained = existing ? sample : await retainSample(sample);
        setFavorites(
          await window.review.favorites({
            action: existing ? "remove" : "add",
            key,
            sample: retained,
            reference_id: retained.backend?.referenceId,
          }),
        );
      } else {
        setFavorites((items) => {
          const next = existing
            ? items.filter((s) => collectionKey(s) !== key)
            : items.length >= 50
              ? items
              : [...items, sample];
          localStorage.setItem("nioh3-favorites", JSON.stringify(next));
          return next;
        });
      }
    } catch (e) {
      notify(String(e));
    } finally {
      pending.current.delete("favorite:" + key);
    }
  }
  return { cart, favorites, cartPending, addCart, removeCart, toggleFavorite };
}

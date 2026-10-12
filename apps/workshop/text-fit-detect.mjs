// Finds text that does not fit: clipped without a hover title, escaping its box,
// overlapping other text, controls drawn outside their card, and selects whose
// chosen option is cut. Runs inside the page (page.evaluate).
export const detectTextFit = () => {
  const path = el => { const a = []; for (let e = el; e && e !== document.body && a.length < 4; e = e.parentElement) a.unshift(e.tagName.toLowerCase() + (e.className && typeof e.className === 'string' ? '.' + e.className.trim().split(/\s+/).slice(0, 2).join('.') : '')); return a.join('>'); };
  const vis = el => { const r = el.getBoundingClientRect(); const cs = getComputedStyle(el); return r.width > 0 && r.height > 0 && cs.visibility !== 'hidden' && cs.display !== 'none' && +cs.opacity !== 0; };
  const res = [];
  for (const el of document.querySelectorAll('body *')) {
    if (!vis(el)) continue;
    const own = [...el.childNodes].some(n => n.nodeType === 3 && n.textContent.trim());
    if (!own || el.closest('.visually-hidden')) continue;
    const cs = getComputedStyle(el);
    const txt = el.textContent.trim().replace(/\s+/g, ' ').slice(0, 50);
    // A deliberate one-line summary keeps its full text in a hover title.
    const titled = (el.getAttribute('title') || '').length >= el.textContent.trim().length;
    if (!titled && el.scrollWidth > el.clientWidth + 1 && cs.overflowX !== 'visible' && cs.overflowX !== 'auto' && cs.overflowX !== 'scroll') res.push({k: 'clipX', txt, at: path(el), d: el.scrollWidth - el.clientWidth});
    if (el.scrollHeight > el.clientHeight + 2 && cs.overflowY === 'hidden' && !cs.webkitLineClamp) res.push({k: 'clipY', txt, at: path(el), d: el.scrollHeight - el.clientHeight});
    const range = document.createRange(); range.selectNodeContents(el); const tr = range.getBoundingClientRect();
    if (tr.width === 0 || titled) continue;
    for (let a = el.parentElement; a && a !== document.body; a = a.parentElement) {
      const ac = getComputedStyle(a);
      if (ac.overflowX === 'visible' && ac.overflowY === 'visible') continue;
      const ar = a.getBoundingClientRect();
      const horiz = tr.right > ar.right + 1 || tr.left < ar.left - 1;
      const vert = ac.overflowY === 'hidden' && (tr.bottom > ar.bottom + 1 || tr.top < ar.top - 1);
      const inView = tr.bottom > ar.top && tr.top < ar.bottom;
      if ((horiz && inView) || vert) res.push({k: 'escape', txt, at: path(el), box: path(a)});
      break;
    }
  }
  // text of one element drawn over the text of another
  const boxes = [];
  for (const el of document.querySelectorAll('body *')) {
    if (!vis(el) || el.closest('.visually-hidden')) continue;
    for (const n of el.childNodes) {
      if (n.nodeType !== 3 || !n.textContent.trim()) continue;
      const r = document.createRange(); r.selectNodeContents(n);
      for (let q of r.getClientRects()) { q = {left: q.left, right: q.right, top: q.top, bottom: q.bottom}; for (let a = el; a && a !== document.body; a = a.parentElement) { const ac = getComputedStyle(a); if (ac.overflowX === 'visible' && ac.overflowY === 'visible') continue; const ar = a.getBoundingClientRect(); q = {left: Math.max(q.left, ar.left), right: Math.min(q.right, ar.right), top: Math.max(q.top, ar.top), bottom: Math.min(q.bottom, ar.bottom)}; } if (q.right - q.left > 2 && q.bottom - q.top > 2) boxes.push({el, q, txt: el.textContent.trim().replace(/\s+/g, ' ').slice(0, 40)}); }
    }
  }
  for (let i = 0; i < boxes.length; i++) for (let j = i + 1; j < boxes.length; j++) {
    const a = boxes[i], c = boxes[j];
    if (a.el === c.el || a.el.contains(c.el) || c.el.contains(a.el)) continue;
    // An open popup or dialog covers the page by design.
    const layer = e => e.closest(".side-popup,dialog,[role=dialog],.toast,.popover");
    if (layer(a.el) !== layer(c.el)) continue;
    const w = Math.min(a.q.right, c.q.right) - Math.max(a.q.left, c.q.left), h = Math.min(a.q.bottom, c.q.bottom) - Math.max(a.q.top, c.q.top);
    if (w > 3 && h > 3) {
      const top = document.elementFromPoint((Math.max(a.q.left, c.q.left) + Math.min(a.q.right, c.q.right)) / 2, (Math.max(a.q.top, c.q.top) + Math.min(a.q.bottom, c.q.bottom)) / 2);
      if (top && (a.el.contains(top) || c.el.contains(top) || top.contains(a.el) || top.contains(c.el))) res.push({k: 'overlap', txt: a.txt + ' ⟷ ' + c.txt, at: path(a.el)});
    }
  }
  // text or a control drawn outside the bordered card that holds it
  // A scrolling or clipping box on the way is not a card: content past its edge is scrolled away, not drawn outside.
  const card = el => { for (let a = el.parentElement; a && a !== document.body; a = a.parentElement) { const cs = getComputedStyle(a); if (cs.position === 'fixed' || cs.position === 'absolute' || cs.overflowX !== 'visible' || cs.overflowY !== 'visible') return null; if (parseFloat(cs.borderTopWidth) > 0 && parseFloat(cs.borderBottomWidth) > 0 && cs.display !== 'inline') return a; } return null; };
  const items = [...document.querySelectorAll('select,input:not([type=checkbox]):not([type=radio]),button')].filter(vis).map(el => ({el, q: el.getBoundingClientRect(), txt: (el.value || el.textContent || '').trim().slice(0, 40)}));
  for (const it of [...boxes, ...items]) {
    const c = card(it.el); if (!c) continue;
    const r = c.getBoundingClientRect();
    if (it.q.bottom > r.bottom + 2 || it.q.top < r.top - 2 || it.q.right > r.right + 2 || it.q.left < r.left - 2) res.push({k: 'outside-card', txt: it.txt, at: path(it.el), box: path(c)});
  }
  // a select whose chosen option does not fit
  const cv = document.createElement('canvas').getContext('2d');
  for (const sel of document.querySelectorAll('select')) {
    if (!vis(sel) || sel.style.opacity === '0' || getComputedStyle(sel).opacity === '0') continue;
    const cs = getComputedStyle(sel); cv.font = cs.font;
    const t = sel.selectedOptions[0]?.textContent.trim() || '';
    const room = sel.clientWidth - parseFloat(cs.paddingLeft) - parseFloat(cs.paddingRight) - 18;
    if (cv.measureText(t).width > room + 1) res.push({k: 'select', txt: t.slice(0, 50), at: path(sel), d: Math.round(cv.measureText(t).width - room)});
  }
  // the page itself must never scroll sideways
  if (document.documentElement.scrollWidth > innerWidth + 1) res.push({k: 'pageX', d: document.documentElement.scrollWidth - innerWidth});
  const seen = new Set();
  return res.filter(r => { const key = r.k + r.txt + r.at; if (seen.has(key)) return false; seen.add(key); return true; });
};

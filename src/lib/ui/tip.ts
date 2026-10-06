// A custom tooltip, shown after a short hover. One shared element follows
// whichever control is hovered, gliding between them.

let el: HTMLDivElement | null = null;
let timer: ReturnType<typeof setTimeout> | undefined;
let warm = false;
let coolTimer: ReturnType<typeof setTimeout> | undefined;

function ensure(): HTMLDivElement {
  if (el) return el;
  el = document.createElement("div");
  el.className = "tip";
  el.setAttribute("role", "tooltip");
  document.body.appendChild(el);
  return el;
}

function show(node: HTMLElement, text: string, side: "top" | "bottom") {
  const t = ensure();
  t.textContent = text;
  t.dataset.side = side;
  const r = node.getBoundingClientRect();
  const w = t.offsetWidth;
  const h = t.offsetHeight;
  let x = r.left + r.width / 2 - w / 2;
  x = Math.max(6, Math.min(x, window.innerWidth - w - 6));
  let y = side === "top" ? r.top - h - 8 : r.bottom + 8;
  if (y < 4) y = r.bottom + 8;
  if (y + h > window.innerHeight - 4) y = r.top - h - 8;
  t.style.transform = `translate(${Math.round(x)}px, ${Math.round(y)}px)`;
  t.classList.add("on");
  warm = true;
  clearTimeout(coolTimer);
}

function hide() {
  clearTimeout(timer);
  el?.classList.remove("on");
  clearTimeout(coolTimer);
  coolTimer = setTimeout(() => (warm = false), 400);
}

export function tip(node: HTMLElement, opts: string | { text: string; side?: "top" | "bottom" }) {
  let o = typeof opts === "string" ? { text: opts } : opts;
  if (!node.getAttribute("aria-label")) node.setAttribute("aria-label", o.text);
  const enter = () => {
    clearTimeout(timer);
    // Once one tip is showing, the next appears at once.
    timer = setTimeout(() => show(node, o.text, o.side ?? "top"), warm ? 0 : 550);
  };
  node.addEventListener("pointerenter", enter);
  node.addEventListener("pointerleave", hide);
  node.addEventListener("pointerdown", hide);
  return {
    update(next: typeof opts) {
      o = typeof next === "string" ? { text: next } : next;
      node.setAttribute("aria-label", o.text);
      if (el?.classList.contains("on") && node.matches(":hover")) show(node, o.text, o.side ?? "top");
    },
    destroy() {
      node.removeEventListener("pointerenter", enter);
      node.removeEventListener("pointerleave", hide);
      node.removeEventListener("pointerdown", hide);
      hide();
    },
  };
}

export function hideTip() {
  hide();
}

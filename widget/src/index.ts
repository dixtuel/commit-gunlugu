// Musteri sitesine tek satirla gomulur:
// <script src="https://cg.sh/w/{widgetKey}.js" async></script>
// Framework'suz vanilla JS - bagimlilik yok, DOM'a dokunmadan once script'in kendi
// <script> etiketinden widgetKey'i cikarir.

type Entry = { id: string; category: "NEW" | "FIX" | "IMPROVEMENT"; title: string; body: string; publishedAt: string };
type WidgetData = {
  brand: { name: string; color: string; logoUrl: string | null; showPoweredBy: boolean };
  changelogUrl: string;
  entries: Entry[];
};

const API_ORIGIN = "https://commit-gunlugu.com";
const STORAGE_KEY = "cg_last_seen";

function getWidgetKey(): string | null {
  const current = document.currentScript as HTMLScriptElement | null;
  const src = current?.src ?? "";
  const match = src.match(/\/w\/([a-zA-Z0-9]+)\.js/);
  return match ? match[1]! : null;
}

function unreadCount(entries: Entry[]): number {
  const lastSeen = Number(localStorage.getItem(STORAGE_KEY) ?? 0);
  return entries.filter((e) => new Date(e.publishedAt).getTime() > lastSeen).length;
}

function categoryLabel(c: Entry["category"]): string {
  return { NEW: "yeni", FIX: "duzeltme", IMPROVEMENT: "iyilestirme" }[c];
}

function render(data: WidgetData) {
  const unread = unreadCount(data.entries);

  const badge = document.createElement("button");
  badge.setAttribute("aria-label", "Yenilikler");
  badge.style.cssText = `
    position: fixed; right: 20px; bottom: 20px; z-index: 999999;
    background: ${data.brand.color}; color: #fff; border: none; border-radius: 999px;
    padding: 10px 16px; font: 600 13px system-ui, sans-serif; cursor: pointer;
    box-shadow: 0 8px 24px -8px rgba(0,0,0,.35); display: flex; align-items: center; gap: 8px;
  `;
  badge.textContent = unread > 0 ? `${unread} yeni guncelleme` : "Yenilikler";

  const panel = document.createElement("div");
  panel.style.cssText = `
    position: fixed; right: 20px; bottom: 76px; z-index: 999999; width: 340px; max-height: 420px;
    overflow-y: auto; background: #fff; color: #1b201c; border-radius: 12px;
    box-shadow: 0 12px 32px -12px rgba(0,0,0,.35); display: none; font: 400 14px system-ui, sans-serif;
  `;

  panel.innerHTML = data.entries
    .map(
      (e) => `
      <div style="padding:14px 16px;border-bottom:1px solid #eee">
        <div style="font-size:11px;text-transform:uppercase;letter-spacing:.04em;color:${data.brand.color};margin-bottom:4px">${categoryLabel(e.category)}</div>
        <div style="font-weight:600;margin-bottom:4px">${e.title}</div>
        <div style="color:#555;font-size:13px">${e.body}</div>
      </div>`,
    )
    .join("");

  if (data.brand.showPoweredBy) {
    const footer = document.createElement("a");
    footer.href = "https://commit-gunlugu.com";
    footer.target = "_blank";
    footer.rel = "noopener";
    footer.textContent = "Commit Gunlugu ile calisir";
    footer.style.cssText = "display:block;padding:10px 16px;font-size:11px;color:#999;text-decoration:none";
    panel.appendChild(footer);
  }

  badge.addEventListener("click", () => {
    const isOpen = panel.style.display === "block";
    panel.style.display = isOpen ? "none" : "block";
    if (!isOpen) {
      localStorage.setItem(STORAGE_KEY, String(Date.now()));
      badge.textContent = "Yenilikler";
    }
  });

  document.body.appendChild(panel);
  document.body.appendChild(badge);
}

async function init() {
  const widgetKey = getWidgetKey();
  if (!widgetKey) return;

  const res = await fetch(`${API_ORIGIN}/api/widget/${widgetKey}`);
  if (!res.ok) return;

  const data = (await res.json()) as WidgetData;
  if (data.entries.length === 0) return;

  render(data);
}

init();

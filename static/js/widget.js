/**
 * Commit Günlüğü — Gömülebilir Sürüm Günlüğü Widget'ı
 * Bağımsız (Zero-dependency) ve Shadow DOM ile stil çakışmasız.
 * Kullanım:
 * <script src="https://commit.dixtuel.tr/static/js/widget.js" data-key="w_xyz..." async></script>
 */
(function () {
  'use strict';

  const STORAGE_KEY = 'cg_widget_last_seen';

  const currentScriptTag = document.currentScript;

  function getScriptConfig() {
    let current = currentScriptTag || document.currentScript;
    if (!current) {
      current = document.querySelector('script[data-key]') || document.querySelector('script[src*="widget.js"]');
    }
    if (!current) return null;

    let key = current.getAttribute('data-key');
    let origin = '';

    try {
      const url = new URL(current.src);
      origin = url.origin;
      if (!key) {
        key = url.searchParams.get('key');
      }
    } catch (e) {
      origin = window.location.origin;
    }

    return key ? { key, origin } : null;
  }

  function categoryLabel(cat) {
    switch (cat) {
      case 'NEW': return 'YENİ';
      case 'FIX': return 'DÜZELTME';
      default: return 'İYİLEŞTİRME';
    }
  }

  function categoryClass(cat) {
    switch (cat) {
      case 'NEW': return 'pill-new';
      case 'FIX': return 'pill-fix';
      default: return 'pill-improvement';
    }
  }

  async function initWidget() {
    const config = getScriptConfig();
    if (!config || !config.key) {
      console.warn('[Commit Günlüğü] Widget anahtarı (data-key) bulunamadı.');
      return;
    }

    try {
      const res = await fetch(`${config.origin}/api/v1/widget/${config.key}`);
      if (!res.ok) return;

      const data = await res.json();
      if (!data.entries || data.entries.length === 0) return;

      createWidgetUI(data, config.origin);
    } catch (e) {
      console.error('[Commit Günlüğü] Widget yükleme hatası:', e);
    }
  }

  function createWidgetUI(data, origin) {
    const host = document.createElement('div');
    host.id = 'commit-gunlugu-widget-root';
    document.body.appendChild(host);

    const shadow = host.attachShadow({ mode: 'open' });
    const lastSeen = Number(localStorage.getItem(STORAGE_KEY) || 0);
    const unreadCount = data.entries.filter(e => new Date(e.published_at).getTime() > lastSeen).length;

    const brandColor = data.brand.color || '#10b981';

    const style = document.createElement('style');
    style.textContent = `
      * { box-sizing: border-box; margin: 0; padding: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif; }
      .cg-badge-btn {
        position: fixed; right: 20px; bottom: 20px; z-index: 999999;
        background: ${brandColor}; color: #ffffff; border: none; border-radius: 999px;
        padding: 10px 18px; font-size: 13px; font-weight: 600; cursor: pointer;
        display: flex; align-items: center; gap: 8px; box-shadow: 0 8px 24px -6px rgba(0,0,0,0.3);
        transition: transform 0.15s ease, box-shadow 0.15s ease;
      }
      .cg-badge-btn:hover { transform: translateY(-2px); box-shadow: 0 12px 28px -6px rgba(0,0,0,0.4); }
      .cg-unread-dot {
        background: #ef4444; color: #fff; font-size: 11px; padding: 2px 6px;
        border-radius: 999px; font-weight: 700;
      }
      .cg-panel {
        position: fixed; right: 20px; bottom: 74px; z-index: 999999; width: 360px; max-width: calc(100vw - 40px);
        max-height: 480px; background: #ffffff; color: #1f2328; border-radius: 12px;
        box-shadow: 0 16px 40px -10px rgba(0,0,0,0.25); display: none; flex-direction: column;
        overflow: hidden; border: 1px solid #e1e4e8; animation: cgFadeIn 0.18s ease-out;
      }
      @keyframes cgFadeIn {
        from { opacity: 0; transform: translateY(10px); }
        to { opacity: 1; transform: translateY(0); }
      }
      .cg-header {
        padding: 14px 16px; border-bottom: 1px solid #f0f2f5; display: flex;
        justify-content: space-between; align-items: center; background: #fafbfc;
      }
      .cg-header h4 { font-size: 14px; font-weight: 700; color: #24292f; }
      .cg-close-btn { background: none; border: none; font-size: 18px; color: #6e7781; cursor: pointer; }
      .cg-content { overflow-y: auto; flex: 1; padding: 12px 16px; }
      .cg-item { padding: 12px 0; border-bottom: 1px solid #f0f2f5; }
      .cg-item:last-child { border-bottom: none; }
      .cg-meta { display: flex; align-items: center; gap: 6px; margin-bottom: 4px; }
      .cg-pill { font-size: 10px; font-weight: 700; padding: 1px 6px; border-radius: 4px; }
      .pill-new { background: #d1fae5; color: #065f46; }
      .pill-fix { background: #fee2e2; color: #991b1b; }
      .pill-improvement { background: #dbeafe; color: #1e40af; }
      .cg-date { font-size: 11px; color: #8c959f; }
      .cg-author { font-size: 11px; color: ${brandColor}; font-weight: 600; }
      .cg-title { font-size: 13px; font-weight: 600; color: #24292f; margin-bottom: 4px; line-height: 1.4; }
      .cg-body { font-size: 12px; color: #57606a; line-height: 1.5; }
      .cg-footer {
        padding: 10px 16px; border-top: 1px solid #f0f2f5; background: #fafbfc;
        display: flex; justify-content: space-between; align-items: center; font-size: 11px;
      }
      .cg-footer a { color: ${brandColor}; text-decoration: none; font-weight: 600; }
      .cg-footer a:hover { text-decoration: underline; }
      .cg-brand { color: #8c959f; text-decoration: none; }
    `;
    shadow.appendChild(style);

    const btn = document.createElement('button');
    btn.className = 'cg-badge-btn';
    btn.innerHTML = `
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="vertical-align: -2px; margin-right: 4px;"><path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"/><path d="M13.73 21a2 2 0 0 1-3.46 0"/></svg>
      <span>Yenilikler</span>
      ${unreadCount > 0 ? `<span class="cg-unread-dot">${unreadCount}</span>` : ''}
    `;

    const panel = document.createElement('div');
    panel.className = 'cg-panel';

    const entriesHtml = data.entries.slice(0, 5).map(e => `
      <div class="cg-item">
        <div class="cg-meta">
          <span class="cg-pill ${categoryClass(e.category)}">${categoryLabel(e.category)}</span>
          <span class="cg-date">${(e.published_at || '').substring(0, 10)}</span>
          ${e.author ? `<span class="cg-author">${e.author}</span>` : ''}
        </div>
        <div class="cg-title">${escapeHtml(e.title)}</div>
        <div class="cg-body">${escapeHtml(e.body)}</div>
      </div>
    `).join('');

    panel.innerHTML = `
      <div class="cg-header">
        <h4>${escapeHtml(data.brand.name || 'Yenilikler')}</h4>
        <button class="cg-close-btn">&times;</button>
      </div>
      <div class="cg-content">
        ${entriesHtml}
      </div>
      <div class="cg-footer">
        <a href="${data.changelog_url}" target="_blank" rel="noopener">Tümünü İncele &rarr;</a>
        <a href="${origin}" target="_blank" rel="noopener" class="cg-brand">Commit Günlüğü</a>
      </div>
    `;

    btn.addEventListener('click', () => {
      const isVisible = panel.style.display === 'flex';
      panel.style.display = isVisible ? 'none' : 'flex';
      if (!isVisible) {
        localStorage.setItem(STORAGE_KEY, String(Date.now()));
        const dot = btn.querySelector('.cg-unread-dot');
        if (dot) dot.remove();
      }
    });

    panel.querySelector('.cg-close-btn').addEventListener('click', () => {
      panel.style.display = 'none';
    });

    shadow.appendChild(panel);
    shadow.appendChild(btn);
  }

  function escapeHtml(str) {
    if (!str) return '';
    return str.replace(/[&<>'"]/g, tag => ({
      '&': '&amp;',
      '<': '&lt;',
      '>': '&gt;',
      "'": '&#39;',
      '"': '&quot;'
    }[tag] || tag));
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initWidget);
  } else {
    initWidget();
  }
})();

/**
 * Commit Günlüğü — Gömülebilir Sürüm Günlüğü & Değişiklik Akışı Widget'ı
 * Bağımsız (Zero-dependency), AdBlocker dostu, tam özelleştirilebilir ve Shadow DOM korumalı.
 *
 * Parametreler (HTML data-* özellikleri):
 * - data-key: Proje widget anahtarı (zorunlu)
 * - data-layout: 'card' (varsayılan) | 'box' / 'square' | 'strip' / 'banner' | 'compact'
 * - data-limit: Gösterilecek commit/sürüm sayısı (ör. 1, 2, 3, 5 - varsayılan: layout'a göre 1-3)
 * - data-width: Özel genişlik (ör. '300px', '100%', '280px')
 * - data-height: Özel yükseklik (ör. 'auto', '200px')
 * - data-max-width: Maksimum genişlik (ör. '320px', 'none')
 * - data-radius: Köşe yuvarlaklığı (ör. '8px', '16px', '0px')
 * - data-theme: 'light' | 'dark' | 'auto' (varsayılan: 'auto')
 * - data-brand-color: Özel vurgu rengi (ör. '#3b82f6')
 * - data-show-desc: Açıklama metni görünsün mü? 'true' | 'false' (varsayılan: true)
 * - data-show-date: Tarih görünsün mü? 'true' | 'false' (varsayılan: true)
 * - data-show-pill: [YENİ]/[DÜZELTME] rozeti görünsün mü? 'true' | 'false' (varsayılan: true)
 * - data-show-footer: Altbilgi çubuğu görünsün mü? 'true' | 'false' (varsayılan: true)
 */
(function () {
  'use strict';

  const STORAGE_KEY = 'cg_widget_last_seen';
  const DEFAULT_ORIGIN = 'https://commit.dixtuel.tr';
  let isInitialized = false;

  const currentScriptTag = document.currentScript;
  const dataCache = new Map();

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

  function getScriptConfig() {
    let current = currentScriptTag || document.currentScript;
    if (!current) {
      current = document.querySelector('script[data-key]') || document.querySelector('script[src*="widget.js"]');
    }

    let origin = DEFAULT_ORIGIN;
    let key = null;
    let mode = 'badge';
    let layout = 'card';
    let theme = 'auto';
    let limit = 5;

    if (current) {
      key = current.getAttribute('data-key');
      try {
        if (current.src) {
          const url = new URL(current.src, window.location.href);
          origin = url.origin;
          if (!key) {
            key = url.searchParams.get('key');
          }
        }
      } catch (e) {
        origin = window.location.origin || DEFAULT_ORIGIN;
      }

      mode = current.getAttribute('data-mode') || 'badge';
      layout = current.getAttribute('data-layout') || 'card';
      theme = current.getAttribute('data-theme') || 'auto';
      limit = parseInt(current.getAttribute('data-limit') || '5', 10);
    }

    return { key, origin, mode, layout, theme, limit, scriptElement: current };
  }

  async function fetchWidgetData(origin, key) {
    const targetOrigin = origin || DEFAULT_ORIGIN;
    const cacheKey = `${targetOrigin}:${key}`;
    if (dataCache.has(cacheKey)) {
      return dataCache.get(cacheKey);
    }

    try {
      const res = await fetch(`${targetOrigin}/api/v1/widget/${key}`);
      if (!res.ok) {
        throw new Error(`HTTP ${res.status}`);
      }
      const data = await res.json();
      dataCache.set(cacheKey, data);
      return data;
    } catch (err) {
      dataCache.delete(cacheKey);
      throw err;
    }
  }

  function getCommonStyles(brandColor, theme, customWidth, customMaxWidth, customHeight, customRadius) {
    return `
      * { box-sizing: border-box; margin: 0; padding: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif; }
      
      :host {
        --cg-brand: ${brandColor};
        --cg-bg: #ffffff;
        --cg-text: #1e293b;
        --cg-text-muted: #64748b;
        --cg-text-dim: #94a3b8;
        --cg-border: #e2e8f0;
        --cg-surface: #f8fafc;
        --cg-surface-hover: #f1f5f9;
        --cg-card-shadow: 0 4px 16px -2px rgba(0,0,0,0.06), 0 2px 6px -1px rgba(0,0,0,0.04);
        --cg-w: ${customWidth || '100%'};
        --cg-max-w: ${customMaxWidth || 'none'};
        --cg-h: ${customHeight || 'auto'};
        --cg-radius: ${customRadius || '14px'};
        display: block;
        width: var(--cg-w);
        max-width: var(--cg-max-w);
      }

      ${theme === 'dark' ? `
        :host {
          --cg-bg: #0f172a;
          --cg-text: #f8fafc;
          --cg-text-muted: #94a3b8;
          --cg-text-dim: #64748b;
          --cg-border: #334155;
          --cg-surface: #1e293b;
          --cg-surface-hover: #293548;
          --cg-card-shadow: 0 4px 20px -2px rgba(0,0,0,0.4);
        }
      ` : theme === 'auto' ? `
        @media (prefers-color-scheme: dark) {
          :host {
            --cg-bg: #0f172a;
            --cg-text: #f8fafc;
            --cg-text-muted: #94a3b8;
            --cg-text-dim: #64748b;
            --cg-border: #334155;
            --cg-surface: #1e293b;
            --cg-surface-hover: #293548;
            --cg-card-shadow: 0 4px 20px -2px rgba(0,0,0,0.4);
          }
        }
      ` : ''}

      .pill {
        font-size: 10px; font-weight: 700; padding: 2px 7px; border-radius: 4px;
        letter-spacing: 0.03em; text-transform: uppercase; display: inline-block;
      }
      .pill-new { background: rgba(16, 185, 129, 0.15); color: #059669; }
      .pill-fix { background: rgba(239, 68, 68, 0.15); color: #dc2626; }
      .pill-improvement { background: rgba(59, 130, 246, 0.15); color: #2563eb; }

      @media (prefers-color-scheme: dark) {
        ${theme !== 'light' ? `
          .pill-new { background: rgba(16, 185, 129, 0.25); color: #34d399; }
          .pill-fix { background: rgba(239, 68, 68, 0.25); color: #f87171; }
          .pill-improvement { background: rgba(59, 130, 246, 0.25); color: #60a5fa; }
        ` : ''}
      }
    `;
  }

  // --- 1. POPUP / BADGE WIDGET (Sağ alttaki buton) ---
  function renderBadgeWidget(data, origin) {
    if (document.getElementById('commit-gunlugu-widget-root')) return;

    const host = document.createElement('div');
    host.id = 'commit-gunlugu-widget-root';
    document.body.appendChild(host);

    const shadow = host.attachShadow({ mode: 'open' });
    const lastSeen = Number(localStorage.getItem(STORAGE_KEY) || 0);
    const unreadCount = (data.entries || []).filter(e => new Date(e.published_at).getTime() > lastSeen).length;
    const brandColor = data.brand?.color || '#5b8a7a';

    const style = document.createElement('style');
    style.textContent = `
      ${getCommonStyles(brandColor, 'auto', 'auto', 'none', 'auto', '12px')}
      .cg-badge-btn {
        position: fixed; right: 20px; bottom: 20px; z-index: 999999;
        background: var(--cg-brand); color: #ffffff; border: none; border-radius: 999px;
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
        max-height: 480px; background: var(--cg-bg); color: var(--cg-text); border-radius: 12px;
        box-shadow: 0 16px 40px -10px rgba(0,0,0,0.25); display: none; flex-direction: column;
        overflow: hidden; border: 1px solid var(--cg-border); animation: cgFadeIn 0.18s ease-out;
      }
      @keyframes cgFadeIn {
        from { opacity: 0; transform: translateY(10px); }
        to { opacity: 1; transform: translateY(0); }
      }
      .cg-header {
        padding: 14px 16px; border-bottom: 1px solid var(--cg-border); display: flex;
        justify-content: space-between; align-items: center; background: var(--cg-surface);
      }
      .cg-header h4 { font-size: 14px; font-weight: 700; color: var(--cg-text); }
      .cg-close-btn { background: none; border: none; font-size: 18px; color: var(--cg-text-muted); cursor: pointer; }
      .cg-content { overflow-y: auto; flex: 1; padding: 12px 16px; }
      .cg-item { padding: 12px 0; border-bottom: 1px solid var(--cg-border); }
      .cg-item:last-child { border-bottom: none; }
      .cg-meta { display: flex; align-items: center; gap: 6px; margin-bottom: 4px; }
      .cg-date { font-size: 11px; color: var(--cg-text-dim); }
      .cg-author { font-size: 11px; color: var(--cg-brand); font-weight: 600; }
      .cg-title { font-size: 13px; font-weight: 600; color: var(--cg-text); margin-bottom: 4px; line-height: 1.4; }
      .cg-body { font-size: 12px; color: var(--cg-text-muted); line-height: 1.5; }
      .cg-footer {
        padding: 10px 16px; border-top: 1px solid var(--cg-border); background: var(--cg-surface);
        display: flex; justify-content: space-between; align-items: center; font-size: 11px;
      }
      .cg-footer a { color: var(--cg-brand); text-decoration: none; font-weight: 600; }
      .cg-footer a:hover { text-decoration: underline; }
      .cg-brand { color: var(--cg-text-dim); text-decoration: none; }
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

    const entriesHtml = (data.entries || []).slice(0, 5).map(e => `
      <div class="cg-item">
        <div class="cg-meta">
          <span class="pill ${categoryClass(e.category)}">${categoryLabel(e.category)}</span>
          <span class="cg-date">${(e.published_at || '').substring(0, 10)}</span>
          ${e.author ? `<span class="cg-author">${e.author}</span>` : ''}
        </div>
        <div class="cg-title">${escapeHtml(e.title)}</div>
        <div class="cg-body">${escapeHtml(e.body)}</div>
      </div>
    `).join('');

    panel.innerHTML = `
      <div class="cg-header">
        <h4>${escapeHtml(data.brand?.name || 'Yenilikler')}</h4>
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

  // --- 2. SAYFA İÇİ GÖMÜLÜ WIDGET (AYARLANABİLİR BOYUT VE SAYI) ---
  function renderInlineWidget(container, data, origin, opts) {
    if (container.shadowRoot) return;

    const shadow = container.attachShadow({ mode: 'open' });
    const brandColor = opts.brandColor || data.brand?.color || '#5b8a7a';
    const rawLayout = (opts.layout || 'card').toLowerCase();
    
    // Layout seçimi
    let layout = 'card';
    if (rawLayout === 'square' || rawLayout === 'box') layout = 'box';
    else if (rawLayout === 'banner' || rawLayout === 'strip' || rawLayout === 'horizontal') layout = 'strip';
    else if (rawLayout === 'compact' || rawLayout === 'mini' || rawLayout === 'minimal') layout = 'compact';

    const theme = opts.theme || 'auto';
    
    // Gösterilecek kayıt sayısı (Kullanıcının verdiği data-limit esastır!)
    let defaultLimit = 3;
    if (layout === 'box') defaultLimit = 2;
    else if (layout === 'strip' || layout === 'compact') defaultLimit = 1;

    const limit = opts.limit !== undefined && opts.limit > 0 ? opts.limit : defaultLimit;

    // Görünüm anahtarları
    const showDesc = opts.showDesc !== false;
    const showDate = opts.showDate !== false;
    const showPill = opts.showPill !== false;
    const showFooter = opts.showFooter !== false;

    // Boyutlar
    const customWidth = opts.width || null;
    const customMaxWidth = opts.maxWidth || (layout === 'box' ? '360px' : layout === 'compact' ? '300px' : layout === 'strip' ? '100%' : '440px');
    const customHeight = opts.height || null;
    const customRadius = opts.radius || (layout === 'strip' ? '12px' : layout === 'box' ? '16px' : '14px');

    const style = document.createElement('style');
    let layoutSpecificStyles = '';

    if (layout === 'box') {
      // 1:1 Kare / Kutu (Sidebar / Grid için)
      const minH = limit === 1 ? 'auto' : '280px';
      layoutSpecificStyles = `
        .cg-wrap-box {
          background: var(--cg-bg);
          color: var(--cg-text);
          border: 1px solid var(--cg-border);
          border-radius: var(--cg-radius);
          padding: 18px;
          box-shadow: var(--cg-card-shadow);
          display: flex;
          flex-direction: column;
          justify-content: space-between;
          width: 100%;
          min-height: ${minH};
          height: var(--cg-h);
          transition: transform 0.2s ease, box-shadow 0.2s ease;
        }
        .cg-wrap-box:hover {
          transform: translateY(-2px);
          box-shadow: 0 8px 24px -4px rgba(0,0,0,0.08);
        }
        .cg-box-head {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding-bottom: 12px;
          border-bottom: 1px solid var(--cg-border);
        }
        .cg-box-title {
          font-size: 14px;
          font-weight: 700;
          color: var(--cg-text);
          display: flex;
          align-items: center;
          gap: 8px;
        }
        .cg-pulse-dot {
          width: 8px; height: 8px; border-radius: 50%; background: var(--cg-brand);
          display: inline-block; box-shadow: 0 0 0 2px rgba(16, 185, 129, 0.25);
        }
        .cg-box-entries {
          flex: 1;
          display: flex;
          flex-direction: column;
          gap: 12px;
          padding: 12px 0;
          overflow: hidden;
        }
        .cg-entry-unit { display: flex; flex-direction: column; gap: 4px; }
        .cg-entry-meta { display: flex; align-items: center; gap: 6px; }
        .cg-entry-date { font-size: 11px; color: var(--cg-text-dim); }
        .cg-entry-title { font-size: 13px; font-weight: 600; color: var(--cg-text); line-height: 1.35; }
        .cg-entry-desc {
          font-size: 12px;
          color: var(--cg-text-muted);
          line-height: 1.45;
          display: -webkit-box;
          -webkit-line-clamp: 2;
          -webkit-box-orient: vertical;
          overflow: hidden;
        }
        .cg-box-foot {
          padding-top: 12px;
          border-top: 1px solid var(--cg-border);
          display: flex;
          align-items: center;
          justify-content: space-between;
        }
        .cg-btn-link {
          background: var(--cg-brand);
          color: #ffffff;
          padding: 7px 14px;
          border-radius: 8px;
          font-size: 12px;
          font-weight: 600;
          text-decoration: none;
          display: inline-flex;
          align-items: center;
          gap: 6px;
          transition: opacity 0.15s ease;
        }
        .cg-btn-link:hover { opacity: 0.9; }
        .cg-watermark { font-size: 11px; color: var(--cg-text-dim); text-decoration: none; }
      `;
    } else if (layout === 'strip') {
      // Yatay Dikdörtgen / Bar
      layoutSpecificStyles = `
        .cg-wrap-strip {
          background: var(--cg-bg);
          color: var(--cg-text);
          border: 1px solid var(--cg-border);
          border-radius: var(--cg-radius);
          padding: 14px 20px;
          box-shadow: var(--cg-card-shadow);
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 16px;
          width: 100%;
          height: var(--cg-h);
          flex-wrap: wrap;
        }
        .cg-strip-left {
          display: flex;
          align-items: center;
          gap: 12px;
          flex: 1;
          min-width: 240px;
        }
        .cg-strip-badge {
          background: var(--cg-surface);
          border: 1px solid var(--cg-border);
          padding: 6px 10px;
          border-radius: 8px;
          font-size: 11px;
          font-weight: 700;
          color: var(--cg-brand);
          white-space: nowrap;
          display: flex;
          align-items: center;
          gap: 6px;
        }
        .cg-strip-info { display: flex; flex-direction: column; gap: 2px; }
        .cg-strip-title {
          font-size: 14px;
          font-weight: 600;
          color: var(--cg-text);
          display: flex;
          align-items: center;
          gap: 8px;
          flex-wrap: wrap;
        }
        .cg-strip-desc {
          font-size: 12px;
          color: var(--cg-text-muted);
          white-space: nowrap;
          overflow: hidden;
          text-overflow: ellipsis;
          max-width: 560px;
        }
        .cg-strip-right {
          display: flex;
          align-items: center;
          gap: 12px;
          white-space: nowrap;
        }
        .cg-btn-strip {
          background: var(--cg-brand);
          color: #ffffff;
          padding: 8px 16px;
          border-radius: 8px;
          font-size: 12px;
          font-weight: 600;
          text-decoration: none;
          display: inline-flex;
          align-items: center;
          gap: 4px;
        }
        .cg-btn-strip:hover { opacity: 0.9; }
      `;
    } else if (layout === 'compact') {
      // Kompakt / Tek Commit Mini Kart
      layoutSpecificStyles = `
        .cg-wrap-compact {
          background: var(--cg-bg);
          color: var(--cg-text);
          border: 1px solid var(--cg-border);
          border-radius: var(--cg-radius);
          padding: 14px 16px;
          box-shadow: var(--cg-card-shadow);
          width: 100%;
          height: var(--cg-h);
          display: flex;
          flex-direction: column;
          gap: 8px;
        }
        .cg-compact-head {
          display: flex;
          align-items: center;
          justify-content: space-between;
          font-size: 11px;
        }
        .cg-compact-title {
          font-size: 13px;
          font-weight: 600;
          color: var(--cg-text);
          line-height: 1.4;
        }
        .cg-compact-desc {
          font-size: 11px;
          color: var(--cg-text-muted);
          line-height: 1.4;
          display: -webkit-box;
          -webkit-line-clamp: 2;
          -webkit-box-orient: vertical;
          overflow: hidden;
        }
        .cg-compact-foot {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding-top: 6px;
          border-top: 1px dashed var(--cg-border);
          font-size: 11px;
        }
        .cg-compact-link {
          color: var(--cg-brand);
          text-decoration: none;
          font-weight: 600;
        }
      `;
    } else {
      // Standart card / Dikey Liste Kartı
      layoutSpecificStyles = `
        .cg-wrap-card {
          background: var(--cg-bg);
          color: var(--cg-text);
          border: 1px solid var(--cg-border);
          border-radius: var(--cg-radius);
          padding: 18px;
          box-shadow: var(--cg-card-shadow);
          width: 100%;
          height: var(--cg-h);
        }
        .cg-card-head {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding-bottom: 12px;
          border-bottom: 1px solid var(--cg-border);
          margin-bottom: 12px;
        }
        .cg-card-heading { font-size: 15px; font-weight: 700; color: var(--cg-text); }
        .cg-card-list { display: flex; flex-direction: column; gap: 14px; }
        .cg-card-unit { padding-bottom: 12px; border-bottom: 1px solid var(--cg-border); }
        .cg-card-unit:last-child { padding-bottom: 0; border-bottom: none; }
        .cg-card-meta { display: flex; align-items: center; gap: 6px; margin-bottom: 4px; }
        .cg-card-date { font-size: 11px; color: var(--cg-text-dim); }
        .cg-card-title { font-size: 13px; font-weight: 600; color: var(--cg-text); margin-bottom: 4px; line-height: 1.4; }
        .cg-card-desc { font-size: 12px; color: var(--cg-text-muted); line-height: 1.45; }
        .cg-card-foot {
          margin-top: 14px;
          padding-top: 12px;
          border-top: 1px solid var(--cg-border);
          display: flex;
          align-items: center;
          justify-content: space-between;
          font-size: 11px;
        }
        .cg-card-foot a { color: var(--cg-brand); text-decoration: none; font-weight: 600; }
        .cg-card-foot a:hover { text-decoration: underline; }
        .cg-card-watermark { color: var(--cg-text-dim); text-decoration: none; }
      `;
    }

    style.textContent = `
      ${getCommonStyles(brandColor, theme, customWidth, customMaxWidth, customHeight, customRadius)}
      ${layoutSpecificStyles}
    `;
    shadow.appendChild(style);

    const entries = (data.entries || []).slice(0, limit);
    const wrapper = document.createElement('div');

    if (layout === 'box') {
      const itemsHtml = entries.map(e => `
        <div class="cg-entry-unit">
          <div class="cg-entry-meta">
            ${showPill ? `<span class="pill ${categoryClass(e.category)}">${categoryLabel(e.category)}</span>` : ''}
            ${showDate ? `<span class="cg-entry-date">${(e.published_at || '').substring(0, 10)}</span>` : ''}
          </div>
          <div class="cg-entry-title">${escapeHtml(e.title)}</div>
          ${showDesc && e.body ? `<div class="cg-entry-desc">${escapeHtml(e.body)}</div>` : ''}
        </div>
      `).join('');

      wrapper.className = 'cg-wrap-box';
      wrapper.innerHTML = `
        <div class="cg-box-head">
          <div class="cg-box-title">
            <span class="cg-pulse-dot"></span>
            <span>${escapeHtml(data.brand?.name || 'Yenilikler')}</span>
          </div>
          <span style="font-size: 11px; font-weight: 600; color: var(--cg-text-dim);">${limit === 1 ? 'Son Güncelleme' : 'Sürüm Notları'}</span>
        </div>
        <div class="cg-box-entries">
          ${itemsHtml}
        </div>
        ${showFooter ? `
        <div class="cg-box-foot">
          <a href="${data.changelog_url}" target="_blank" rel="noopener" class="cg-btn-link">
            Tümünü Gör &rarr;
          </a>
          <a href="${origin}" target="_blank" rel="noopener" class="cg-watermark">Commit Günlüğü</a>
        </div>` : ''}
      `;
    } else if (layout === 'strip') {
      const topEntry = entries[0] || { title: 'Yeni Güncelleme', category: 'NEW', body: '', published_at: '' };
      wrapper.className = 'cg-wrap-strip';
      wrapper.innerHTML = `
        <div class="cg-strip-left">
          <div class="cg-strip-badge">
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><path d="M13 2L3 14h9l-1 8 10-12h-9l1-8z"/></svg>
            <span>YENİLİK</span>
          </div>
          <div class="cg-strip-info">
            <div class="cg-strip-title">
              ${showPill ? `<span class="pill ${categoryClass(topEntry.category)}">${categoryLabel(topEntry.category)}</span>` : ''}
              <span>${escapeHtml(topEntry.title)}</span>
            </div>
            ${showDesc && topEntry.body ? `<div class="cg-strip-desc">${escapeHtml(topEntry.body)}</div>` : ''}
          </div>
        </div>
        <div class="cg-strip-right">
          ${showDate ? `<span style="font-size: 11px; color: var(--cg-text-dim);">${(topEntry.published_at || '').substring(0, 10)}</span>` : ''}
          <a href="${data.changelog_url}" target="_blank" rel="noopener" class="cg-btn-strip">
            İncele &rarr;
          </a>
        </div>
      `;
    } else if (layout === 'compact') {
      const topEntry = entries[0] || { title: 'Yeni Güncelleme', category: 'NEW', body: '', published_at: '' };
      wrapper.className = 'cg-wrap-compact';
      wrapper.innerHTML = `
        <div class="cg-compact-head">
          ${showPill ? `<span class="pill ${categoryClass(topEntry.category)}">${categoryLabel(topEntry.category)}</span>` : ''}
          ${showDate ? `<span style="color: var(--cg-text-dim);">${(topEntry.published_at || '').substring(0, 10)}</span>` : ''}
        </div>
        <div class="cg-compact-title">${escapeHtml(topEntry.title)}</div>
        ${showDesc && topEntry.body ? `<div class="cg-compact-desc">${escapeHtml(topEntry.body)}</div>` : ''}
        <div class="cg-compact-foot">
          <a href="${data.changelog_url}" target="_blank" rel="noopener" class="cg-compact-link">İncele &rarr;</a>
          <span style="color: var(--cg-text-dim); font-size: 10px;">${escapeHtml(data.brand?.name || 'Commit Günlüğü')}</span>
        </div>
      `;
    } else {
      // card
      const itemsHtml = entries.map(e => `
        <div class="cg-card-unit">
          <div class="cg-card-meta">
            ${showPill ? `<span class="pill ${categoryClass(e.category)}">${categoryLabel(e.category)}</span>` : ''}
            ${showDate ? `<span class="cg-card-date">${(e.published_at || '').substring(0, 10)}</span>` : ''}
          </div>
          <div class="cg-card-title">${escapeHtml(e.title)}</div>
          ${showDesc && e.body ? `<div class="cg-card-desc">${escapeHtml(e.body)}</div>` : ''}
        </div>
      `).join('');

      wrapper.className = 'cg-wrap-card';
      wrapper.innerHTML = `
        <div class="cg-card-head">
          <h4 class="cg-card-heading">${escapeHtml(data.brand?.name || 'Yenilikler')}</h4>
          <span style="font-size: 11px; color: var(--cg-brand); font-weight: 600;">${limit === 1 ? 'Son Sürüm' : 'Son Sürümler'}</span>
        </div>
        <div class="cg-card-list">
          ${itemsHtml}
        </div>
        ${showFooter ? `
        <div class="cg-card-foot">
          <a href="${data.changelog_url}" target="_blank" rel="noopener">Tüm Güncellemeler &rarr;</a>
          <a href="${origin}" target="_blank" rel="noopener" class="cg-card-watermark">Commit Günlüğü</a>
        </div>` : ''}
      `;
    }

    shadow.appendChild(wrapper);
  }

  // --- 3. BAŞLATICI / INITIALIZER ---
  async function initWidgets() {
    if (isInitialized) return;
    isInitialized = true;

    const scriptConfig = getScriptConfig();
    const inlineElements = Array.from(document.querySelectorAll('[data-cg-widget], [data-cg-key], .commit-gunlugu-widget, .commit-gunlugu-embed'));

    // 1) Sayfada AdSense tarzı yerleştirilmiş inline container'lar varsa doldur
    for (const el of inlineElements) {
      const key = el.getAttribute('data-key') || el.getAttribute('data-cg-key') || scriptConfig.key;
      if (!key) continue;

      const origin = el.getAttribute('data-origin') || scriptConfig.origin || DEFAULT_ORIGIN;
      const layout = el.getAttribute('data-layout') || scriptConfig.layout || 'card';
      const theme = el.getAttribute('data-theme') || scriptConfig.theme || 'auto';
      const brandColor = el.getAttribute('data-brand-color') || null;
      
      const width = el.getAttribute('data-width') || null;
      const height = el.getAttribute('data-height') || null;
      const maxWidth = el.getAttribute('data-max-width') || null;
      const radius = el.getAttribute('data-radius') || null;

      const showDesc = el.getAttribute('data-show-desc') !== 'false';
      const showDate = el.getAttribute('data-show-date') !== 'false';
      const showPill = el.getAttribute('data-show-pill') !== 'false';
      const showFooter = el.getAttribute('data-show-footer') !== 'false';

      const limitAttr = el.getAttribute('data-limit');
      const limit = limitAttr ? parseInt(limitAttr, 10) : undefined;

      try {
        const data = await fetchWidgetData(origin, key);
        if (data && data.entries) {
          renderInlineWidget(el, data, origin, {
            layout,
            theme,
            brandColor,
            limit,
            width,
            height,
            maxWidth,
            radius,
            showDesc,
            showDate,
            showPill,
            showFooter
          });
        }
      } catch (err) {
        console.error('[Commit Günlüğü] Inline widget yüklenemedi:', err);
      }
    }

    // 2) Eğer script'in kendisi data-mode="inline" ise
    if (scriptConfig.mode === 'inline' && inlineElements.length === 0 && scriptConfig.scriptElement && scriptConfig.key) {
      const host = document.createElement('div');
      scriptConfig.scriptElement.parentNode.insertBefore(host, scriptConfig.scriptElement);

      try {
        const data = await fetchWidgetData(scriptConfig.origin, scriptConfig.key);
        if (data && data.entries) {
          renderInlineWidget(host, data, scriptConfig.origin, {
            layout: scriptConfig.layout,
            theme: scriptConfig.theme,
            limit: scriptConfig.limit
          });
        }
      } catch (err) {
        console.error('[Commit Günlüğü] Script inline widget yüklenemedi:', err);
      }
      return;
    }

    // 3) Badge Rozet (Sağ alttaki buton)
    if (scriptConfig.key && scriptConfig.mode !== 'inline' && (!inlineElements.length || scriptConfig.mode === 'badge')) {
      try {
        const data = await fetchWidgetData(scriptConfig.origin, scriptConfig.key);
        if (data && data.entries && data.entries.length > 0) {
          renderBadgeWidget(data, scriptConfig.origin);
        }
      } catch (err) {
        console.error('[Commit Günlüğü] Badge widget yüklenemedi:', err);
      }
    }
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initWidgets);
  } else {
    initWidgets();
  }
})();

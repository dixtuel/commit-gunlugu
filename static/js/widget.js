/**
 * Commit Günlüğü — Gömülebilir Sürüm Günlüğü & Değişiklik Akışı Widget'ı
 * Bağımsız (Zero-dependency), AdBlocker dostu, tam özelleştirilebilir (Full Custom) ve Shadow DOM / Light DOM destekli.
 *
 * Parametreler (HTML data-* özellikleri):
 * - data-key: Tek proje widget anahtarı
 * - data-keys: Çoklu proje için virgülle ayrılmış anahtarlar (ör. "w_1, w_2, w_3")
 * - data-layout: 'card' (varsayılan) | 'box' / 'square' | 'strip' / 'banner' | 'compact' | 'timeline'
 * - data-mode: 'shadow' (varsayılan: izole Shadow DOM) | 'lightdom' (sitenin kendi CSS'i ile doğrudan render)
 * - data-limit: Gösterilecek commit/sürüm sayısı (varsayılan: 3)
 * - data-distinct: Çoklu projede her depodan en fazla 1 güncel kayıt alma (varsayılan: 'true')
 * - data-theme: 'auto' (varsayılan) | 'light' | 'dark' | 'custom'
 *
 * Özel Tema & Tasarım Parametreleri (data-theme="custom" veya doğrudan):
 * - data-bg: Arka plan rengi (ör. '#ffffff', '#F5F1E8', 'transparent')
 * - data-text: Metin rengi (ör. '#1e293b', '#2C2416')
 * - data-text-muted: İkincil metin/tarih rengi (ör. '#64748b', '#8B7355')
 * - data-border: Kenarlık rengi veya stili (ör. '#e2e8f0', '#C4BDB0', 'none')
 * - data-surface: Kart içi arka plan rengi (ör. '#f8fafc', 'transparent')
 * - data-accent / data-brand-color: Vurgu/rozet rengi (ör. '#10b981', '#D97642')
 * - data-font: Yazı tipi ailesi (ör. "'Source Sans 3', sans-serif")
 * - data-mono-font: Sabit aralıklı font ailesi (ör. "'Fira Code', monospace")
 * - data-radius: Köşe yuvarlaklığı (ör. '14px', '4px', '0px')
 * - data-shadow: Gölge stili (ör. 'none', '0 4px 16px rgba(0,0,0,0.06)')
 * - data-width: Özel genişlik (ör. '100%', '320px')
 * - data-max-width: Maksimum genişlik (ör. 'none', '440px')
 * - data-height: Özel yükseklik
 *
 * Light DOM Sınıf Özelleştirmeleri (data-mode="lightdom" için):
 * - data-class-entry: Kayıt kartı sınıfı (varsayılan: 'cg-entry')
 * - data-class-date: Tarih/meta sınıfı (varsayılan: 'cg-date')
 * - data-class-separator: Ayırıcı çizgi sınıfı (varsayılan: 'cg-separator')
 * - data-class-title: Başlık sınıfı (varsayılan: 'cg-title')
 * - data-class-body: Açıklama sınıfı (varsayılan: 'cg-body')
 * - data-class-link: Bağlantı sınıfı (varsayılan: 'cg-link')
 *
 * Görünüm Toggle'ları:
 * - data-show-desc: Açıklama metni görünsün mü? (varsayılan: true)
 * - data-show-date: Tarih görünsün mü? (varsayılan: true)
 * - data-show-pill: [YENİ]/[DÜZELTME] rozeti görünsün mü? (varsayılan: true)
 * - data-show-footer: Altbilgi çubuğu görünsün mü? (varsayılan: true)
 */
(function () {
  'use strict';

  const STORAGE_KEY = 'cg_widget_last_seen';
  const DEFAULT_ORIGIN = 'https://commit.dixtuel.tr';

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

  function categoryLabelEn(cat) {
    switch (cat) {
      case 'NEW': return 'NEW';
      case 'FIX': return 'FIX';
      default: return 'IMPROVEMENT';
    }
  }

  function formatTrDate(isoStr) {
    if (!isoStr) return '';
    const d = new Date(isoStr);
    if (isNaN(d.getTime())) return isoStr.substring(0, 10);
    const monthsTr = ['Oca', 'Şub', 'Mar', 'Nis', 'May', 'Haz', 'Tem', 'Ağu', 'Eyl', 'Eki', 'Kas', 'Ara'];
    const day = String(d.getDate()).padStart(2, '0');
    return `${day}.${monthsTr[d.getMonth()]}.${d.getFullYear()}`;
  }

  function formatEnDate(isoStr) {
    if (!isoStr) return '';
    const d = new Date(isoStr);
    if (isNaN(d.getTime())) return isoStr.substring(0, 10);
    const monthsEn = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];
    const day = String(d.getDate()).padStart(2, '0');
    return `${monthsEn[d.getMonth()]}.${day}.${d.getFullYear()}`;
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
      current = document.querySelector('script[data-key]') || document.querySelector('script[data-keys]') || document.querySelector('script[src*="widget.js"]');
    }

    let origin = DEFAULT_ORIGIN;
    let key = null;
    let keys = null;
    let mode = 'badge';
    let layout = 'card';
    let theme = 'auto';
    let limit = 5;

    if (current) {
      key = current.getAttribute('data-key');
      keys = current.getAttribute('data-keys');
      try {
        if (current.src) {
          const url = new URL(current.src, window.location.href);
          origin = url.origin;
          if (!key && !keys) {
            key = url.searchParams.get('key');
            keys = url.searchParams.get('keys');
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

    return { key, keys, origin, mode, layout, theme, limit, scriptElement: current };
  }

  // Tek proje verisini getir
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

  // Çoklu proje verisini harmanlayarak getir
  async function fetchMultiWidgetData(origin, keys, limit, distinct) {
    const targetOrigin = origin || DEFAULT_ORIGIN;
    const cacheKey = `${targetOrigin}:multi:${keys}:${limit}:${distinct}`;
    if (dataCache.has(cacheKey)) {
      return dataCache.get(cacheKey);
    }

    try {
      const url = `${targetOrigin}/api/v1/widget/multi?keys=${encodeURIComponent(keys)}&limit=${limit || 3}&distinct=${distinct !== false}`;
      const res = await fetch(url);
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

  // Ortak CSS Kuralları & CSS Değişkenleri
  function getCommonStyles(opts) {
    const brandColor = opts.accent || opts.brandColor || '#5b8a7a';
    const theme = opts.theme || 'auto';
    const customWidth = opts.width || '100%';
    const customMaxWidth = opts.maxWidth || 'none';
    const customHeight = opts.height || 'auto';
    const customRadius = opts.radius || '14px';
    const customFont = opts.font || '-apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif';
    const customMonoFont = opts.monoFont || 'ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace';

    const customBg = opts.bg || null;
    const customText = opts.text || null;
    const customTextMuted = opts.textMuted || null;
    const customBorder = opts.border || null;
    const customSurface = opts.surface || null;
    const customShadow = opts.shadow || null;

    const hasCustomOverride = customBg || customText || customBorder || customSurface || customShadow || theme === 'custom';

    return `
      * { box-sizing: border-box; margin: 0; padding: 0; font-family: var(--cg-font, ${customFont}); }
      
      :host {
        --cg-brand: var(--cg-custom-brand, ${brandColor});
        --cg-bg: var(--cg-custom-bg, ${customBg || (theme === 'dark' ? '#0f172a' : '#ffffff')});
        --cg-text: var(--cg-custom-text, ${customText || (theme === 'dark' ? '#f8fafc' : '#1e293b')});
        --cg-text-muted: var(--cg-custom-muted, ${customTextMuted || (theme === 'dark' ? '#94a3b8' : '#64748b')});
        --cg-text-dim: var(--cg-custom-dim, #94a3b8);
        --cg-border: var(--cg-custom-border, ${customBorder || (theme === 'dark' ? '#334155' : '#e2e8f0')});
        --cg-surface: var(--cg-custom-surface, ${customSurface || (theme === 'dark' ? '#1e293b' : '#f8fafc')});
        --cg-surface-hover: var(--cg-custom-surface-hover, ${theme === 'dark' ? '#293548' : '#f1f5f9'});
        --cg-card-shadow: var(--cg-custom-shadow, ${customShadow || (theme === 'dark' ? '0 4px 20px -2px rgba(0,0,0,0.4)' : '0 4px 16px -2px rgba(0,0,0,0.06), 0 2px 6px -1px rgba(0,0,0,0.04)')});
        --cg-w: ${customWidth};
        --cg-max-w: ${customMaxWidth};
        --cg-h: ${customHeight};
        --cg-radius: ${customRadius};
        --cg-font: ${customFont};
        --cg-mono-font: ${customMonoFont};
        display: block;
        width: var(--cg-w);
        max-width: var(--cg-max-w);
      }

      ${!hasCustomOverride && theme === 'auto' ? `
        @media (prefers-color-scheme: dark) {
          :host {
            --cg-bg: var(--cg-custom-bg, #0f172a);
            --cg-text: var(--cg-custom-text, #f8fafc);
            --cg-text-muted: var(--cg-custom-muted, #94a3b8);
            --cg-text-dim: var(--cg-custom-dim, #64748b);
            --cg-border: var(--cg-custom-border, #334155);
            --cg-surface: var(--cg-custom-surface, #1e293b);
            --cg-surface-hover: var(--cg-custom-surface-hover, #293548);
            --cg-card-shadow: 0 4px 20px -2px rgba(0,0,0,0.4);
          }
        }
      ` : ''}

      .cg-proj-tag {
        font-size: 10px; font-weight: 700; padding: 2px 6px; border-radius: 4px;
        letter-spacing: 0.02em; border: 1px solid currentColor; display: inline-block;
        white-space: nowrap; font-family: var(--cg-mono-font); opacity: 0.9;
      }

      .pill {
        font-size: 10px; font-weight: 700; padding: 2px 7px; border-radius: 4px;
        letter-spacing: 0.03em; text-transform: uppercase; display: inline-block;
      }
      .pill-new { background: rgba(16, 185, 129, 0.15); color: #059669; }
      .pill-fix { background: rgba(239, 68, 68, 0.15); color: #dc2626; }
      .pill-improvement { background: rgba(59, 130, 246, 0.15); color: #2563eb; }

      @media (prefers-color-scheme: dark) {
        ${!hasCustomOverride && theme !== 'light' ? `
          .pill-new { background: rgba(16, 185, 129, 0.25); color: #34d399; }
          .pill-fix { background: rgba(239, 68, 68, 0.25); color: #f87171; }
          .pill-improvement { background: rgba(59, 130, 246, 0.25); color: #60a5fa; }
        ` : ''}
      }
    `;
  }

  // --- 1. POPUP / BADGE WIDGET (Sağ alttaki buton) ---
  function renderBadgeWidget(data, origin, customOpts) {
    if (document.getElementById('commit-gunlugu-widget-root')) return;

    const host = document.createElement('div');
    host.id = 'commit-gunlugu-widget-root';
    document.body.appendChild(host);

    const shadow = host.attachShadow({ mode: 'open' });
    const lastSeen = Number(localStorage.getItem(STORAGE_KEY) || 0);
    const unreadCount = (data.entries || []).filter(e => new Date(e.published_at).getTime() > lastSeen).length;
    const brandColor = customOpts?.accent || data.brand?.color || '#5b8a7a';

    const style = document.createElement('style');
    style.textContent = `
      ${getCommonStyles({ ...customOpts, brandColor, theme: customOpts?.theme || 'auto', radius: '12px' })}
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
        align-items: center; justify-content: space-between; background: var(--cg-bg);
      }
      .cg-header h3 { font-size: 14px; font-weight: 700; margin: 0; color: var(--cg-text); }
      .cg-close { background: none; border: none; color: var(--cg-text-muted); cursor: pointer; font-size: 18px; line-height: 1; padding: 4px; }
      .cg-close:hover { color: var(--cg-text); }
      .cg-list { overflow-y: auto; padding: 8px 0; flex: 1; }
      .cg-item { padding: 12px 16px; border-bottom: 1px solid var(--cg-border); }
      .cg-item:last-child { border-bottom: none; }
      .cg-item-meta { display: flex; align-items: center; gap: 8px; margin-bottom: 6px; }
      .cg-date { font-size: 11px; color: var(--cg-text-dim); }
      .cg-title { font-size: 13px; font-weight: 600; color: var(--cg-text); margin-bottom: 4px; line-height: 1.4; }
      .cg-body { font-size: 12px; color: var(--cg-text-muted); line-height: 1.5; }
      .cg-footer {
        padding: 10px 16px; border-top: 1px solid var(--cg-border); font-size: 11px;
        display: flex; justify-content: space-between; align-items: center; background: var(--cg-surface);
      }
      .cg-footer a { color: var(--cg-brand); text-decoration: none; font-weight: 600; }
      .cg-footer a:hover { text-decoration: underline; }
    `;
    shadow.appendChild(style);

    const btn = document.createElement('button');
    btn.className = 'cg-badge-btn';
    btn.innerHTML = `
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9"></path>
        <path d="M13.73 21a2 2 0 0 1-3.46 0"></path>
      </svg>
      <span>Yenilikler</span>
      ${unreadCount > 0 ? `<span class="cg-unread-dot">${unreadCount}</span>` : ''}
    `;

    const panel = document.createElement('div');
    panel.className = 'cg-panel';

    const entries = data.entries || [];
    const entriesHtml = entries.length === 0
      ? '<div style="padding: 24px; text-align: center; color: var(--cg-text-muted); font-size: 12px;">Henüz yayınlanmış bir sürüm notu yok.</div>'
      : entries.map(e => `
        <div class="cg-item">
          <div class="cg-item-meta">
            ${e.project_name ? `<span class="cg-proj-tag" style="border-color: ${e.brand_color}; color: ${e.brand_color}">${escapeHtml(e.project_name)}</span>` : ''}
            <span class="pill ${categoryClass(e.category)}">${categoryLabel(e.category)}</span>
            <span class="cg-date">${(e.published_at || '').substring(0, 10)}</span>
          </div>
          <div class="cg-title">${escapeHtml(e.title)}</div>
          ${e.body ? `<div class="cg-body">${escapeHtml(e.body)}</div>` : ''}
        </div>
      `).join('');

    panel.innerHTML = `
      <div class="cg-header">
        <h3>${escapeHtml(data.brand?.name || 'Sürüm Notları')}</h3>
        <button class="cg-close" aria-label="Kapat">&times;</button>
      </div>
      <div class="cg-list">
        ${entriesHtml}
      </div>
      <div class="cg-footer">
        <a href="${data.changelog_url || origin}" target="_blank" rel="noopener">Tümünü İncele &rarr;</a>
        <a href="${origin}" target="_blank" rel="noopener" style="color: var(--cg-text-dim); font-weight: normal;">Commit Günlüğü</a>
      </div>
    `;

    shadow.appendChild(btn);
    shadow.appendChild(panel);

    let isOpen = false;
    function togglePanel() {
      isOpen = !isOpen;
      panel.style.display = isOpen ? 'flex' : 'none';
      if (isOpen) {
        localStorage.setItem(STORAGE_KEY, String(Date.now()));
        const dot = btn.querySelector('.cg-unread-dot');
        if (dot) dot.remove();
      }
    }

    btn.addEventListener('click', togglePanel);
    panel.querySelector('.cg-close').addEventListener('click', togglePanel);
  }

  // --- 2. LIGHT DOM (SİTENİN KENDİ CSS'İ İLE RENDER) ---
  function renderLightDomWidget(container, data, origin, opts) {
    const entries = (data.entries || []).slice(0, opts.limit || 3);
    const classEntry = opts.classEntry || 'cg-entry';
    const classDate = opts.classDate || 'cg-date';
    const classSeparator = opts.classSeparator || 'cg-separator';
    const classTitle = opts.classTitle || 'cg-title';
    const classBody = opts.classBody || 'cg-body';
    const classLink = opts.classLink || 'cg-link';
    const classPill = opts.classPill || 'cg-pill';
    const classProj = opts.classProj || 'cg-proj';

    const showDesc = opts.showDesc !== false;
    const showDate = opts.showDate !== false;
    const showPill = opts.showPill !== false;
    const showFooter = opts.showFooter !== false;

    const html = entries.map(e => {
      const dateTr = formatTrDate(e.published_at);
      const dateEn = formatEnDate(e.published_at);
      const projHtml = e.project_name ? `<span class="${classProj}" style="color: ${e.brand_color}; border-color: ${e.brand_color};">${escapeHtml(e.project_name)}</span>` : '';
      const pillHtml = showPill ? `<span class="${classPill}"><span data-lang="tr">${categoryLabel(e.category)}</span><span data-lang="en">${categoryLabelEn(e.category)}</span></span>` : '';
      const dateHtml = showDate ? `<span data-lang="tr">${dateTr}</span><span data-lang="en">${dateEn}</span>` : '';
      const descHtml = showDesc && e.body ? `<p class="${classBody}">${escapeHtml(e.body)}</p>` : '';
      const linkHtml = showFooter && e.changelog_url ? `<div class="${classLink}"><a href="${e.changelog_url}" target="_blank" rel="noopener"><span data-lang="tr">İncele &rarr;</span><span data-lang="en">View &rarr;</span></a></div>` : '';

      return `
        <div class="${classEntry}">
          <div class="${classDate}">
            ${dateHtml}
            ${projHtml}
            ${pillHtml}
          </div>
          <hr class="${classSeparator}">
          ${e.title ? `<div class="${classTitle}"><strong>${escapeHtml(e.title)}</strong></div>` : ''}
          ${descHtml}
          ${linkHtml}
        </div>
      `;
    }).join('');

    container.innerHTML = html;
  }

  // --- 3. SHADOW DOM INLINE WIDGET (TAM ÖZELLEŞTİRİLEBİLİR İZOLE KUTULAR) ---
  function renderInlineWidget(container, data, origin, opts) {
    if (opts.mode === 'lightdom' || opts.unstyled) {
      renderLightDomWidget(container, data, origin, opts);
      return;
    }

    if (container.shadowRoot) return;

    const shadow = container.attachShadow({ mode: 'open' });
    const rawLayout = (opts.layout || 'card').toLowerCase();
    
    let layout = 'card';
    if (rawLayout === 'square' || rawLayout === 'box') layout = 'box';
    else if (rawLayout === 'banner' || rawLayout === 'strip' || rawLayout === 'horizontal') layout = 'strip';
    else if (rawLayout === 'compact' || rawLayout === 'mini' || rawLayout === 'minimal') layout = 'compact';
    else if (rawLayout === 'timeline' || rawLayout === 'feed') layout = 'timeline';

    let defaultLimit = 3;
    if (layout === 'box') defaultLimit = 2;
    else if (layout === 'strip' || layout === 'compact') defaultLimit = 1;

    const limit = opts.limit !== undefined && opts.limit > 0 ? opts.limit : defaultLimit;

    const showDesc = opts.showDesc !== false;
    const showDate = opts.showDate !== false;
    const showPill = opts.showPill !== false;
    const showFooter = opts.showFooter !== false;

    const customWidth = opts.width || null;
    const customMaxWidth = opts.maxWidth || (layout === 'box' ? '360px' : layout === 'compact' ? '300px' : layout === 'strip' ? '100%' : '100%');
    const customHeight = opts.height || null;
    const customRadius = opts.radius || (layout === 'strip' ? '12px' : layout === 'box' ? '16px' : '14px');

    const style = document.createElement('style');
    let layoutSpecificStyles = '';

    if (layout === 'timeline') {
      layoutSpecificStyles = `
        .cg-wrap-timeline {
          background: var(--cg-bg);
          color: var(--cg-text);
          border: 1px solid var(--cg-border);
          border-radius: var(--cg-radius);
          padding: 24px;
          box-shadow: var(--cg-card-shadow);
          display: flex; flex-direction: column; gap: 24px;
          width: 100%; height: var(--cg-h);
        }
        .cg-tl-item {
          border-left: 3px solid var(--cg-brand);
          padding-left: 18px;
          display: flex; flex-direction: column; gap: 8px;
        }
        .cg-tl-meta {
          display: flex; align-items: center; gap: 10px;
          font-family: var(--cg-mono-font); font-size: 12px; color: var(--cg-text-muted);
        }
        .cg-tl-title { font-size: 15px; font-weight: 700; color: var(--cg-text); line-height: 1.4; }
        .cg-tl-body { font-size: 13.5px; color: var(--cg-text-muted); line-height: 1.6; }
        .cg-tl-link { font-size: 12px; font-weight: 600; color: var(--cg-brand); text-decoration: none; align-self: flex-start; }
        .cg-tl-link:hover { text-decoration: underline; }
      `;
    } else if (layout === 'box') {
      const minH = limit === 1 ? 'auto' : '280px';
      layoutSpecificStyles = `
        .cg-wrap-box {
          background: var(--cg-bg); color: var(--cg-text);
          border: 1px solid var(--cg-border); border-radius: var(--cg-radius);
          padding: 18px; box-shadow: var(--cg-card-shadow);
          display: flex; flex-direction: column; justify-content: space-between;
          width: 100%; min-height: ${minH}; height: var(--cg-h);
          transition: transform 0.2s ease, box-shadow 0.2s ease;
        }
        .cg-wrap-box:hover { transform: translateY(-2px); }
        .cg-box-head {
          display: flex; align-items: center; justify-content: space-between;
          padding-bottom: 12px; border-bottom: 1px solid var(--cg-border);
        }
        .cg-box-title { font-size: 14px; font-weight: 700; color: var(--cg-text); display: flex; align-items: center; gap: 8px; }
        .cg-pulse-dot { width: 8px; height: 8px; border-radius: 50%; background: var(--cg-brand); display: inline-block; }
        .cg-box-entries { flex: 1; display: flex; flex-direction: column; gap: 12px; padding: 12px 0; overflow: hidden; }
        .cg-entry-unit { display: flex; flex-direction: column; gap: 4px; }
        .cg-entry-meta { display: flex; align-items: center; gap: 6px; }
        .cg-entry-date { font-size: 11px; color: var(--cg-text-dim); }
        .cg-entry-title { font-size: 13px; font-weight: 600; color: var(--cg-text); line-height: 1.35; }
        .cg-entry-desc {
          font-size: 12px; color: var(--cg-text-muted); line-height: 1.45;
          display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden;
        }
        .cg-box-foot {
          padding-top: 12px; border-top: 1px solid var(--cg-border);
          display: flex; align-items: center; justify-content: space-between; font-size: 11px;
        }
        .cg-btn-link { color: #ffffff; background: var(--cg-brand); text-decoration: none; padding: 5px 12px; border-radius: 6px; font-weight: 600; }
        .cg-watermark { color: var(--cg-text-dim); text-decoration: none; }
      `;
    } else if (layout === 'strip') {
      layoutSpecificStyles = `
        .cg-wrap-strip {
          background: var(--cg-bg); color: var(--cg-text);
          border: 1px solid var(--cg-border); border-radius: var(--cg-radius);
          padding: 12px 20px; box-shadow: var(--cg-card-shadow);
          display: flex; align-items: center; justify-content: space-between; gap: 16px;
          width: 100%; height: var(--cg-h);
        }
        .cg-strip-left { display: flex; align-items: center; gap: 14px; flex: 1; min-width: 0; }
        .cg-strip-badge {
          background: var(--cg-brand); color: #fff; font-size: 11px; font-weight: 700;
          padding: 4px 10px; border-radius: 6px; display: inline-flex; align-items: center; gap: 5px; flex-shrink: 0;
        }
        .cg-strip-info { display: flex; flex-direction: column; min-width: 0; }
        .cg-strip-title { font-size: 13.5px; font-weight: 600; color: var(--cg-text); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; display: flex; align-items: center; gap: 8px; }
        .cg-strip-desc { font-size: 12px; color: var(--cg-text-muted); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
        .cg-strip-right { display: flex; align-items: center; gap: 12px; flex-shrink: 0; }
        .cg-btn-strip { background: var(--cg-surface); color: var(--cg-text); border: 1px solid var(--cg-border); text-decoration: none; padding: 6px 14px; border-radius: 6px; font-size: 12px; font-weight: 600; }
        .cg-btn-strip:hover { background: var(--cg-surface-hover); }
      `;
    } else if (layout === 'compact') {
      layoutSpecificStyles = `
        .cg-wrap-compact {
          background: var(--cg-bg); color: var(--cg-text);
          border: 1px solid var(--cg-border); border-radius: var(--cg-radius);
          padding: 14px; box-shadow: var(--cg-card-shadow);
          display: flex; flex-direction: column; gap: 8px;
          width: 100%; height: var(--cg-h);
        }
        .cg-compact-head { display: flex; align-items: center; justify-content: space-between; font-size: 11px; }
        .cg-compact-title { font-size: 13px; font-weight: 700; color: var(--cg-text); line-height: 1.35; }
        .cg-compact-desc { font-size: 12px; color: var(--cg-text-muted); line-height: 1.4; display: -webkit-box; -webkit-line-clamp: 2; -webkit-box-orient: vertical; overflow: hidden; }
        .cg-compact-foot { display: flex; align-items: center; justify-content: space-between; margin-top: 4px; padding-top: 8px; border-top: 1px solid var(--cg-border); }
        .cg-compact-link { font-size: 11px; font-weight: 600; color: var(--cg-brand); text-decoration: none; }
      `;
    } else {
      // card (varsayılan)
      layoutSpecificStyles = `
        .cg-wrap-card {
          background: var(--cg-bg); color: var(--cg-text);
          border: 1px solid var(--cg-border); border-radius: var(--cg-radius);
          padding: 20px; box-shadow: var(--cg-card-shadow);
          display: flex; flex-direction: column; justify-content: space-between;
          width: 100%; height: var(--cg-h);
        }
        .cg-card-head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 14px; padding-bottom: 10px; border-bottom: 1px solid var(--cg-border); }
        .cg-card-heading { font-size: 14px; font-weight: 700; margin: 0; color: var(--cg-text); }
        .cg-card-list { display: flex; flex-direction: column; gap: 14px; }
        .cg-card-unit { display: flex; flex-direction: column; gap: 4px; }
        .cg-card-meta { display: flex; align-items: center; gap: 8px; }
        .cg-card-date { font-size: 11px; color: var(--cg-text-dim); }
        .cg-card-title { font-size: 13px; font-weight: 600; color: var(--cg-text); line-height: 1.4; }
        .cg-card-desc { font-size: 12px; color: var(--cg-text-muted); line-height: 1.45; }
        .cg-card-foot {
          margin-top: 16px; padding-top: 12px; border-top: 1px solid var(--cg-border);
          display: flex; align-items: center; justify-content: space-between; font-size: 11px;
        }
        .cg-card-foot a { color: var(--cg-brand); text-decoration: none; font-weight: 600; }
        .cg-card-foot a:hover { text-decoration: underline; }
        .cg-card-watermark { color: var(--cg-text-dim) !important; font-weight: normal !important; }
      `;
    }

    style.textContent = `
      ${getCommonStyles({ ...opts, customWidth, customMaxWidth, customHeight, customRadius })}
      ${layoutSpecificStyles}
    `;
    shadow.appendChild(style);

    const entries = (data.entries || []).slice(0, limit);
    const wrapper = document.createElement('div');

    if (layout === 'timeline') {
      const itemsHtml = entries.map(e => `
        <div class="cg-tl-item">
          <div class="cg-tl-meta">
            ${showDate ? `<span>${(e.published_at || '').substring(0, 10)}</span>` : ''}
            ${e.project_name ? `<span class="cg-proj-tag" style="border-color: ${e.brand_color}; color: ${e.brand_color};">${escapeHtml(e.project_name)}</span>` : ''}
            ${showPill ? `<span class="pill ${categoryClass(e.category)}">${categoryLabel(e.category)}</span>` : ''}
          </div>
          <div class="cg-tl-title">${escapeHtml(e.title)}</div>
          ${showDesc && e.body ? `<div class="cg-tl-body">${escapeHtml(e.body)}</div>` : ''}
          ${showFooter && e.changelog_url ? `<a href="${e.changelog_url}" target="_blank" rel="noopener" class="cg-tl-link">İncele &rarr;</a>` : ''}
        </div>
      `).join('');

      wrapper.className = 'cg-wrap-timeline';
      wrapper.innerHTML = itemsHtml;
    } else if (layout === 'box') {
      const itemsHtml = entries.map(e => `
        <div class="cg-entry-unit">
          <div class="cg-entry-meta">
            ${e.project_name ? `<span class="cg-proj-tag" style="border-color: ${e.brand_color}; color: ${e.brand_color};">${escapeHtml(e.project_name)}</span>` : ''}
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
          <a href="${data.changelog_url || origin}" target="_blank" rel="noopener" class="cg-btn-link">
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
              ${topEntry.project_name ? `<span class="cg-proj-tag" style="border-color: ${topEntry.brand_color}; color: ${topEntry.brand_color};">${escapeHtml(topEntry.project_name)}</span>` : ''}
              ${showPill ? `<span class="pill ${categoryClass(topEntry.category)}">${categoryLabel(topEntry.category)}</span>` : ''}
              <span>${escapeHtml(topEntry.title)}</span>
            </div>
            ${showDesc && topEntry.body ? `<div class="cg-strip-desc">${escapeHtml(topEntry.body)}</div>` : ''}
          </div>
        </div>
        <div class="cg-strip-right">
          ${showDate ? `<span style="font-size: 11px; color: var(--cg-text-dim);">${(topEntry.published_at || '').substring(0, 10)}</span>` : ''}
          <a href="${topEntry.changelog_url || data.changelog_url || origin}" target="_blank" rel="noopener" class="cg-btn-strip">
            İncele &rarr;
          </a>
        </div>
      `;
    } else if (layout === 'compact') {
      const topEntry = entries[0] || { title: 'Yeni Güncelleme', category: 'NEW', body: '', published_at: '' };
      wrapper.className = 'cg-wrap-compact';
      wrapper.innerHTML = `
        <div class="cg-compact-head">
          <div style="display: flex; align-items: center; gap: 6px;">
            ${topEntry.project_name ? `<span class="cg-proj-tag" style="border-color: ${topEntry.brand_color}; color: ${topEntry.brand_color};">${escapeHtml(topEntry.project_name)}</span>` : ''}
            ${showPill ? `<span class="pill ${categoryClass(topEntry.category)}">${categoryLabel(topEntry.category)}</span>` : ''}
          </div>
          ${showDate ? `<span style="color: var(--cg-text-dim); font-size: 11px;">${(topEntry.published_at || '').substring(0, 10)}</span>` : ''}
        </div>
        <div class="cg-compact-title">${escapeHtml(topEntry.title)}</div>
        ${showDesc && topEntry.body ? `<div class="cg-compact-desc">${escapeHtml(topEntry.body)}</div>` : ''}
        <div class="cg-compact-foot">
          <a href="${topEntry.changelog_url || data.changelog_url || origin}" target="_blank" rel="noopener" class="cg-compact-link">İncele &rarr;</a>
          <span style="color: var(--cg-text-dim); font-size: 10px;">${escapeHtml(topEntry.project_name || data.brand?.name || 'Commit Günlüğü')}</span>
        </div>
      `;
    } else {
      // card (varsayılan)
      const itemsHtml = entries.map(e => `
        <div class="cg-card-unit">
          <div class="cg-card-meta">
            ${e.project_name ? `<span class="cg-proj-tag" style="border-color: ${e.brand_color}; color: ${e.brand_color};">${escapeHtml(e.project_name)}</span>` : ''}
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
          <a href="${data.changelog_url || origin}" target="_blank" rel="noopener">Tüm Güncellemeler &rarr;</a>
          <a href="${origin}" target="_blank" rel="noopener" class="cg-card-watermark">Commit Günlüğü</a>
        </div>` : ''}
      `;
    }

    shadow.appendChild(wrapper);
  }

  // --- 4. BAŞLATICI / INITIALIZER ---
  async function initWidgets() {
    const scriptConfig = getScriptConfig();
    const inlineElements = Array.from(document.querySelectorAll('[data-cg-widget], [data-cg-key], [data-cg-keys], .commit-gunlugu-widget, .commit-gunlugu-embed'));

    // 1) Sayfada AdSense tarzı yerleştirilmiş inline container'lar varsa doldur
    for (const el of inlineElements) {
      if (el.shadowRoot || el.getAttribute('data-cg-rendered') === 'true') continue;

      const rawKeys = el.getAttribute('data-keys') || el.getAttribute('data-key') || el.getAttribute('data-cg-key') || el.getAttribute('data-cg-keys') || scriptConfig.keys || scriptConfig.key;
      if (!rawKeys) continue;

      const origin = el.getAttribute('data-origin') || scriptConfig.origin || DEFAULT_ORIGIN;
      const layout = el.getAttribute('data-layout') || scriptConfig.layout || 'card';
      const mode = el.getAttribute('data-mode') || 'shadow';
      const theme = el.getAttribute('data-theme') || scriptConfig.theme || 'auto';

      // Özel renk & stil parametreleri
      const bg = el.getAttribute('data-bg') || null;
      const text = el.getAttribute('data-text') || null;
      const textMuted = el.getAttribute('data-text-muted') || null;
      const border = el.getAttribute('data-border') || null;
      const surface = el.getAttribute('data-surface') || null;
      const accent = el.getAttribute('data-accent') || el.getAttribute('data-brand-color') || null;
      const font = el.getAttribute('data-font') || null;
      const monoFont = el.getAttribute('data-mono-font') || null;
      const shadow = el.getAttribute('data-shadow') || null;

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
      const distinct = el.getAttribute('data-distinct') !== 'false';

      // LightDOM sınıf adları
      const classEntry = el.getAttribute('data-class-entry') || 'cg-entry';
      const classDate = el.getAttribute('data-class-date') || 'cg-date';
      const classSeparator = el.getAttribute('data-class-separator') || 'cg-separator';
      const classTitle = el.getAttribute('data-class-title') || 'cg-title';
      const classBody = el.getAttribute('data-class-body') || 'cg-body';
      const classLink = el.getAttribute('data-class-link') || 'cg-link';
      const classPill = el.getAttribute('data-class-pill') || 'cg-pill';
      const classProj = el.getAttribute('data-class-proj') || 'cg-proj';

      el.setAttribute('data-cg-rendered', 'true');

      try {
        let data;
        if (rawKeys.includes(',')) {
          // Çoklu proje modu
          data = await fetchMultiWidgetData(origin, rawKeys, limit || 3, distinct);
        } else {
          // Tek proje modu
          data = await fetchWidgetData(origin, rawKeys);
        }

        if (data && data.entries) {
          renderInlineWidget(el, data, origin, {
            layout,
            mode,
            theme,
            bg,
            text,
            textMuted,
            border,
            surface,
            accent,
            font,
            monoFont,
            shadow,
            limit,
            width,
            height,
            maxWidth,
            radius,
            showDesc,
            showDate,
            showPill,
            showFooter,
            classEntry,
            classDate,
            classSeparator,
            classTitle,
            classBody,
            classLink,
            classPill,
            classProj
          });
        }
      } catch (err) {
        console.error('[Commit Günlüğü] Inline widget yüklenemedi:', err);
      }
    }

    // 2) Script üzerinden tek başına badge modu
    if (scriptConfig.key && scriptConfig.mode === 'badge' && !inlineElements.length) {
      try {
        const data = await fetchWidgetData(scriptConfig.origin, scriptConfig.key);
        if (data && data.entries && data.entries.length > 0) {
          renderBadgeWidget(data, scriptConfig.origin, { theme: scriptConfig.theme });
        }
      } catch (err) {
        console.error('[Commit Günlüğü] Badge widget yüklenemedi:', err);
      }
    }
  }

  // Global API
  window.CommitGunlugu = {
    init: initWidgets,
    fetch: fetchWidgetData,
    fetchMulti: fetchMultiWidgetData
  };

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initWidgets);
  } else {
    initWidgets();
  }
})();

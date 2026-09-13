/**
 * identity-guard.js — Commit Günlüğü
 * Scraper/bot anti-harvesting protection for contact email & operator identity.
 * Kişisel verilerin kaynak kodundan taranmasını önleyen dinamik koruma katmanı.
 */
(function() {
  function decode(arr) {
    var str = '';
    for (var i = 0; i < arr.length; i++) {
      str += String.fromCharCode(arr[i]);
    }
    return str;
  }

  // Obfuscated character codes
  var USER_CHARS = [97, 115, 114, 105, 110, 107, 108, 99, 99]; // asrinklcc
  var DIXTUEL_DOMAIN = [100, 105, 120, 116, 117, 101, 108, 46, 116, 114]; // dixtuel.tr
  var SELY_DOMAIN = [115, 101, 108, 121, 46, 116, 114]; // sely.tr
  var OPERATOR_NAME_CHARS = [65, 115, 114, 305, 110, 32, 75, 305, 108, 305, 231, 32, 40, 100, 105, 120, 116, 117, 101, 108, 41]; // Asrın Kılıç (dixtuel)

  function renderProtectedContact(el) {
    var domainType = el.getAttribute('data-domain');
    var domain = (domainType === 'sely') ? decode(SELY_DOMAIN) : decode(DIXTUEL_DOMAIN);
    var user = decode(USER_CHARS);
    var email = user + '@' + domain;
    var locale = el.getAttribute('data-locale') || (document.documentElement.lang === 'en' ? 'en' : 'tr');
    var isEn = locale === 'en';

    var container = document.createElement('span');
    container.className = 'protected-contact-link';
    container.setAttribute('role', 'button');
    container.setAttribute('tabindex', '0');
    container.setAttribute('title', isEn ? 'Click to open mail client & copy address' : 'E-posta göndermek ve adresi kopyalamak için tıklayın');
    container.setAttribute('aria-label', email);

    container.innerHTML = 
      '<span class="p-u">' + user + '</span>' +
      '<span class="bot-decoy" style="display:none!important" aria-hidden="true">-anti-bot-harvest-</span>' +
      '<span class="p-at" aria-hidden="true">&#64;</span>' +
      '<span class="bot-decoy" style="display:none!important" aria-hidden="true">-do-not-scrape-</span>' +
      '<span class="p-d">' + domain + '</span>' +
      '<span class="p-badge" style="display:none;margin-left:6px;font-size:0.82em;padding:1px 6px;border-radius:4px;border:1px solid currentColor;opacity:0.85;" aria-live="polite"></span>';

    function handleAction(e) {
      if (e) e.preventDefault();
      if (navigator.clipboard && navigator.clipboard.writeText) {
        navigator.clipboard.writeText(email).then(function() {
          var badge = container.querySelector('.p-badge');
          if (badge) {
            badge.textContent = isEn ? 'copied!' : 'kopyalandı!';
            badge.style.display = 'inline-block';
            setTimeout(function() { badge.style.display = 'none'; }, 2200);
          }
        }).catch(function() {});
      }
      window.location.href = 'mailto:' + email;
    }

    container.addEventListener('click', handleAction);
    container.addEventListener('keydown', function(e) {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        handleAction(e);
      }
    });

    el.parentNode.replaceChild(container, el);
  }

  function renderProtectedName(el) {
    var name = decode(OPERATOR_NAME_CHARS);
    var span = document.createElement('span');
    span.className = 'protected-name-val';
    var parts = name.split(' ');
    // "Asrın Kılıç (dixtuel)" -> "Asrın", "Kılıç", "(dixtuel)"
    span.innerHTML = 
      '<span>' + (parts[0] || '') + '</span> ' +
      '<span class="bot-decoy" style="display:none!important" aria-hidden="true">-fake-entity-</span>' +
      '<span>' + (parts[1] || '') + '</span> ' +
      '<span>' + (parts.slice(2).join(' ') || '') + '</span>';
    el.parentNode.replaceChild(span, el);
  }

  function init() {
    var contacts = document.querySelectorAll('.protected-contact');
    for (var i = 0; i < contacts.length; i++) {
      renderProtectedContact(contacts[i]);
    }
    var names = document.querySelectorAll('.protected-name');
    for (var j = 0; j < names.length; j++) {
      renderProtectedName(names[j]);
    }
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();

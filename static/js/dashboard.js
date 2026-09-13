// Commit Günlüğü Dashboard JavaScript

function toggleModal(id) {
  const modal = document.getElementById(id);
  if (!modal) return;
  modal.style.display = modal.style.display === "none" || !modal.style.display ? "flex" : "none";
}

async function updateStatus(entryId, action) {
  try {
    const res = await fetch(`/api/v1/entries/${entryId}/${action}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" }
    });

    if (!res.ok) {
      alert("İşlem başarısız oldu.");
      return;
    }

    const card = document.getElementById(`entry-${entryId}`);
    if (card) {
      if (action === "publish") {
        card.classList.remove("entry-status-draft", "entry-status-dismissed");
        card.classList.add("entry-status-published");
        const pill = card.querySelector(".status-pill");
        if (pill) {
          pill.className = "status-pill status-published";
          pill.textContent = "PUBLISHED";
        }
        const actions = card.querySelector(".entry-actions");
        if (actions) {
          actions.innerHTML = `
            <span class="published-label">Yayında</span>
            <button class="btn-action btn-dismiss" onclick="updateStatus('${entryId}', 'dismiss')">Kaldır</button>
          `;
        }
      } else if (action === "dismiss") {
        card.classList.remove("entry-status-draft", "entry-status-published");
        card.classList.add("entry-status-dismissed");
        const pill = card.querySelector(".status-pill");
        if (pill) {
          pill.className = "status-pill status-dismissed";
          pill.textContent = "DISMISSED";
        }
        const actions = card.querySelector(".entry-actions");
        if (actions) {
          actions.innerHTML = `
            <span class="dismissed-label">Yoksayıldı</span>
            <button class="btn-action btn-publish" onclick="updateStatus('${entryId}', 'publish')">Yeniden Yayınla</button>
          `;
        }
      }
    }
  } catch (err) {
    console.error(err);
    alert("Bir bağlantı hatası oluştu.");
  }
}

async function submitNewProject(e) {
  e.preventDefault();
  const repo = document.getElementById("repo_name").value.trim();
  const name = document.getElementById("proj_name").value.trim();
  const color = document.getElementById("brand_color").value;
  const parseMode = document.getElementById("parse_mode") ? document.getElementById("parse_mode").value : "ai_editorial";
  const audience = document.getElementById("audience") ? document.getElementById("audience").value : "end_user";

  try {
    const res = await fetch("/api/v1/projects", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        github_repo_full_name: repo,
        name: name,
        brand_color: color,
        parse_mode: parseMode,
        audience: audience
      })
    });

    if (res.ok) {
      window.location.reload();
    } else {
      const data = await res.json();
      alert("Hata: " + (data.message || "Proje kaydedilemedi."));
    }
  } catch (err) {
    alert("Sunucuya ulaşılamadı.");
  }
}

function copyWidgetSnippet(key) {
  const origin = window.location.origin;
  const snippet = `<script src="${origin}/static/js/widget.js" data-key="${key}" async></script>`;
  navigator.clipboard.writeText(snippet).then(() => {
    alert("Widget kodu panoya kopyalandı!\n\n" + snippet);
  });
}

document.addEventListener("DOMContentLoaded", () => {
  document.querySelectorAll(".copy-btn").forEach(btn => {
    btn.addEventListener("click", () => {
      const key = btn.getAttribute("data-key");
      if (key) copyWidgetSnippet(key);
    });
  });
});

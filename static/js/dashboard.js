// Commit Günlüğü — Dashboard JavaScript Controller
// Zero external libraries • Direct REST API integration • Toast feedback

let currentEditingProjectId = null;
let currentDeletingProjectId = null;

function showToast(message, type = "success") {
  let container = document.getElementById("toast-container");
  if (!container) {
    container = document.createElement("div");
    container.id = "toast-container";
    container.className = "toast-container";
    document.body.appendChild(container);
  }

  const toast = document.createElement("div");
  toast.className = `toast-message toast-${type}`;
  toast.textContent = message;

  container.appendChild(toast);

  setTimeout(() => {
    toast.classList.add("toast-show");
  }, 10);

  setTimeout(() => {
    toast.classList.remove("toast-show");
    setTimeout(() => toast.remove(), 200);
  }, 3200);
}

function toggleModal(id) {
  const modal = document.getElementById(id);
  if (!modal) return;
  const isHidden = modal.style.display === "none" || !modal.style.display;
  modal.style.display = isHidden ? "flex" : "none";
}

function filterEntries(filter) {
  document.querySelectorAll(".filter-btn").forEach(btn => {
    btn.classList.toggle("active", btn.getAttribute("data-filter") === filter);
  });

  document.querySelectorAll(".entry-item").forEach(item => {
    if (filter === "all") {
      item.style.display = "block";
    } else {
      item.style.display = item.classList.contains(`entry-status-${filter}`) ? "block" : "none";
    }
  });
}

async function updateStatus(entryId, action) {
  try {
    const res = await fetch(`/api/v1/entries/${entryId}/${action}`, {
      method: "POST",
      headers: { "Content-Type": "application/json" }
    });

    if (!res.ok) {
      showToast("İşlem gerçekleştirilemedi.", "error");
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
          pill.textContent = "Yayında";
        }
        const actions = card.querySelector(".entry-actions");
        if (actions) {
          actions.innerHTML = `
            <span class="published-label">Yayında</span>
            <button class="btn-action btn-dismiss" onclick="updateStatus('${entryId}', 'dismiss')">Kaldır</button>
            <button class="btn-action btn-delete" onclick="deleteEntry('${entryId}')">Sil</button>
          `;
        }
        showToast("Sürüm notu yayına alındı.");
      } else if (action === "dismiss") {
        card.classList.remove("entry-status-draft", "entry-status-published");
        card.classList.add("entry-status-dismissed");
        const pill = card.querySelector(".status-pill");
        if (pill) {
          pill.className = "status-pill status-dismissed";
          pill.textContent = "Yoksayıldı";
        }
        const actions = card.querySelector(".entry-actions");
        if (actions) {
          actions.innerHTML = `
            <span class="dismissed-label">Yoksayıldı</span>
            <button class="btn-action btn-publish" onclick="updateStatus('${entryId}', 'publish')">Yayınla</button>
            <button class="btn-action btn-delete" onclick="deleteEntry('${entryId}')">Sil</button>
          `;
        }
        showToast("Sürüm notu yayından kaldırıldı.");
      }
    }
  } catch (err) {
    console.error(err);
    showToast("Sunucu bağlantı hatası.", "error");
  }
}

async function deleteEntry(entryId) {
  if (!confirm("Bu sürüm notu kaydını silmek istediğinize emin misiniz?")) {
    return;
  }

  try {
    const res = await fetch(`/api/v1/entries/${entryId}/delete`, {
      method: "POST",
      headers: { "Content-Type": "application/json" }
    });

    if (!res.ok) {
      showToast("Kayıt silinemedi.", "error");
      return;
    }

    const card = document.getElementById(`entry-${entryId}`);
    if (card) {
      card.remove();
    }
    showToast("Sürüm notu silindi.");
  } catch (err) {
    console.error(err);
    showToast("Bağlantı hatası.", "error");
  }
}

function togglePrivateTokenField(mode) {
  if (mode === "create") {
    const isPrivate = document.getElementById("is_private") ? document.getElementById("is_private").checked : false;
    const label = document.getElementById("create_token_label");
    const hint = document.getElementById("create_token_hint");
    const input = document.getElementById("custom_github_token");
    if (label) {
      label.textContent = isPrivate ? "Kişisel GitHub Token (Zorunlu)" : "Kişisel GitHub Token (İsteğe Bağlı)";
    }
    if (hint) {
      hint.innerHTML = isPrivate
        ? "Gizli deponuza erişebilmek için <code>repo</code> iznine sahip PAT girilmesi zorunludur."
        : "Açık (public) depolarda boş bırakırsanız sunucu kotası kullanılır. Kendi token'ınızı kullanmak isterseniz girebilirsiniz.";
    }
    if (input) input.required = isPrivate;
  } else if (mode === "edit") {
    const isPrivate = document.getElementById("edit_is_private") ? document.getElementById("edit_is_private").checked : false;
    const label = document.getElementById("edit_token_label");
    const hint = document.getElementById("edit_token_hint");
    if (label) {
      label.textContent = isPrivate ? "Kişisel GitHub Token (Private için Zorunlu)" : "Kişisel GitHub Token (İsteğe Bağlı)";
    }
    if (hint) {
      hint.innerHTML = isPrivate
        ? "Gizli depolar için kayıtlı geçerli bir token bulunmalıdır. Değiştirmek istemiyorsanız boş bırakın."
        : "Açık depolarda isteğe bağlıdır; boş bırakırsanız platform kotası kullanılır.";
    }
  }
}

async function submitNewProject(e) {
  e.preventDefault();
  const repo = document.getElementById("repo_name").value.trim();
  const name = document.getElementById("proj_name").value.trim();
  const color = document.getElementById("brand_color").value;
  const parseMode = document.getElementById("parse_mode") ? document.getElementById("parse_mode").value : "ai_editorial";
  const audience = document.getElementById("audience") ? document.getElementById("audience").value : "end_user";
  const isPrivate = document.getElementById("is_private") ? document.getElementById("is_private").checked : false;
  const customTokenInput = document.getElementById("custom_github_token");
  const customToken = customTokenInput ? customTokenInput.value.trim() : "";

  if (isPrivate && !customToken) {
    showToast("Gizli (private) depolar için kişisel GitHub Token girmeniz zorunludur.", "error");
    return;
  }

  try {
    const res = await fetch("/api/v1/projects", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        github_repo_full_name: repo,
        name: name,
        brand_color: color,
        parse_mode: parseMode,
        audience: audience,
        is_private: isPrivate,
        custom_github_token: customToken || null
      })
    });

    if (res.ok) {
      window.location.reload();
    } else {
      const data = await res.json();
      showToast(data.message || "Proje kaydedilemedi.", "error");
    }
  } catch (err) {
    showToast("Sunucuya ulaşılamadı.", "error");
  }
}

function openEditProjectModal(id, name, color, mode, audience, isPrivate) {
  currentEditingProjectId = id;
  const nameInput = document.getElementById("edit_proj_name");
  const colorInput = document.getElementById("edit_brand_color");
  const modeSelect = document.getElementById("edit_parse_mode");
  const audienceSelect = document.getElementById("edit_audience");
  const isPrivateCheckbox = document.getElementById("edit_is_private");
  const tokenInput = document.getElementById("edit_custom_github_token");

  if (nameInput) nameInput.value = name;
  if (colorInput) colorInput.value = color || "#2563eb";
  if (modeSelect) modeSelect.value = mode || "ai_editorial";
  if (audienceSelect) audienceSelect.value = audience || "end_user";
  if (isPrivateCheckbox) isPrivateCheckbox.checked = (isPrivate === 1);
  if (tokenInput) tokenInput.value = "";

  togglePrivateTokenField("edit");
  toggleModal("edit-project-modal");
}

async function submitEditProject(e) {
  e.preventDefault();
  if (!currentEditingProjectId) return;

  const name = document.getElementById("edit_proj_name").value.trim();
  const color = document.getElementById("edit_brand_color").value;
  const parseMode = document.getElementById("edit_parse_mode").value;
  const audience = document.getElementById("edit_audience").value;
  const isPrivate = document.getElementById("edit_is_private") ? document.getElementById("edit_is_private").checked : false;
  const tokenInput = document.getElementById("edit_custom_github_token");
  const customToken = tokenInput ? tokenInput.value.trim() : "";

  try {
    const payload = {
      name: name,
      brand_color: color,
      parse_mode: parseMode,
      audience: audience,
      template_style: "standard",
      is_private: isPrivate
    };
    if (customToken) {
      payload.custom_github_token = customToken;
    }

    const res = await fetch(`/api/v1/projects/${currentEditingProjectId}/settings`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(payload)
    });

    if (res.ok) {
      window.location.reload();
    } else {
      const data = await res.json();
      showToast(data.message || "Ayarlar güncellenemedi.", "error");
    }
  } catch (err) {
    showToast("Sunucu bağlantı hatası.", "error");
  }
}

function openDeleteProjectModal(id, name) {
  currentDeletingProjectId = id;
  const label = document.getElementById("delete_project_name_label");
  if (label) label.textContent = name;
  toggleModal("delete-project-modal");
}

async function confirmDeleteProject() {
  if (!currentDeletingProjectId) return;

  try {
    const res = await fetch(`/api/v1/projects/${currentDeletingProjectId}/delete`, {
      method: "POST",
      headers: { "Content-Type": "application/json" }
    });

    if (res.ok) {
      window.location.reload();
    } else {
      const data = await res.json();
      showToast(data.message || "Proje silinemedi.", "error");
    }
  } catch (err) {
    showToast("Sunucuya ulaşılamadı.", "error");
  }
}

async function submitManualEntry(e) {
  e.preventDefault();
  const projectId = document.getElementById("entry_project_id").value;
  const category = document.getElementById("entry_category").value;
  const title = document.getElementById("entry_title").value.trim();
  const body = document.getElementById("entry_body").value.trim();
  const status = document.getElementById("entry_status").value;

  if (!projectId) {
    showToast("Lütfen bir proje seçin.", "error");
    return;
  }

  try {
    const res = await fetch(`/api/v1/projects/${projectId}/entries`, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        category: category,
        title: title,
        body: body,
        status: status
      })
    });

    if (res.ok) {
      window.location.reload();
    } else {
      const data = await res.json();
      showToast(data.message || "Kayıt eklenemedi.", "error");
    }
  } catch (err) {
    showToast("Sunucuya ulaşılamadı.", "error");
  }
}

function copyWidgetSnippet(key) {
  const origin = window.location.origin;
  const snippet = `<script src="${origin}/static/js/widget.js" data-key="${key}" async></script>`;
  navigator.clipboard.writeText(snippet).then(() => {
    showToast("Widget entegrasyon kodu panoya kopyalandı!");
  }).catch(() => {
    showToast("Kopyalama başarısız oldu.", "error");
  });
}

function copyText(text, label = "Metin") {
  navigator.clipboard.writeText(text).then(() => {
    showToast(`${label} panoya kopyalandı.`);
  }).catch(() => {
    showToast("Kopyalama başarısız oldu.", "error");
  });
}

async function syncProjectGithub(projectId, btn) {
  if (!btn) return;
  const originalText = btn.textContent;
  btn.disabled = true;
  btn.textContent = "Çekiliyor...";
  try {
    const res = await fetch(`/api/v1/projects/${projectId}/sync-github`, {
      method: "POST",
      headers: { "Content-Type": "application/json" }
    });
    const data = await res.json();
    if (res.ok) {
      showToast(data.message || "Commit'ler başarıyla çekildi.");
      setTimeout(() => window.location.reload(), 800);
    } else {
      showToast(data.message || "GitHub'dan çekme başarısız oldu.", "error");
    }
  } catch (err) {
    showToast("Sunucuya ulaşılamadı.", "error");
  } finally {
    btn.disabled = false;
    btn.textContent = originalText;
  }
}

-- Commit Günlüğü SQLite Schema (WAL Mode)

CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    name TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_users_email ON users(email);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_sessions_user ON sessions(user_id);
CREATE INDEX IF NOT EXISTS idx_sessions_expires ON sessions(expires_at);

CREATE TABLE IF NOT EXISTS password_resets (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash TEXT NOT NULL UNIQUE,
    expires_at TEXT NOT NULL,
    used_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_password_resets_token ON password_resets(token_hash);

-- KVKK İmha Ledger'ı (Tombstone Ledger):
-- Silinen kullanıcıların özet kayıtları burada tutulur ve R2'ye bağımsız senkronize edilir.
-- Eski bir yedekten geri yükleme yapılsa bile bu listedeki kullanıcılar dirilmeden otomatik imha edilir.
CREATE TABLE IF NOT EXISTS erasure_ledger (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    email_hash TEXT NOT NULL UNIQUE,
    purged_at TEXT NOT NULL DEFAULT (datetime('now')),
    reason TEXT NOT NULL DEFAULT 'kvkk_user_request'
);

CREATE INDEX IF NOT EXISTS idx_erasure_email_hash ON erasure_ledger(email_hash);

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
    github_repo_full_name TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    widget_key TEXT NOT NULL UNIQUE,
    brand_name TEXT,
    brand_color TEXT NOT NULL DEFAULT '#10b981',
    brand_logo_url TEXT,
    webhook_secret TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_projects_slug ON projects(slug);
CREATE INDEX IF NOT EXISTS idx_projects_widget_key ON projects(widget_key);
CREATE INDEX IF NOT EXISTS idx_projects_repo ON projects(github_repo_full_name);
CREATE INDEX IF NOT EXISTS idx_projects_user ON projects(user_id);

CREATE TABLE IF NOT EXISTS entries (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    category TEXT NOT NULL CHECK(category IN ('NEW', 'FIX', 'IMPROVEMENT')),
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'DRAFT' CHECK(status IN ('DRAFT', 'PUBLISHED', 'DISMISSED')),
    ai_generated INTEGER NOT NULL DEFAULT 1,
    source_commit_shas TEXT NOT NULL DEFAULT '[]',
    source_pr_number INTEGER,
    author_username TEXT,
    published_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_entries_project_status ON entries(project_id, status);
CREATE INDEX IF NOT EXISTS idx_entries_published_at ON entries(published_at);

CREATE TABLE IF NOT EXISTS webhook_events (
    id TEXT PRIMARY KEY,
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    github_delivery_id TEXT NOT NULL UNIQUE,
    event_type TEXT NOT NULL,
    payload_summary TEXT NOT NULL,
    processed_at TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_webhook_delivery ON webhook_events(github_delivery_id);

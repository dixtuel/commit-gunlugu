-- Security hardening: Searchable blind index for encrypted emails
ALTER TABLE users ADD COLUMN email_hash TEXT;
CREATE INDEX IF NOT EXISTS idx_users_email_hash ON users(email_hash);

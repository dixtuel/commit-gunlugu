-- 20260913000003_private_repos.sql
-- Add is_private flag and custom_github_token column for private repository support

ALTER TABLE projects ADD COLUMN is_private INTEGER NOT NULL DEFAULT 0;
ALTER TABLE projects ADD COLUMN custom_github_token TEXT;

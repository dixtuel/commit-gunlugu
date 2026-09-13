-- 20260913000002_project_modes.sql
-- Add parse_mode, audience, and template_style to projects table

ALTER TABLE projects ADD COLUMN parse_mode TEXT NOT NULL DEFAULT 'ai_editorial';
ALTER TABLE projects ADD COLUMN audience TEXT NOT NULL DEFAULT 'end_user';
ALTER TABLE projects ADD COLUMN template_style TEXT NOT NULL DEFAULT 'standard';

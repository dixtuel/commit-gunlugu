-- 20260918000001_project_language.sql
-- Add language column to projects table (e.g. 'auto', 'tr', 'en')

ALTER TABLE projects ADD COLUMN language TEXT NOT NULL DEFAULT 'auto';

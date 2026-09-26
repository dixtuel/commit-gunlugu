CREATE TABLE IF NOT EXISTS entry_branches (
    entry_id TEXT NOT NULL REFERENCES entries(id) ON DELETE CASCADE,
    project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    branch_name TEXT NOT NULL,
    PRIMARY KEY (entry_id, branch_name)
);

CREATE INDEX IF NOT EXISTS idx_entry_branches_project_branch
    ON entry_branches(project_id, branch_name, entry_id);

-- Eski tek branch ayarının bilinen geçmişini entry'lere aktar.
INSERT OR IGNORE INTO entry_branches (entry_id, project_id, branch_name)
SELECT e.id, e.project_id, p.tracked_branch
FROM entries e
JOIN projects p ON p.id = e.project_id
WHERE p.tracked_branch <> '';

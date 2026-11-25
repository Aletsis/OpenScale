-- OpenScale Schema v2 (0002_branches.sql)
-- Adds Branches (sucursales) with per-branch PLU catalog selection.

CREATE TABLE IF NOT EXISTS branches (
    id           TEXT PRIMARY KEY NOT NULL,
    store_id     TEXT NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    code         TEXT NOT NULL,
    name         TEXT NOT NULL,
    address      TEXT,
    notes        TEXT,
    catalog_mode TEXT NOT NULL DEFAULT 'all',
    sync_version INTEGER NOT NULL DEFAULT 1,
    updated_at   TEXT NOT NULL,
    deleted_at   TEXT,
    UNIQUE(store_id, code)
);

-- Seed default local branch
INSERT OR IGNORE INTO branches (
    id, store_id, code, name, address, notes, catalog_mode, sync_version, updated_at, deleted_at
) VALUES (
    '01900000-0000-7000-8000-000000000010',
    '01900000-0000-7000-8000-000000000001',
    'MAIN',
    'Principal',
    NULL,
    NULL,
    'all',
    1,
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    NULL
);

CREATE TABLE IF NOT EXISTS branch_plus (
    branch_id  TEXT NOT NULL REFERENCES branches(id) ON DELETE CASCADE,
    did        INTEGER NOT NULL,
    plu_number INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (branch_id, did, plu_number)
);
CREATE INDEX IF NOT EXISTS idx_branch_plus_branch ON branch_plus(branch_id);

ALTER TABLE scales ADD COLUMN branch_id TEXT REFERENCES branches(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_scales_branch ON scales(branch_id);

-- Assign existing scales to the default local branch if unset
UPDATE scales
SET branch_id = '01900000-0000-7000-8000-000000000010'
WHERE branch_id IS NULL AND store_id = '01900000-0000-7000-8000-000000000001';

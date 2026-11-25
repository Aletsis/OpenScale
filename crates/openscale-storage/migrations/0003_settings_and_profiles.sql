-- OpenScale Schema v3 (0003_settings_and_profiles.sql)
-- Adds application settings and global/individual scale configuration profiles.

CREATE TABLE IF NOT EXISTS app_settings (
    key        TEXT PRIMARY KEY NOT NULL,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS scale_config_profiles (
    id                  TEXT PRIMARY KEY NOT NULL,
    store_id            TEXT NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    scope               TEXT NOT NULL,
    target_id           TEXT NOT NULL,
    family              TEXT NOT NULL,
    machine_params_json TEXT,
    banks_748_json      TEXT,
    overrides_json      TEXT NOT NULL DEFAULT '[]',
    updated_at          TEXT NOT NULL,
    UNIQUE(store_id, scope, target_id, family)
);
CREATE INDEX IF NOT EXISTS idx_scale_config_profiles_lookup
    ON scale_config_profiles(store_id, scope, target_id, family);

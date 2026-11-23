-- OpenScale Initial Schema (0001_init.sql)
-- SQLite (WAL) today, designed with store_id + UUIDv7 + sync_version for PostgreSQL multi-store evolution.

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS stores (
    id          TEXT PRIMARY KEY NOT NULL,
    code        TEXT NOT NULL UNIQUE,
    name        TEXT NOT NULL,
    timezone    TEXT NOT NULL DEFAULT 'UTC',
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- Seed default local store (deterministic UUIDv7-like nil-safe local ID)
INSERT OR IGNORE INTO stores (id, code, name, timezone, created_at, updated_at)
VALUES (
    '01900000-0000-7000-8000-000000000001',
    'LOCAL',
    'Local Store',
    'UTC',
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
    strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
);

CREATE TABLE IF NOT EXISTS scale_groups (
    id           TEXT PRIMARY KEY NOT NULL,
    store_id     TEXT NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    description  TEXT,
    sync_version INTEGER NOT NULL DEFAULT 1,
    updated_at   TEXT NOT NULL,
    deleted_at   TEXT,
    UNIQUE(store_id, name)
);

CREATE TABLE IF NOT EXISTS scales (
    id               TEXT PRIMARY KEY NOT NULL,
    store_id         TEXT NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    group_id         TEXT REFERENCES scale_groups(id) ON DELETE SET NULL,
    name             TEXT NOT NULL,
    family           TEXT NOT NULL,
    ip_address       TEXT NOT NULL,
    data_port        INTEGER NOT NULL,
    service_port     INTEGER,
    serial_number    TEXT,
    model            TEXT,
    firmware_version TEXT,
    mac_address      TEXT,
    status           TEXT NOT NULL DEFAULT 'unknown',
    last_seen_at     TEXT,
    sync_version     INTEGER NOT NULL DEFAULT 1,
    updated_at       TEXT NOT NULL,
    deleted_at       TEXT
);
CREATE INDEX IF NOT EXISTS idx_scales_store_ip ON scales(store_id, ip_address);

CREATE TABLE IF NOT EXISTS departments (
    id           TEXT PRIMARY KEY NOT NULL,
    store_id     TEXT NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    did          INTEGER NOT NULL,
    name         TEXT NOT NULL,
    sync_version INTEGER NOT NULL DEFAULT 1,
    updated_at   TEXT NOT NULL,
    deleted_at   TEXT,
    UNIQUE(store_id, did)
);

CREATE TABLE IF NOT EXISTS plus (
    id                   TEXT PRIMARY KEY NOT NULL,
    store_id             TEXT NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    did                  INTEGER NOT NULL,
    plu_number           INTEGER NOT NULL,
    description          TEXT NOT NULL,
    button_text          TEXT,
    price_1_cents        INTEGER NOT NULL DEFAULT 0,
    price_2_cents        INTEGER NOT NULL DEFAULT 0,
    price_3_cents        INTEGER NOT NULL DEFAULT 0,
    product_type         TEXT NOT NULL DEFAULT 'by_weight',
    ean                  TEXT,
    sku                  TEXT,
    group_id             INTEGER NOT NULL DEFAULT 0,
    barcode_format_id    INTEGER NOT NULL DEFAULT 0,
    label_format_id      INTEGER NOT NULL DEFAULT 0,
    tare_id              INTEGER,
    tare_grams           INTEGER,
    date_offset_1        INTEGER NOT NULL DEFAULT 0,
    date_print_control_1 INTEGER NOT NULL DEFAULT 0,
    date_offset_2        INTEGER NOT NULL DEFAULT 0,
    date_print_control_2 INTEGER NOT NULL DEFAULT 0,
    tax_rate_id_1        INTEGER NOT NULL DEFAULT 0,
    tax_rate_id_2        INTEGER NOT NULL DEFAULT 0,
    text_areas_json      TEXT NOT NULL DEFAULT '[]',
    delta_hash           TEXT NOT NULL,
    sync_version         INTEGER NOT NULL DEFAULT 1,
    updated_at           TEXT NOT NULL,
    deleted_at           TEXT,
    UNIQUE(store_id, did, plu_number)
);
CREATE INDEX IF NOT EXISTS idx_plus_store_did_plu ON plus(store_id, did, plu_number);
CREATE INDEX IF NOT EXISTS idx_plus_store_ean ON plus(store_id, ean);

CREATE TABLE IF NOT EXISTS tares (
    id           TEXT PRIMARY KEY NOT NULL,
    store_id     TEXT NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    cid          INTEGER NOT NULL,
    name         TEXT NOT NULL,
    weight_grams INTEGER NOT NULL DEFAULT 0,
    sync_version INTEGER NOT NULL DEFAULT 1,
    updated_at   TEXT NOT NULL,
    deleted_at   TEXT,
    UNIQUE(store_id, cid)
);

CREATE TABLE IF NOT EXISTS tax_rates (
    id                TEXT PRIMARY KEY NOT NULL,
    store_id          TEXT NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    tax_id            INTEGER NOT NULL,
    name              TEXT NOT NULL,
    rate_basis_points INTEGER NOT NULL DEFAULT 0,
    sync_version      INTEGER NOT NULL DEFAULT 1,
    updated_at        TEXT NOT NULL,
    deleted_at        TEXT,
    UNIQUE(store_id, tax_id)
);

CREATE TABLE IF NOT EXISTS stored_texts (
    id           TEXT PRIMARY KEY NOT NULL,
    store_id     TEXT NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    text_id      INTEGER NOT NULL,
    title        TEXT NOT NULL,
    content      TEXT NOT NULL,
    sync_version INTEGER NOT NULL DEFAULT 1,
    updated_at   TEXT NOT NULL,
    deleted_at   TEXT,
    UNIQUE(store_id, text_id)
);

CREATE TABLE IF NOT EXISTS media_assets (
    id           TEXT PRIMARY KEY NOT NULL,
    store_id     TEXT NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    media_id     INTEGER NOT NULL,
    name         TEXT NOT NULL,
    mime_type    TEXT NOT NULL,
    data         BLOB NOT NULL,
    sync_version INTEGER NOT NULL DEFAULT 1,
    updated_at   TEXT NOT NULL,
    deleted_at   TEXT,
    UNIQUE(store_id, media_id)
);

CREATE TABLE IF NOT EXISTS barcode_formats (
    id              TEXT PRIMARY KEY NOT NULL,
    store_id        TEXT NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    bid             INTEGER NOT NULL,
    name            TEXT NOT NULL,
    definition_json TEXT NOT NULL,
    sync_version    INTEGER NOT NULL DEFAULT 1,
    updated_at      TEXT NOT NULL,
    deleted_at      TEXT,
    UNIQUE(store_id, bid)
);

CREATE TABLE IF NOT EXISTS label_formats (
    id               TEXT PRIMARY KEY NOT NULL,
    store_id         TEXT NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    pfid             INTEGER NOT NULL,
    name             TEXT NOT NULL,
    width_tenths_mm  INTEGER NOT NULL,
    height_tenths_mm INTEGER NOT NULL,
    definition_json  TEXT NOT NULL,
    raw_binary       BLOB,
    sync_version     INTEGER NOT NULL DEFAULT 1,
    updated_at       TEXT NOT NULL,
    deleted_at       TEXT,
    UNIQUE(store_id, pfid)
);

CREATE TABLE IF NOT EXISTS sales_snapshots (
    id           TEXT PRIMARY KEY NOT NULL,
    store_id     TEXT NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    scale_id     TEXT NOT NULL REFERENCES scales(id) ON DELETE CASCADE,
    period       INTEGER NOT NULL DEFAULT 0,
    captured_at  TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    sync_version INTEGER NOT NULL DEFAULT 1,
    updated_at   TEXT NOT NULL,
    deleted_at   TEXT
);
CREATE INDEX IF NOT EXISTS idx_sales_snapshots_scale_time ON sales_snapshots(scale_id, captured_at);

CREATE TABLE IF NOT EXISTS sync_jobs (
    id              TEXT PRIMARY KEY NOT NULL,
    store_id        TEXT NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    mode            TEXT NOT NULL,
    status          TEXT NOT NULL,
    total_items     INTEGER NOT NULL DEFAULT 0,
    processed_items INTEGER NOT NULL DEFAULT 0,
    failed_items    INTEGER NOT NULL DEFAULT 0,
    started_at      TEXT,
    finished_at     TEXT,
    created_at      TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sync_job_items (
    id            TEXT PRIMARY KEY NOT NULL,
    job_id        TEXT NOT NULL REFERENCES sync_jobs(id) ON DELETE CASCADE,
    scale_id      TEXT NOT NULL REFERENCES scales(id) ON DELETE CASCADE,
    entity_type   TEXT NOT NULL,
    entity_key    TEXT NOT NULL,
    action        TEXT NOT NULL,
    status        TEXT NOT NULL,
    error_message TEXT,
    updated_at    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_sync_job_items_job ON sync_job_items(job_id);

CREATE TABLE IF NOT EXISTS scale_plu_checksums (
    scale_id     TEXT NOT NULL REFERENCES scales(id) ON DELETE CASCADE,
    did          INTEGER NOT NULL,
    plu_number   INTEGER NOT NULL,
    delta_hash   TEXT NOT NULL,
    synced_at    TEXT NOT NULL,
    PRIMARY KEY (scale_id, did, plu_number)
);

CREATE TABLE IF NOT EXISTS users (
    id            TEXT PRIMARY KEY NOT NULL,
    store_id      TEXT NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    username      TEXT NOT NULL,
    display_name  TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    role          TEXT NOT NULL,
    is_active     INTEGER NOT NULL DEFAULT 1,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    UNIQUE(store_id, username)
);

CREATE TABLE IF NOT EXISTS api_keys (
    id           TEXT PRIMARY KEY NOT NULL,
    store_id     TEXT NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    key_prefix   TEXT NOT NULL,
    key_hash     TEXT NOT NULL UNIQUE,
    role         TEXT NOT NULL,
    scopes_json  TEXT NOT NULL DEFAULT '[]',
    expires_at   TEXT,
    last_used_at TEXT,
    created_at   TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS audit_logs (
    id           TEXT PRIMARY KEY NOT NULL,
    store_id     TEXT NOT NULL REFERENCES stores(id) ON DELETE CASCADE,
    actor_id     TEXT,
    actor_name   TEXT NOT NULL,
    action       TEXT NOT NULL,
    target_type  TEXT NOT NULL,
    target_id    TEXT,
    details_json TEXT,
    created_at   TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_audit_logs_store_time ON audit_logs(store_id, created_at);

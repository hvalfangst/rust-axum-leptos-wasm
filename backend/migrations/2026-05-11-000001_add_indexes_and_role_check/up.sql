-- Indexes on foreign-key columns. Postgres does NOT create these automatically
-- for FKs, so joins and FK lookup scans were sequential before.
CREATE INDEX IF NOT EXISTS empires_location_id_idx ON empires (location_id);
CREATE INDEX IF NOT EXISTS ships_empire_id_idx     ON ships    (empire_id);
CREATE INDEX IF NOT EXISTS players_user_id_idx     ON players  (user_id);
CREATE INDEX IF NOT EXISTS players_location_id_idx ON players  (location_id);
CREATE INDEX IF NOT EXISTS players_active_ship_idx ON players  (active_ship_id);

-- Constrain users.role to the four supported values at the DB level so a stray
-- INSERT bypassing the application layer can't write "ADMIN " or "root".
ALTER TABLE users
    ADD CONSTRAINT users_role_check
    CHECK (role IN ('READER', 'WRITER', 'EDITOR', 'ADMIN'));

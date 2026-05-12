ALTER TABLE users DROP CONSTRAINT IF EXISTS users_role_check;

DROP INDEX IF EXISTS players_active_ship_idx;
DROP INDEX IF EXISTS players_location_id_idx;
DROP INDEX IF EXISTS players_user_id_idx;
DROP INDEX IF EXISTS ships_empire_id_idx;
DROP INDEX IF EXISTS empires_location_id_idx;

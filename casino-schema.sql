-- Casino tables for ForestBot-RS
-- Run once on forestbot_hub database to enable casino commands.

CREATE TABLE IF NOT EXISTS casino_balance (
    player_name  VARCHAR(64) PRIMARY KEY,
    chips        INT      NOT NULL DEFAULT 1000,
    streak       INT      NOT NULL DEFAULT 0,
    last_claim   DATETIME,
    last_scratch DATETIME,
    updated_at   DATETIME NOT NULL DEFAULT NOW()
);

-- Single-row jackpot state (INSERT IGNORE auto-creates on first use)
CREATE TABLE IF NOT EXISTS casino_jackpot (
    id        INT PRIMARY KEY DEFAULT 1,
    pot       INT      NOT NULL DEFAULT 0,
    last_draw DATETIME NULL DEFAULT NULL
);

CREATE TABLE IF NOT EXISTS casino_jackpot_tickets (
    player_name  VARCHAR(64) NOT NULL,
    ticket_count INT         NOT NULL DEFAULT 0,
    PRIMARY KEY (player_name)
);

-- Lotto ticket purchases; draw_date = the Saturday this ticket enters
CREATE TABLE IF NOT EXISTS casino_lotto_tickets (
    id          INT AUTO_INCREMENT PRIMARY KEY,
    player_name VARCHAR(64) NOT NULL,
    numbers     VARCHAR(32) NOT NULL,
    draw_date   DATE        NOT NULL,
    created_at  DATETIME    NOT NULL DEFAULT NOW(),
    INDEX idx_draw_date (draw_date),
    INDEX idx_player (player_name)
);

-- Record of completed draws
CREATE TABLE IF NOT EXISTS casino_lotto_draws (
    id        INT AUTO_INCREMENT PRIMARY KEY,
    draw_date DATE        NOT NULL,
    numbers   VARCHAR(32) NOT NULL,
    draw_time DATETIME    NOT NULL DEFAULT NOW(),
    INDEX idx_draw_date (draw_date)
);

-- Single-row lotto jackpot pot (grows with ticket sales, resets on jackpot win)
CREATE TABLE IF NOT EXISTS casino_lotto_pot (
    id  INT PRIMARY KEY DEFAULT 1,
    pot BIGINT NOT NULL DEFAULT 1000
);

-- Pending casino draw notifications (delivered on player join, deleted on claim)
CREATE TABLE IF NOT EXISTS casino_notifications (
    id          INT AUTO_INCREMENT PRIMARY KEY,
    player_name VARCHAR(64)  NOT NULL,
    message     TEXT         NOT NULL,
    created_at  DATETIME     NOT NULL DEFAULT NOW(),
    INDEX idx_player (player_name)
);

-- ─────────────────────────────────────────────────────────────────────────────
-- Migration for existing installs (run if you applied the schema before this
-- version):
--
-- ALTER TABLE casino_jackpot
--     CHANGE COLUMN next_draw last_draw DATETIME NULL DEFAULT NULL;
--
-- ALTER TABLE casino_lotto_tickets
--     DROP INDEX IF EXISTS idx_draw,
--     DROP COLUMN draw_id,
--     ADD COLUMN draw_date DATE NOT NULL DEFAULT (CURDATE()),
--     ADD COLUMN created_at DATETIME NOT NULL DEFAULT NOW(),
--     ADD INDEX idx_draw_date (draw_date);
--
-- ALTER TABLE casino_lotto_draws
--     ADD COLUMN draw_date DATE NOT NULL DEFAULT (CURDATE()),
--     ADD INDEX idx_draw_date (draw_date);
-- ─────────────────────────────────────────────────────────────────────────────

CREATE TABLE users (
    id         INTEGER PRIMARY KEY,
    name       TEXT NOT NULL,
    email      TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE events (
    id      INTEGER PRIMARY KEY,
    user_id INTEGER REFERENCES users(id),
    kind    TEXT NOT NULL,
    payload TEXT,
    at      TEXT DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_events_user ON events(user_id);
INSERT INTO users (name, email) VALUES
    ('Ada Lovelace',   'ada@example.com'),
    ('Linus Torvalds', 'linus@example.com'),
    ('Grace Hopper',   'grace@example.com');
INSERT INTO events (user_id, kind, payload) VALUES
    (1, 'login',  '{}'),
    (1, 'query',  '{}'),
    (2, 'login',  '{}'),
    (3, 'signup', '{}');

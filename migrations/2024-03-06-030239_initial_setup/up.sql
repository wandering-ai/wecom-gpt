-- 初始化应用状态表
CREATE TABLE db_init_status (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    initialized_at TIMESTAMP NOT NULL
);
-- 初始化用户表
CREATE TABLE guests (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    name VARCHAR(255) UNIQUE NOT NULL,
    credit DOUBLE NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL,
    admin BOOLEAN NOT NULL DEFAULT FALSE
);
-- 初始化会话表
CREATE TABLE conversations (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    guest_id INTEGER NOT NULL REFERENCES guests(id),
    assistant_id INTEGER NOT NULL,
    active BOOLEAN NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);
-- 初始化消息表
CREATE TABLE messages (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL REFERENCES conversations(id),
    created_at DATETIME NOT NULL,
    content TEXT NOT NULL,
    cost DOUBLE NOT NULL,
    message_type INTEGER NOT NULL,
    content_type INTEGER NOT NULL,
    prompt_tokens INTEGER NOT NULL,
    completion_tokens INTEGER NOT NULL
);
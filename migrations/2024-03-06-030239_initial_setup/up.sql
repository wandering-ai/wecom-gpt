-- 初始化用户表
CREATE TABLE guests (
    id INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
    name VARCHAR(255) UNIQUE NOT NULL,
    credit DOUBLE NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);
-- 初始化AI供应商表
CREATE TABLE providers (
    id INTEGER NOT NULL PRIMARY KEY,
    name VARCHAR NOT NULL
);
-- 初始化企业微信应用表
CREATE TABLE assistants (
    id INTEGER NOT NULL PRIMARY KEY,
    name VARCHAR NOT NULL,
    agent_id INTEGER NOT NULL,
    provider_id INTEGEREGER REFERENCES providers(id)
);
-- 初始化会话表
CREATE TABLE conversations (
    id INTEGER NOT NULL PRIMARY KEY,
    guest_id INTEGER NOT NULL REFERENCES guests(id),
    assistant_id INTEGER NOT NULL REFERENCES assistants(id),
    active BOOLEAN NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);
-- 初始化消息类型表
CREATE TABLE msg_types (
    id INTEGER NOT NULL PRIMARY KEY,
    name VARCHAR UNIQUE NOT NULL
);
-- 初始化消息内容类型表
CREATE TABLE content_types (
    id INTEGER NOT NULL PRIMARY KEY,
    name VARCHAR UNIQUE NOT NULL
);
-- 初始化消息表
CREATE TABLE messages (
    id BIGINT NOT NULL PRIMARY KEY,
    conversation_id INTEGER NOT NULL REFERENCES conversations(id),
    created_at DATETIME NOT NULL,
    content TEXT NOT NULL,
    cost DOUBLE NOT NULL,
    message_type INTEGER NOT NULL REFERENCES msg_types(id),
    content_type INTEGER NOT NULL REFERENCES content_types(id)
);
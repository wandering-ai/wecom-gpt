-- 初始化用户表
CREATE TABLE guests (
    id INT NOT NULL PRIMARY KEY,
    name VARCHAR(255) UNIQUE NOT NULL,
    credit DOUBLE NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);
-- 初始化AI供应商表
CREATE TABLE providers (
    id INT NOT NULL PRIMARY KEY,
    name VARCHAR NOT NULL
);
-- 初始化企业微信应用表
CREATE TABLE assistants (
    id INT NOT NULL PRIMARY KEY,
    name VARCHAR NOT NULL,
    agent_id INT NOT NULL,
    provider_id INTEGER REFERENCES providers(id)
);
-- 初始化会话表
CREATE TABLE conversations (
    id INT NOT NULL PRIMARY KEY,
    guest_id INT NOT NULL REFERENCES guests(id),
    assistant_id INT NOT NULL REFERENCES assistants(id),
    active BOOLEAN NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);
-- 初始化消息类型表
CREATE TABLE msg_types (
    id INT NOT NULL PRIMARY KEY,
    name VARCHAR UNIQUE NOT NULL
);
-- 初始化消息内容类型表
CREATE TABLE content_types (
    id INT NOT NULL PRIMARY KEY,
    name VARCHAR UNIQUE NOT NULL
);
-- 初始化消息表
CREATE TABLE messages (
    id BIGINT NOT NULL PRIMARY KEY,
    conversation_id INT NOT NULL REFERENCES conversations(id),
    created_at DATETIME NOT NULL,
    content TEXT NOT NULL,
    cost DOUBLE NOT NULL,
    message_type INT NOT NULL REFERENCES msg_types(id),
    content_type INT NOT NULL REFERENCES content_types(id)
);
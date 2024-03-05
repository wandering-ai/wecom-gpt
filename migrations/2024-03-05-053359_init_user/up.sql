-- Your SQL goes here
-- 初始化用户表
CREATE TABLE guests (
    id INT NOT NULL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    credit DOUBLE NOT NULL DEFAULT 0,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
);
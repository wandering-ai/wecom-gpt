-- This file should undo anything in `up.sql`
-- 移除消息表
DROP TABLE messages;
-- 移除消息类型表
DROP TABLE msg_types;
-- 移除消息内容类型表
DROP TABLE content_types;
-- 移除会话表
DROP TABLE conversations;
-- 移除企业微信应用表
DROP TABLE assistants;
-- 移除AI供应商表
DROP TABLE providers;
-- 移除用户表
DROP TABLE guests;
-- 移除数据库初始化状态表
DROP TABLE db_init_status;
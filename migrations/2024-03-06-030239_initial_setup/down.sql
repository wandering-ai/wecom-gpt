-- This file should undo anything in `up.sql`
-- 移除消息表
DROP TABLE messages;
-- 移除会话表
DROP TABLE conversations;
-- 移除用户表
DROP TABLE guests;
-- 移除数据库初始化状态表
DROP TABLE db_init_status;
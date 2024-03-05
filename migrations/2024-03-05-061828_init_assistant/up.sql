-- 初始化企业微信应用表
CREATE TABLE assistants (
    id INT NOT NULL PRIMARY KEY,
    name VARCHAR NOT NULL,
    agent_id INT NOT NULL,
    provider_id INTEGER REFERENCES providers(id)
)
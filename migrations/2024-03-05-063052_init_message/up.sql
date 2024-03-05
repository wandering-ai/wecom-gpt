CREATE TABLE messages (
    id INT NOT NULL PRIMARY KEY,
    conversation_id INT NOT NULL REFERENCES conversations(id),
    created_at DATETIME NOT NULL,
    content TEXT NOT NULL,
    cost DOUBLE NOT NULL,
    message_type INT NOT NULL REFERENCES msg_types(id),
    content_type INT NOT NULL REFERENCES content_types(id)
)
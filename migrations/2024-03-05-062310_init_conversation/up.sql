-- Your SQL goes here
CREATE TABLE conversations (
    id INT NOT NULL PRIMARY KEY,
    guest_id INT NOT NULL REFERENCES guests(id),
    assistant_id INT NOT NULL REFERENCES assistants(id),
    active BOOLEAN NOT NULL,
    created_at TIMESTAMP NOT NULL,
    updated_at TIMESTAMP NOT NULL
)
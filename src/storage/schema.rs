// @generated automatically by Diesel CLI.

diesel::table! {
    conversations (id) {
        id -> Integer,
        guest_id -> Integer,
        assistant_id -> Integer,
        active -> Bool,
        created_at -> Timestamp,
        updated_at -> Timestamp,
    }
}

diesel::table! {
    db_init_status (id) {
        id -> Integer,
        initialized_at -> Timestamp,
    }
}

diesel::table! {
    guests (id) {
        id -> Integer,
        name -> Text,
        credit -> Double,
        created_at -> Timestamp,
        updated_at -> Timestamp,
        admin -> Bool,
    }
}

diesel::table! {
    messages (id) {
        id -> Integer,
        conversation_id -> Integer,
        created_at -> Timestamp,
        content -> Text,
        cost -> Double,
        message_type -> Integer,
        content_type -> Integer,
        prompt_tokens -> Integer,
        completion_tokens -> Integer,
    }
}

diesel::joinable!(conversations -> guests (guest_id));
diesel::joinable!(messages -> conversations (conversation_id));

diesel::allow_tables_to_appear_in_same_query!(
    conversations,
    db_init_status,
    guests,
    messages,
);

// @generated automatically by Diesel CLI.

diesel::table! {
    assistants (id) {
        id -> Integer,
        name -> Text,
        agent_id -> Integer,
        provider_id -> Integer,
    }
}

diesel::table! {
    content_types (id) {
        id -> Integer,
        name -> Text,
    }
}

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
        meta -> Text,
    }
}

diesel::table! {
    msg_types (id) {
        id -> Integer,
        name -> Text,
    }
}

diesel::table! {
    providers (id) {
        id -> Integer,
        name -> Text,
        endpoint -> Text,
        max_tokens -> Integer,
        prompt_token_price -> Double,
        completion_token_price -> Double,
    }
}

diesel::joinable!(assistants -> providers (provider_id));
diesel::joinable!(conversations -> assistants (assistant_id));
diesel::joinable!(conversations -> guests (guest_id));
diesel::joinable!(messages -> content_types (content_type));
diesel::joinable!(messages -> conversations (conversation_id));
diesel::joinable!(messages -> msg_types (message_type));

diesel::allow_tables_to_appear_in_same_query!(
    assistants,
    content_types,
    conversations,
    db_init_status,
    guests,
    messages,
    msg_types,
    providers,
);

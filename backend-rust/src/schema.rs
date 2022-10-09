// @generated automatically by Diesel CLI.

diesel::table! {
    use diesel::sql_types::*;
    use diesel_full_text_search::*;

    courses_unfinished (tucan_id) {
        tucan_id -> Bytea,
        tucan_last_checked -> Timestamptz,
        title -> Text,
        course_id -> Text,
        sws -> Int2,
        content -> Text,
        done -> Bool,
        tsv -> Tsvector,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use diesel_full_text_search::*;

    module_courses (module, course) {
        module -> Bytea,
        course -> Bytea,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use diesel_full_text_search::*;

    module_menu_module (module_id, module_menu_id) {
        module_menu_id -> Bytea,
        module_id -> Bytea,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use diesel_full_text_search::*;

    module_menu_unfinished (tucan_id) {
        tucan_id -> Bytea,
        tucan_last_checked -> Timestamptz,
        name -> Text,
        child_type -> Int2,
        parent -> Nullable<Bytea>,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use diesel_full_text_search::*;

    modules_unfinished (tucan_id) {
        tucan_id -> Bytea,
        tucan_last_checked -> Timestamptz,
        title -> Text,
        module_id -> Text,
        credits -> Nullable<Int4>,
        content -> Text,
        done -> Bool,
        tsv -> Tsvector,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use diesel_full_text_search::*;

    sessions (tu_id, session_nr, session_id) {
        tu_id -> Text,
        session_nr -> Int8,
        session_id -> Text,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use diesel_full_text_search::*;

    users_studies (user_id, study) {
        user_id -> Text,
        study -> Bytea,
    }
}

diesel::table! {
    use diesel::sql_types::*;
    use diesel_full_text_search::*;

    users_unfinished (tu_id) {
        tu_id -> Text,
        title -> Text,
        academic_title -> Text,
        post_name -> Text,
        first_name -> Text,
        middle_name -> Text,
        last_name -> Text,
        pre_name -> Text,
        redirect_messages_to_university_email -> Bool,
        subject -> Text,
        email -> Text,
        department -> Int4,
        post_title -> Text,
        street -> Text,
        address_addition -> Text,
        country -> Text,
        plz -> Int4,
        city -> Text,
        phone_number -> Text,
        done -> Bool,
    }
}

diesel::joinable!(module_courses -> courses_unfinished (course));
diesel::joinable!(module_courses -> modules_unfinished (module));
diesel::joinable!(module_menu_module -> module_menu_unfinished (module_menu_id));
diesel::joinable!(module_menu_module -> modules_unfinished (module_id));
diesel::joinable!(sessions -> users_unfinished (tu_id));
diesel::joinable!(users_studies -> module_menu_unfinished (study));
diesel::joinable!(users_studies -> users_unfinished (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    courses_unfinished,
    module_courses,
    module_menu_module,
    module_menu_unfinished,
    modules_unfinished,
    sessions,
    users_studies,
    users_unfinished,
);

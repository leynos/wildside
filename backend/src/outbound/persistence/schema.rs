//! Diesel table definitions for the PostgreSQL schema.
//!
//! These definitions are maintained manually to keep the persistence adapter
//! compile-safe while migrations evolve. Native geometric columns are
//! represented with Diesel-compatible placeholder types (`Text`) because the
//! current adapter layer does not query them via Diesel yet.

// -----------------------------------------------------------------------------
// Existing application tables
// -----------------------------------------------------------------------------

diesel::table! {
    idempotency_keys (key, user_id, mutation_type) {
        key -> Uuid,
        user_id -> Uuid,
        mutation_type -> Text,
        payload_hash -> Bytea,
        response_snapshot -> Jsonb,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    users (id) {
        id -> Uuid,
        display_name -> Varchar,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    routes (id) {
        id -> Uuid,
        user_id -> Nullable<Uuid>,
        path -> Text,
        generation_params -> Jsonb,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    user_preferences (user_id) {
        user_id -> Uuid,
        interest_theme_ids -> Array<Uuid>,
        safety_toggle_ids -> Array<Uuid>,
        unit_system -> Text,
        revision -> Int4,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    route_notes (id) {
        id -> Uuid,
        route_id -> Uuid,
        poi_id -> Nullable<Uuid>,
        user_id -> Uuid,
        body -> Text,
        revision -> Int4,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    route_progress (route_id, user_id) {
        route_id -> Uuid,
        user_id -> Uuid,
        visited_stop_ids -> Array<Uuid>,
        revision -> Int4,
        updated_at -> Timestamptz,
    }
}

diesel::table! {
    example_data_runs (seed_key) {
        seed_key -> Text,
        seeded_at -> Timestamptz,
        user_count -> Int4,
        seed -> Int8,
    }
}

// -----------------------------------------------------------------------------
// Offline bundle and walk session tables (roadmap 3.3.2)
// -----------------------------------------------------------------------------

diesel::table! {
    offline_bundles (id) {
        id -> Uuid,
        owner_user_id -> Nullable<Uuid>,
        device_id -> Text,
        kind -> Text,
        route_id -> Nullable<Uuid>,
        region_id -> Nullable<Text>,
        bounds -> Array<Float8>,
        min_zoom -> Int4,
        max_zoom -> Int4,
        estimated_size_bytes -> Int8,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
        status -> Text,
        progress -> Float4,
    }
}

diesel::table! {
    walk_sessions (id) {
        id -> Uuid,
        user_id -> Uuid,
        route_id -> Uuid,
        started_at -> Timestamptz,
        ended_at -> Nullable<Timestamptz>,
        primary_stats -> Jsonb,
        secondary_stats -> Jsonb,
        highlighted_poi_ids -> Array<Uuid>,
        created_at -> Timestamptz,
        updated_at -> Timestamptz,
    }
}

// -----------------------------------------------------------------------------
// Data platform baseline tables (roadmap 3.1.1)
// -----------------------------------------------------------------------------

diesel::table! {
    interest_themes (id) {
        id -> Uuid,
        name -> Text,
        description -> Nullable<Text>,
    }
}

diesel::table! {
    user_interest_themes (user_id, theme_id) {
        user_id -> Uuid,
        theme_id -> Uuid,
    }
}

diesel::table! {
    pois (element_type, id) {
        element_type -> Text,
        id -> Int8,
        location -> Text,
        osm_tags -> Jsonb,
        narrative -> Nullable<Text>,
        popularity_score -> Float4,
    }
}

diesel::table! {
    osm_ingestion_provenance (id) {
        id -> Uuid,
        geofence_id -> Text,
        source_url -> Text,
        input_digest -> Text,
        imported_at -> Timestamptz,
        bounds_min_lng -> Float8,
        bounds_min_lat -> Float8,
        bounds_max_lng -> Float8,
        bounds_max_lat -> Float8,
        raw_poi_count -> Int8,
        filtered_poi_count -> Int8,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    poi_interest_themes (poi_element_type, poi_id, theme_id) {
        poi_element_type -> Text,
        poi_id -> Int8,
        theme_id -> Uuid,
    }
}

diesel::table! {
    route_pois (route_id, poi_element_type, poi_id) {
        route_id -> Uuid,
        poi_element_type -> Text,
        poi_id -> Int8,
        position -> Int4,
    }
}

diesel::table! {
    route_categories (id) {
        id -> Uuid,
        slug -> Text,
        icon_key -> Text,
        localizations -> Jsonb,
        route_count -> Int4,
    }
}

diesel::table! {
    themes (id) {
        id -> Uuid,
        slug -> Text,
        icon_key -> Text,
        localizations -> Jsonb,
        image -> Jsonb,
        walk_count -> Int4,
        distance_range_metres -> Array<Int4>,
        rating -> Float4,
    }
}

diesel::table! {
    route_summaries (id) {
        id -> Uuid,
        route_id -> Uuid,
        category_id -> Uuid,
        theme_id -> Uuid,
        slug -> Nullable<Text>,
        localizations -> Jsonb,
        hero_image -> Jsonb,
        distance_metres -> Int4,
        duration_seconds -> Int4,
        rating -> Float4,
        badge_ids -> Array<Uuid>,
        difficulty -> Text,
        interest_theme_ids -> Array<Uuid>,
        created_at -> Timestamptz,
    }
}

diesel::table! {
    route_collections (id) {
        id -> Uuid,
        slug -> Text,
        icon_key -> Text,
        localizations -> Jsonb,
        lead_image -> Jsonb,
        map_preview -> Jsonb,
        distance_range_metres -> Array<Int4>,
        duration_range_seconds -> Array<Int4>,
        difficulty -> Text,
        route_ids -> Array<Uuid>,
    }
}

diesel::table! {
    trending_route_highlights (id) {
        id -> Uuid,
        route_summary_id -> Uuid,
        trend_delta -> Text,
        subtitle_localizations -> Jsonb,
        highlighted_at -> Timestamptz,
    }
}

diesel::table! {
    community_picks (id) {
        id -> Uuid,
        route_summary_id -> Nullable<Uuid>,
        user_id -> Nullable<Uuid>,
        localizations -> Jsonb,
        curator_display_name -> Text,
        curator_avatar -> Jsonb,
        rating -> Float4,
        distance_metres -> Int4,
        duration_seconds -> Int4,
        saves -> Int4,
        picked_at -> Timestamptz,
    }
}

diesel::table! {
    tags (id) {
        id -> Uuid,
        slug -> Text,
        icon_key -> Text,
        localizations -> Jsonb,
    }
}

diesel::table! {
    badges (id) {
        id -> Uuid,
        slug -> Text,
        icon_key -> Text,
        localizations -> Jsonb,
    }
}

diesel::table! {
    safety_toggles (id) {
        id -> Uuid,
        slug -> Text,
        icon_key -> Text,
        localizations -> Jsonb,
    }
}

diesel::table! {
    safety_presets (id) {
        id -> Uuid,
        slug -> Text,
        icon_key -> Text,
        localizations -> Jsonb,
        safety_toggle_ids -> Array<Uuid>,
    }
}

// -----------------------------------------------------------------------------
// Foreign key relationships
// -----------------------------------------------------------------------------

diesel::joinable!(routes -> users (user_id));
diesel::joinable!(user_preferences -> users (user_id));
diesel::joinable!(route_notes -> routes (route_id));
diesel::joinable!(route_notes -> users (user_id));
diesel::joinable!(route_progress -> routes (route_id));
diesel::joinable!(route_progress -> users (user_id));
diesel::joinable!(offline_bundles -> users (owner_user_id));
diesel::joinable!(offline_bundles -> routes (route_id));
diesel::joinable!(walk_sessions -> users (user_id));
diesel::joinable!(walk_sessions -> routes (route_id));
diesel::joinable!(user_interest_themes -> users (user_id));
diesel::joinable!(user_interest_themes -> interest_themes (theme_id));
diesel::joinable!(poi_interest_themes -> interest_themes (theme_id));
diesel::joinable!(route_pois -> routes (route_id));
diesel::joinable!(route_summaries -> routes (route_id));
diesel::joinable!(route_summaries -> route_categories (category_id));
diesel::joinable!(route_summaries -> themes (theme_id));
diesel::joinable!(trending_route_highlights -> route_summaries (route_summary_id));
diesel::joinable!(community_picks -> route_summaries (route_summary_id));
diesel::joinable!(community_picks -> users (user_id));

diesel::allow_tables_to_appear_in_same_query!(
    badges,
    community_picks,
    example_data_runs,
    idempotency_keys,
    interest_themes,
    offline_bundles,
    osm_ingestion_provenance,
    poi_interest_themes,
    pois,
    route_categories,
    route_collections,
    route_notes,
    route_pois,
    route_progress,
    route_summaries,
    routes,
    safety_presets,
    safety_toggles,
    tags,
    themes,
    trending_route_highlights,
    user_interest_themes,
    user_preferences,
    users,
    walk_sessions,
);

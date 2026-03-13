export type Json =
    | string
    | number
    | boolean
    | null
    | { [key: string]: Json | undefined }
    | Json[];

export type Database = {
    graphql_public: {
        Tables: {
            [_ in never]: never;
        };
        Views: {
            [_ in never]: never;
        };
        Functions: {
            graphql: {
                Args: {
                    extensions?: Json;
                    operationName?: string;
                    query?: string;
                    variables?: Json;
                };
                Returns: Json;
            };
        };
        Enums: {
            [_ in never]: never;
        };
        CompositeTypes: {
            [_ in never]: never;
        };
    };
    public: {
        Tables: {
            beatmap_scores: {
                Row: {
                    beatmap_id: number;
                    id: string;
                    mods: string[];
                    played_at: string;
                    player_handle: string;
                    pp: number;
                    profile_id: string | null;
                    score: number;
                };
                Insert: {
                    beatmap_id: number;
                    id?: string;
                    mods?: string[];
                    played_at?: string;
                    player_handle: string;
                    pp: number;
                    profile_id?: string | null;
                    score: number;
                };
                Update: {
                    beatmap_id?: number;
                    id?: string;
                    mods?: string[];
                    played_at?: string;
                    player_handle?: string;
                    pp?: number;
                    profile_id?: string | null;
                    score?: number;
                };
                Relationships: [
                    {
                        foreignKeyName: "beatmap_scores_beatmap_id_fkey";
                        columns: ["beatmap_id"];
                        isOneToOne: false;
                        referencedRelation: "beatmaps";
                        referencedColumns: ["id"];
                    },
                    {
                        foreignKeyName: "beatmap_scores_profile_id_fkey";
                        columns: ["profile_id"];
                        isOneToOne: false;
                        referencedRelation: "profiles";
                        referencedColumns: ["id"];
                    },
                ];
            };
            beatmap_tags: {
                Row: {
                    beatmap_id: number;
                    id: string;
                    tag: string;
                };
                Insert: {
                    beatmap_id: number;
                    id?: string;
                    tag: string;
                };
                Update: {
                    beatmap_id?: number;
                    id?: string;
                    tag?: string;
                };
                Relationships: [
                    {
                        foreignKeyName: "beatmap_tags_beatmap_id_fkey";
                        columns: ["beatmap_id"];
                        isOneToOne: false;
                        referencedRelation: "beatmaps";
                        referencedColumns: ["id"];
                    },
                ];
            };
            beatmaps: {
                Row: {
                    artist: string;
                    audio_url: string;
                    bpm: number;
                    community_stars: number;
                    created_at: string;
                    creator: string;
                    data_url: string;
                    description: string | null;
                    difficulty: number;
                    downloads: number | null;
                    id: number;
                    length_seconds: number;
                    likes: number | null;
                    mapper_id: string;
                    music_source: string;
                    music_start_seconds: number;
                    music_unicode_author: string;
                    music_unicode_title: string;
                    name: string;
                    nominator_stars: number;
                    plays: number | null;
                    ranked_at: string | null;
                    spawn_position: number[];
                    status: string | null;
                    timeline_duration_seconds: number;
                    title: string;
                    updated_at: string;
                };
                Insert: {
                    artist: string;
                    audio_url: string;
                    bpm?: number;
                    community_stars?: number;
                    created_at?: string;
                    creator?: string;
                    data_url: string;
                    description?: string | null;
                    difficulty?: number;
                    downloads?: number | null;
                    id?: number;
                    length_seconds?: number;
                    likes?: number | null;
                    mapper_id: string;
                    music_source?: string;
                    music_start_seconds?: number;
                    music_unicode_author?: string;
                    music_unicode_title?: string;
                    name: string;
                    nominator_stars?: number;
                    plays?: number | null;
                    ranked_at?: string | null;
                    spawn_position?: number[];
                    status?: string | null;
                    timeline_duration_seconds?: number;
                    title: string;
                    updated_at?: string;
                };
                Update: {
                    artist?: string;
                    audio_url?: string;
                    bpm?: number;
                    community_stars?: number;
                    created_at?: string;
                    creator?: string;
                    data_url?: string;
                    description?: string | null;
                    difficulty?: number;
                    downloads?: number | null;
                    id?: number;
                    length_seconds?: number;
                    likes?: number | null;
                    mapper_id?: string;
                    music_source?: string;
                    music_start_seconds?: number;
                    music_unicode_author?: string;
                    music_unicode_title?: string;
                    name?: string;
                    nominator_stars?: number;
                    plays?: number | null;
                    ranked_at?: string | null;
                    spawn_position?: number[];
                    status?: string | null;
                    timeline_duration_seconds?: number;
                    title?: string;
                    updated_at?: string;
                };
                Relationships: [
                    {
                        foreignKeyName: "beatmaps_mapper_id_fkey";
                        columns: ["mapper_id"];
                        isOneToOne: false;
                        referencedRelation: "profiles";
                        referencedColumns: ["id"];
                    },
                ];
            };
            comment_votes: {
                Row: {
                    comment_id: string;
                    created_at: string;
                    id: string;
                    user_id: string;
                    vote: number;
                };
                Insert: {
                    comment_id: string;
                    created_at?: string;
                    id?: string;
                    user_id: string;
                    vote: number;
                };
                Update: {
                    comment_id?: string;
                    created_at?: string;
                    id?: string;
                    user_id?: string;
                    vote?: number;
                };
                Relationships: [
                    {
                        foreignKeyName: "comment_votes_comment_id_fkey";
                        columns: ["comment_id"];
                        isOneToOne: false;
                        referencedRelation: "comments";
                        referencedColumns: ["id"];
                    },
                ];
            };
            comments: {
                Row: {
                    body: string;
                    created_at: string;
                    id: string;
                    parent_id: string | null;
                    profile_id: string | null;
                    resource_id: string;
                    resource_type: string;
                    updated_at: string | null;
                    votes: number;
                };
                Insert: {
                    body: string;
                    created_at?: string;
                    id?: string;
                    parent_id?: string | null;
                    profile_id?: string | null;
                    resource_id: string;
                    resource_type: string;
                    updated_at?: string | null;
                    votes?: number;
                };
                Update: {
                    body?: string;
                    created_at?: string;
                    id?: string;
                    parent_id?: string | null;
                    profile_id?: string | null;
                    resource_id?: string;
                    resource_type?: string;
                    updated_at?: string | null;
                    votes?: number;
                };
                Relationships: [
                    {
                        foreignKeyName: "comments_parent_id_fkey";
                        columns: ["parent_id"];
                        isOneToOne: false;
                        referencedRelation: "comments";
                        referencedColumns: ["id"];
                    },
                    {
                        foreignKeyName: "comments_profile_id_fkey";
                        columns: ["profile_id"];
                        isOneToOne: false;
                        referencedRelation: "profiles";
                        referencedColumns: ["id"];
                    },
                ];
            };
            profile_rank_pp_snapshots: {
                Row: {
                    country_rank: number | null;
                    created_at: string;
                    global_rank: number | null;
                    id: number;
                    profile_id: string;
                    snapshot_date: string;
                    total_pp: number;
                };
                Insert: {
                    country_rank?: number | null;
                    created_at?: string;
                    global_rank?: number | null;
                    id?: number;
                    profile_id: string;
                    snapshot_date?: string;
                    total_pp: number;
                };
                Update: {
                    country_rank?: number | null;
                    created_at?: string;
                    global_rank?: number | null;
                    id?: number;
                    profile_id?: string;
                    snapshot_date?: string;
                    total_pp?: number;
                };
                Relationships: [
                    {
                        foreignKeyName: "profile_rank_pp_snapshots_profile_id_fkey";
                        columns: ["profile_id"];
                        isOneToOne: false;
                        referencedRelation: "profiles";
                        referencedColumns: ["id"];
                    },
                ];
            };
            profile_stats: {
                Row: {
                    country_rank: number | null;
                    global_rank: number | null;
                    maps_cleared: number;
                    profile_id: string;
                    ranked_score: number;
                    total_pp: number;
                    updated_at: string;
                };
                Insert: {
                    country_rank?: number | null;
                    global_rank?: number | null;
                    maps_cleared?: number;
                    profile_id: string;
                    ranked_score?: number;
                    total_pp?: number;
                    updated_at?: string;
                };
                Update: {
                    country_rank?: number | null;
                    global_rank?: number | null;
                    maps_cleared?: number;
                    profile_id?: string;
                    ranked_score?: number;
                    total_pp?: number;
                    updated_at?: string;
                };
                Relationships: [
                    {
                        foreignKeyName: "profile_stats_profile_id_fkey";
                        columns: ["profile_id"];
                        isOneToOne: true;
                        referencedRelation: "profiles";
                        referencedColumns: ["id"];
                    },
                ];
            };
            profiles: {
                Row: {
                    avatar_url: string | null;
                    banned_until: string | null;
                    bio: string;
                    country: string;
                    favorite_mods: string[];
                    follower_count: number;
                    id: string;
                    is_admin: boolean;
                    joined_at: string;
                    last_seen_at: string | null;
                    mapper_tier: string;
                    muted_until: string | null;
                    number_id: number;
                    updated_at: string | null;
                    username: string | null;
                    website: string | null;
                };
                Insert: {
                    avatar_url?: string | null;
                    banned_until?: string | null;
                    bio?: string;
                    country?: string;
                    favorite_mods?: string[];
                    follower_count?: number;
                    id: string;
                    is_admin?: boolean;
                    joined_at?: string;
                    last_seen_at?: string | null;
                    mapper_tier?: string;
                    muted_until?: string | null;
                    number_id?: never;
                    updated_at?: string | null;
                    username?: string | null;
                    website?: string | null;
                };
                Update: {
                    avatar_url?: string | null;
                    banned_until?: string | null;
                    bio?: string;
                    country?: string;
                    favorite_mods?: string[];
                    follower_count?: number;
                    id?: string;
                    is_admin?: boolean;
                    joined_at?: string;
                    last_seen_at?: string | null;
                    mapper_tier?: string;
                    muted_until?: string | null;
                    number_id?: never;
                    updated_at?: string | null;
                    username?: string | null;
                    website?: string | null;
                };
                Relationships: [];
            };
            user_2fa_config: {
                Row: {
                    backup_codes: string[];
                    created_at: string | null;
                    current_challenge: string | null;
                    totp_enabled: boolean;
                    totp_secret: string | null;
                    updated_at: string | null;
                    user_id: string;
                    webauthn_enabled: boolean;
                };
                Insert: {
                    backup_codes?: string[];
                    created_at?: string | null;
                    current_challenge?: string | null;
                    totp_enabled?: boolean;
                    totp_secret?: string | null;
                    updated_at?: string | null;
                    user_id: string;
                    webauthn_enabled?: boolean;
                };
                Update: {
                    backup_codes?: string[];
                    created_at?: string | null;
                    current_challenge?: string | null;
                    totp_enabled?: boolean;
                    totp_secret?: string | null;
                    updated_at?: string | null;
                    user_id?: string;
                    webauthn_enabled?: boolean;
                };
                Relationships: [];
            };
            user_webauthn_credentials: {
                Row: {
                    counter: number;
                    created_at: string | null;
                    id: string;
                    public_key: string;
                    transports: string[];
                    user_id: string;
                };
                Insert: {
                    counter?: number;
                    created_at?: string | null;
                    id: string;
                    public_key: string;
                    transports?: string[];
                    user_id: string;
                };
                Update: {
                    counter?: number;
                    created_at?: string | null;
                    id?: string;
                    public_key?: string;
                    transports?: string[];
                    user_id?: string;
                };
                Relationships: [];
            };
        };
        Views: {
            [_ in never]: never;
        };
        Functions: {
            apply_profile_restriction: {
                Args: {
                    restriction_type: string;
                    restriction_until?: string;
                    target_user_id: string;
                };
                Returns: undefined;
            };
            capture_daily_rank_pp_snapshots: { Args: never; Returns: number };
            check_if_email_exists: {
                Args: { email_to_check: string };
                Returns: boolean;
            };
            increment_beatmap_downloads: {
                Args: { target_beatmap_id: number };
                Returns: undefined;
            };
            recalculate_ranks: { Args: never; Returns: undefined };
            resolve_email_to_id: {
                Args: { email_to_resolve: string };
                Returns: string;
            };
            resolve_username_to_email: {
                Args: { username_to_resolve: string };
                Returns: string;
            };
            update_last_seen: { Args: never; Returns: undefined };
        };
        Enums: {
            [_ in never]: never;
        };
        CompositeTypes: {
            [_ in never]: never;
        };
    };
};

type DatabaseWithoutInternals = Omit<Database, "__InternalSupabase">;

type DefaultSchema = DatabaseWithoutInternals[Extract<
    keyof Database,
    "public"
>];

export type Tables<
    DefaultSchemaTableNameOrOptions extends
        | keyof (DefaultSchema["Tables"] & DefaultSchema["Views"])
        | { schema: keyof DatabaseWithoutInternals },
    TableName extends DefaultSchemaTableNameOrOptions extends {
        schema: keyof DatabaseWithoutInternals;
    }
        ? keyof (DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"] &
              DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Views"])
        : never = never,
> = DefaultSchemaTableNameOrOptions extends {
    schema: keyof DatabaseWithoutInternals;
}
    ? (DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"] &
          DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Views"])[TableName] extends {
          Row: infer R;
      }
        ? R
        : never
    : DefaultSchemaTableNameOrOptions extends keyof (DefaultSchema["Tables"] &
            DefaultSchema["Views"])
      ? (DefaultSchema["Tables"] &
            DefaultSchema["Views"])[DefaultSchemaTableNameOrOptions] extends {
            Row: infer R;
        }
          ? R
          : never
      : never;

export type TablesInsert<
    DefaultSchemaTableNameOrOptions extends
        | keyof DefaultSchema["Tables"]
        | { schema: keyof DatabaseWithoutInternals },
    TableName extends DefaultSchemaTableNameOrOptions extends {
        schema: keyof DatabaseWithoutInternals;
    }
        ? keyof DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"]
        : never = never,
> = DefaultSchemaTableNameOrOptions extends {
    schema: keyof DatabaseWithoutInternals;
}
    ? DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"][TableName] extends {
          Insert: infer I;
      }
        ? I
        : never
    : DefaultSchemaTableNameOrOptions extends keyof DefaultSchema["Tables"]
      ? DefaultSchema["Tables"][DefaultSchemaTableNameOrOptions] extends {
            Insert: infer I;
        }
          ? I
          : never
      : never;

export type TablesUpdate<
    DefaultSchemaTableNameOrOptions extends
        | keyof DefaultSchema["Tables"]
        | { schema: keyof DatabaseWithoutInternals },
    TableName extends DefaultSchemaTableNameOrOptions extends {
        schema: keyof DatabaseWithoutInternals;
    }
        ? keyof DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"]
        : never = never,
> = DefaultSchemaTableNameOrOptions extends {
    schema: keyof DatabaseWithoutInternals;
}
    ? DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"][TableName] extends {
          Update: infer U;
      }
        ? U
        : never
    : DefaultSchemaTableNameOrOptions extends keyof DefaultSchema["Tables"]
      ? DefaultSchema["Tables"][DefaultSchemaTableNameOrOptions] extends {
            Update: infer U;
        }
          ? U
          : never
      : never;

export type Enums<
    DefaultSchemaEnumNameOrOptions extends
        | keyof DefaultSchema["Enums"]
        | { schema: keyof DatabaseWithoutInternals },
    EnumName extends DefaultSchemaEnumNameOrOptions extends {
        schema: keyof DatabaseWithoutInternals;
    }
        ? keyof DatabaseWithoutInternals[DefaultSchemaEnumNameOrOptions["schema"]]["Enums"]
        : never = never,
> = DefaultSchemaEnumNameOrOptions extends {
    schema: keyof DatabaseWithoutInternals;
}
    ? DatabaseWithoutInternals[DefaultSchemaEnumNameOrOptions["schema"]]["Enums"][EnumName]
    : DefaultSchemaEnumNameOrOptions extends keyof DefaultSchema["Enums"]
      ? DefaultSchema["Enums"][DefaultSchemaEnumNameOrOptions]
      : never;

export type CompositeTypes<
    PublicCompositeTypeNameOrOptions extends
        | keyof DefaultSchema["CompositeTypes"]
        | { schema: keyof DatabaseWithoutInternals },
    CompositeTypeName extends PublicCompositeTypeNameOrOptions extends {
        schema: keyof DatabaseWithoutInternals;
    }
        ? keyof DatabaseWithoutInternals[PublicCompositeTypeNameOrOptions["schema"]]["CompositeTypes"]
        : never = never,
> = PublicCompositeTypeNameOrOptions extends {
    schema: keyof DatabaseWithoutInternals;
}
    ? DatabaseWithoutInternals[PublicCompositeTypeNameOrOptions["schema"]]["CompositeTypes"][CompositeTypeName]
    : PublicCompositeTypeNameOrOptions extends keyof DefaultSchema["CompositeTypes"]
      ? DefaultSchema["CompositeTypes"][PublicCompositeTypeNameOrOptions]
      : never;

export const Constants = {
    graphql_public: {
        Enums: {},
    },
    public: {
        Enums: {},
    },
} as const;

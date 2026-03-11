-- Index for fast leaderboard retrieval.
create index if not exists beatmap_scores_beatmap_id_score_idx on public.beatmap_scores(beatmap_id, score desc);
-- Index for fast profile top-play retrieval.
create index if not exists beatmap_scores_profile_id_pp_idx on public.beatmap_scores(profile_id, pp desc, played_at desc);
-- Index to fetch comments by resource, sorted by newest first.
create index if not exists comments_resource_type_resource_id_created_at_idx on public.comments(resource_type, resource_id, created_at desc);
create index if not exists comments_parent_id_idx on public.comments(parent_id);
-- Index for fetching recent PP snapshots for a profile.
create index if not exists profile_rank_pp_snapshots_profile_id_snapshot_date_idx on public.profile_rank_pp_snapshots(profile_id, snapshot_date desc);

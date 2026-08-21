-- Replace Supabase bootstrap defaults with an explicit API privilege matrix.
revoke create on schema public from public, anon, authenticated, service_role;
grant usage on schema public to anon, authenticated, service_role;

alter default privileges in schema public
revoke all on tables from public, anon, authenticated, service_role;
alter default privileges in schema public
revoke all on sequences from public, anon, authenticated, service_role;
alter default privileges in schema public
revoke all on functions from public, anon, authenticated, service_role;

revoke all privileges on all tables in schema public
from public, anon, authenticated, service_role;
revoke all privileges on all sequences in schema public
from public, anon, authenticated, service_role;
revoke all privileges on all functions in schema public
from public, anon, authenticated, service_role;

grant select (
    id,
    number_id,
    updated_at,
    username,
    avatar_url,
    website,
    bio,
    country,
    joined_at,
    follower_count,
    mapper_tier,
    favorite_mods
) on public.profiles to anon, authenticated;
grant select on public.profile_stats to anon, authenticated;
grant select on public.beatmaps to anon, authenticated;
grant select on public.beatmap_tags to anon, authenticated;
grant select on public.beatmap_scores to anon, authenticated;
grant select on public.comments to anon, authenticated;
grant select on public.comment_votes to anon, authenticated;
grant select on public.profile_rank_pp_snapshots to anon, authenticated;

grant insert (id, username, avatar_url, website, bio, country, favorite_mods)
on public.profiles to authenticated;
grant update (username, avatar_url, website, bio, country, favorite_mods, updated_at)
on public.profiles to authenticated;
grant usage on sequence public.profiles_number_id_seq to authenticated;

grant insert (
    name,
    title,
    artist,
    mapper_id,
    description,
    creator,
    difficulty,
    audio_url,
    data_url,
    music_source,
    music_start_seconds,
    music_unicode_author,
    music_unicode_title,
    spawn_position,
    timeline_duration_seconds,
    length_seconds,
    bpm
) on public.beatmaps to authenticated;
grant update (
    name,
    title,
    artist,
    description,
    creator,
    difficulty,
    audio_url,
    data_url,
    music_source,
    music_start_seconds,
    music_unicode_author,
    music_unicode_title,
    spawn_position,
    timeline_duration_seconds,
    length_seconds,
    bpm,
    updated_at
) on public.beatmaps to authenticated;
grant usage on sequence public.beatmaps_id_seq to authenticated;

grant insert (beatmap_id, tag) on public.beatmap_tags to authenticated;
grant update (tag) on public.beatmap_tags to authenticated;
grant delete on public.beatmap_tags to authenticated;

grant insert (resource_type, resource_id, parent_id, profile_id, body)
on public.comments to authenticated;
grant update (body, updated_at) on public.comments to authenticated;
grant delete on public.comments to authenticated;

grant insert (comment_id, user_id, vote) on public.comment_votes to authenticated;
grant update (vote) on public.comment_votes to authenticated;
grant delete on public.comment_votes to authenticated;

grant select (user_id, totp_enabled, webauthn_enabled, created_at, updated_at)
on public.user_2fa_config to authenticated;

grant select on public.profiles to service_role;
grant all privileges on public.profile_stats to service_role;
grant select on public.beatmaps to service_role;
grant select on public.beatmap_tags to service_role;
grant select, insert on public.beatmap_scores to service_role;
grant select on public.comments to service_role;
grant select on public.comment_votes to service_role;
grant select on public.profile_rank_pp_snapshots to service_role;
grant all privileges on public.user_2fa_config to service_role;
grant all privileges on public.user_webauthn_credentials to service_role;
grant all privileges on public.auth_handoffs to service_role;
grant usage, select on all sequences in schema public to service_role;

revoke all on function public.claim_auth_handoff(uuid, text) from public;
grant execute on function public.claim_auth_handoff(uuid, text) to service_role;
revoke all on function public.cleanup_auth_handoffs() from public;
grant execute on function public.cleanup_auth_handoffs() to service_role;
revoke all on function public.resolve_username_to_email(text) from public;
grant execute on function public.resolve_username_to_email(text) to service_role;
revoke all on function public.resolve_email_to_id(text) from public;
grant execute on function public.resolve_email_to_id(text) to service_role;
revoke all on function public.check_if_email_exists(text) from public;
grant execute on function public.check_if_email_exists(text) to service_role;
revoke all on function public.update_last_seen() from public;
grant execute on function public.update_last_seen() to authenticated;
revoke all on function public.apply_profile_restriction(uuid, text, timestamp with time zone)
from public;
grant execute on function public.apply_profile_restriction(uuid, text, timestamp with time zone)
to authenticated;
revoke all on function public.recalculate_ranks() from public;
grant execute on function public.recalculate_ranks() to service_role;
revoke all on function public.capture_daily_rank_pp_snapshots() from public;
grant execute on function public.capture_daily_rank_pp_snapshots() to service_role;
revoke all on function public.increment_beatmap_downloads(bigint) from public;
grant execute on function public.increment_beatmap_downloads(bigint) to service_role;

create or replace function public.current_user_is_admin()
returns boolean
language sql
stable
security definer
set search_path = ''
as $$
    select coalesce(
        (auth.jwt() -> 'app_metadata' ->> 'is_admin')::boolean,
        false
    ) and exists (
        select 1
        from public.profiles
        where id = auth.uid()
            and is_admin
    );
$$;
revoke all on function public.current_user_is_admin() from public;
grant execute on function public.current_user_is_admin() to authenticated;

create or replace function public.apply_profile_restriction(
    target_user_id uuid,
    restriction_type text,
    restriction_until timestamp with time zone default null
) returns void
language plpgsql
security definer
set search_path = ''
as $$
declare
    actor_is_admin boolean;
begin
    select public.current_user_is_admin() into actor_is_admin;

    if actor_is_admin is not true then
        raise exception 'Only admins can restrict users';
    end if;

    if restriction_type = 'mute' then
        update public.profiles
        set muted_until = restriction_until
        where id = target_user_id;
    elsif restriction_type = 'unmute' then
        update public.profiles
        set muted_until = null
        where id = target_user_id;
    elsif restriction_type = 'ban' then
        update public.profiles
        set banned_until = restriction_until
        where id = target_user_id;
    elsif restriction_type = 'unban' then
        update public.profiles
        set banned_until = null
        where id = target_user_id;
    else
        raise exception 'Invalid restriction type';
    end if;

    if not found then
        raise exception 'Target user not found';
    end if;

    if restriction_type in ('ban', 'unban') then
        perform public.recalculate_ranks();
    end if;
end;
$$;

create or replace function public.can_current_user_comment()
returns boolean
language sql
stable
security definer
set search_path = ''
as $$
    select not exists (
        select 1
        from public.profiles
        where id = (select auth.uid())
            and (
                muted_until > now()
                or banned_until > now()
            )
    );
$$;
revoke all on function public.can_current_user_comment() from public;
grant execute on function public.can_current_user_comment() to authenticated;

drop policy if exists "Users can update own profile, except for admin status."
on public.profiles;
create policy "Users can update own profile, except for admin status."
on public.profiles for update to authenticated
using ((select auth.uid()) = id)
with check ((select auth.uid()) = id);

drop policy if exists "Users can post their own comments."
on public.comments;
create policy "Users can post their own comments."
on public.comments for insert to authenticated
with check (
    (select auth.uid()) = profile_id
    and (select public.can_current_user_comment())
);

drop policy if exists "Users can update their own comments."
on public.comments;
create policy "Users can update their own comments."
on public.comments for update to authenticated
using (
    (select auth.uid()) = profile_id
    and (select public.can_current_user_comment())
)
with check (
    (select auth.uid()) = profile_id
    and (select public.can_current_user_comment())
);

drop policy if exists "Users can delete their own comments, admins delete any."
on public.comments;
create policy "Users can delete their own comments, admins delete any."
on public.comments for delete to authenticated
using (
    (select auth.uid()) = profile_id
    or (select public.current_user_is_admin())
);

drop policy if exists "Users can manage their own WebAuthn credentials."
on public.user_webauthn_credentials;
create policy "Service role can manage WebAuthn credentials."
on public.user_webauthn_credentials for all to service_role
using (true) with check (true);
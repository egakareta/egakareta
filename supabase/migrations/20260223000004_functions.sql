-- Function to resolve a username to an email address.
-- This is used during login to allow users to sign in with their username.
-- Marked as 'security definer' to allow access to the 'auth' schema.
create or replace function public.resolve_username_to_email(username_to_resolve text) returns text language plpgsql security definer
set search_path = public,
    auth as $$
declare resolved_email text;
begin
select au.email into resolved_email
from public.profiles p
    join auth.users au on p.id = au.id
where p.username = username_to_resolve;
return resolved_email;
end;
$$;
-- Function to resolve an email address to a user ID.
-- Marked as 'security definer' to allow access to the 'auth' schema.
create or replace function public.resolve_email_to_id(email_to_resolve text) returns uuid language plpgsql security definer
set search_path = public,
    auth as $$
declare resolved_id uuid;
begin
select au.id into resolved_id
from auth.users au
where lower(au.email) = lower(email_to_resolve);
return resolved_id;
end;
$$;
-- Check if an email already exists in auth.users.
-- Used by signup and check-email routes.
create or replace function public.check_if_email_exists(email_to_check text) returns boolean language plpgsql security definer
set search_path = public,
    auth as $$
declare email_exists boolean;
begin
select exists(
        select 1
        from auth.users u
        where lower(u.email) = lower(email_to_check)
    ) into email_exists;
return coalesce(email_exists, false);
end;
$$;
-- Handle new user signups. When a new user is created in the auth.users table, this function will automatically insert a corresponding profile into the profiles table and initialize their 2FA configuration in the user_2fa_config table.
create or replace function public.handle_new_user() returns trigger language plpgsql security definer
set search_path = public as $$ begin
insert into public.profiles (id, username, country)
values (
        new.id,
        new.raw_user_meta_data->>'username',
        case
            when upper(coalesce(new.raw_user_meta_data->>'country', '')) ~ '^[A-Z]{2}$' then upper(new.raw_user_meta_data->>'country')
            else 'UN'
        end
    );
insert into public.user_2fa_config (user_id)
values (new.id);
return new;
end;
$$;
-- Whenever a new user is created in the auth.users table, automatically create a corresponding profile and 2FA config.
create trigger on_auth_user_created
after
insert on auth.users for each row execute procedure public.handle_new_user();
-- Utility function to update the `last_seen_at` timestamp for the currently authenticated user.
create or replace function public.update_last_seen() returns void language plpgsql security definer
set search_path = public as $$ begin
update public.profiles
set last_seen_at = timezone('utc', now())
where id = auth.uid();
end;
$$;
-- Admin function to apply a restriction (mute or ban) to a user profile. Only users with admin privileges can execute this function. It updates the `muted_until` or `banned_until` fields in the profiles table based on the restriction type and recalculates ranks if a ban status changes.
create or replace function public.apply_profile_restriction(
        target_user_id uuid,
        restriction_type text,
        restriction_until timestamp with time zone default null
    ) returns void language plpgsql security definer
set search_path = public as $$
declare actor_is_admin boolean;
begin
select coalesce(
        (auth.jwt()->'app_metadata'->>'is_admin')::boolean,
        false
    ) into actor_is_admin;
if actor_is_admin is not true then raise exception 'Only admins can restrict users';
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
else raise exception 'Invalid restriction type';
end if;
if not found then raise exception 'Target user not found';
end if;
if restriction_type in ('ban', 'unban') then perform public.recalculate_ranks();
end if;
end;
$$;
revoke execute on function public.apply_profile_restriction(uuid, text, timestamp with time zone)
from anon,
    public;
grant execute on function public.apply_profile_restriction(uuid, text, timestamp with time zone) to authenticated;
grant execute on function public.apply_profile_restriction(uuid, text, timestamp with time zone) to service_role;
-- Automatically updates the denormalized `votes` count in `comments` when a vote is added, changed, or removed.
-- This optimization allows for fast retrieval of comment scores without expensive joins.
create or replace function update_comment_votes_count() returns trigger as $$ begin if (TG_OP = 'INSERT') then
update public.comments
set votes = votes + NEW.vote
where id = NEW.comment_id;
ELSIF (TG_OP = 'DELETE') then
update public.comments
set votes = votes - OLD.vote
where id = OLD.comment_id;
ELSIF (TG_OP = 'UPDATE') then
update public.comments
set votes = votes - OLD.vote + NEW.vote
where id = NEW.comment_id;
end if;
return null;
end;
$$ LANGUAGE plpgsql security definer set search_path = public;
-- Trigger to keep comment vote tallies in sync across insert, update, and delete operations.
create trigger update_comment_votes_count_trigger
after
insert
    or
update
    or delete on public.comment_votes for EACH row execute function update_comment_votes_count();
-- Recalculate ranks
create or replace function public.recalculate_ranks() returns void language plpgsql security definer
set search_path = public as $$ begin -- Reset ranks for banned users
update public.profile_stats ps
set global_rank = null,
    country_rank = null
from public.profiles p
where ps.profile_id = p.id
    and (
        p.banned_until is not null
        and p.banned_until > now()
    );
-- Update global ranks for non-banned users
update public.profile_stats ps
set global_rank = t.new_rank
from (
        select ps2.profile_id,
            rank() over (
                order by ps2.total_pp desc
            ) as new_rank
        from public.profile_stats ps2
            join public.profiles p on p.id = ps2.profile_id
        where p.banned_until is null
            or p.banned_until <= now()
    ) t
where ps.profile_id = t.profile_id;
-- Update country ranks for non-banned users
update public.profile_stats ps
set country_rank = t.new_country_rank
from (
        select ps2.profile_id,
            rank() over (
                partition by p.country
                order by ps2.total_pp desc
            ) as new_country_rank
        from public.profile_stats ps2
            join public.profiles p on p.id = ps2.profile_id
        where p.banned_until is null
            or p.banned_until <= now()
    ) t
where ps.profile_id = t.profile_id;
end;
$$;
create or replace function public.capture_daily_rank_pp_snapshots() returns integer language plpgsql security definer
set search_path = public as $$
declare affected_rows integer := 0;
begin -- First, ensure ranks are up to date
perform public.recalculate_ranks();
insert into public.profile_rank_pp_snapshots (
        profile_id,
        snapshot_date,
        global_rank,
        country_rank,
        total_pp
    )
select p.id,
    (timezone('utc', now()))::date,
    ps.global_rank,
    ps.country_rank,
    ps.total_pp
from public.profiles p
    inner join public.profile_stats ps on ps.profile_id = p.id
where p.last_seen_at >= timezone('utc', now()) - interval '1 month' on conflict (profile_id, snapshot_date) do
update
set global_rank = excluded.global_rank,
    country_rank = excluded.country_rank,
    total_pp = excluded.total_pp,
    created_at = timezone('utc', now());
get diagnostics affected_rows = row_count;
return affected_rows;
end;
$$;
do $$ begin if exists (
    select 1
    from cron.job
    where jobname = 'daily-profile-rank-pp-snapshots'
) then perform cron.unschedule('daily-profile-rank-pp-snapshots');
end if;
end;
$$;
select cron.schedule(
        'daily-profile-rank-pp-snapshots',
        '15 0 * * *',
        $$select public.capture_daily_rank_pp_snapshots();
$$
);
select public.capture_daily_rank_pp_snapshots();

-- Atomically increments beatmap downloads for successful EGZ downloads.
create or replace function public.increment_beatmap_downloads(target_beatmap_id bigint) returns void language plpgsql security definer
set search_path = public as $$
begin
    update public.beatmaps
    set downloads = coalesce(downloads, 0) + 1
    where id = target_beatmap_id;
end;
$$;

revoke execute on function public.increment_beatmap_downloads(bigint)
from public;
grant execute on function public.increment_beatmap_downloads(bigint) to anon;
grant execute on function public.increment_beatmap_downloads(bigint) to authenticated;
grant execute on function public.increment_beatmap_downloads(bigint) to service_role;

begin;
set local search_path = public, extensions;

select plan(23);

select ok(
  not has_schema_privilege('anon', 'public', 'CREATE'),
  'anon cannot create objects in public'
);

select ok(
  not has_schema_privilege('authenticated', 'public', 'CREATE'),
  'authenticated cannot create objects in public'
);

select ok(
  not exists (
    select 1
    from pg_class c
    join pg_namespace n on n.oid = c.relnamespace
    where n.nspname = 'public'
      and c.relkind in ('r', 'p')
      and not c.relrowsecurity
  ),
  'all public tables have RLS enabled'
);

select ok(
  not exists (
    select 1
    from unnest(array[
      'public.profile_stats',
      'public.beatmaps',
      'public.beatmap_tags',
      'public.beatmap_scores',
      'public.comments',
      'public.comment_votes',
      'public.profile_rank_pp_snapshots'
    ]) as readable_table
    where not has_table_privilege('anon', readable_table, 'SELECT')
  ),
  'anon can read only intended public content tables'
);

select ok(
  not exists (
    select 1
    from unnest(array[
      'public.profile_stats',
      'public.beatmaps',
      'public.beatmap_tags',
      'public.beatmap_scores',
      'public.comments',
      'public.comment_votes',
      'public.profile_rank_pp_snapshots'
    ]) as readable_table
    where not has_table_privilege('authenticated', readable_table, 'SELECT')
  ),
  'authenticated can read intended public content tables'
);

select ok(
  has_column_privilege('anon', 'public.profiles', 'username', 'SELECT')
    and has_column_privilege('authenticated', 'public.profiles', 'bio', 'SELECT')
    and not has_column_privilege('anon', 'public.profiles', 'last_seen_at', 'SELECT')
    and not has_column_privilege('anon', 'public.profiles', 'is_admin', 'SELECT')
    and not has_column_privilege('authenticated', 'public.profiles', 'muted_until', 'SELECT')
    and not has_column_privilege('authenticated', 'public.profiles', 'banned_until', 'SELECT'),
  'public profile reads exclude activity and moderation fields'
);

select ok(
  not exists (
    select 1
    from pg_class c
    join pg_namespace n on n.oid = c.relnamespace
    where n.nspname = 'public'
      and c.relkind in ('r', 'p')
      and (
        has_table_privilege('anon', c.oid, 'INSERT')
        or has_table_privilege('anon', c.oid, 'UPDATE')
        or has_table_privilege('anon', c.oid, 'DELETE')
        or has_table_privilege('anon', c.oid, 'TRUNCATE')
      )
  ),
  'anon has no public table write privileges'
);

select ok(
  not exists (
    select 1
    from pg_class c
    join pg_namespace n on n.oid = c.relnamespace
    where n.nspname = 'public'
      and c.relkind in ('r', 'p')
      and (
        has_table_privilege('authenticated', c.oid, 'TRUNCATE')
        or has_table_privilege('authenticated', c.oid, 'REFERENCES')
        or has_table_privilege('authenticated', c.oid, 'TRIGGER')
      )
  ),
  'authenticated has no structural table privileges'
);

select ok(
  has_column_privilege('authenticated', 'public.profiles', 'username', 'INSERT')
    and has_column_privilege('authenticated', 'public.profiles', 'bio', 'UPDATE')
    and not has_column_privilege('authenticated', 'public.profiles', 'mapper_tier', 'UPDATE')
    and not has_column_privilege('authenticated', 'public.profiles', 'is_admin', 'INSERT')
    and not has_column_privilege('authenticated', 'public.profiles', 'muted_until', 'UPDATE'),
  'profile writes are limited to user-owned metadata columns'
);

select ok(
  has_column_privilege('authenticated', 'public.beatmaps', 'title', 'INSERT')
    and has_column_privilege('authenticated', 'public.beatmaps', 'description', 'UPDATE')
    and not has_column_privilege('authenticated', 'public.beatmaps', 'status', 'INSERT')
    and not has_column_privilege('authenticated', 'public.beatmaps', 'downloads', 'UPDATE')
    and not has_column_privilege('authenticated', 'public.beatmaps', 'ranked_at', 'UPDATE'),
  'beatmap writes exclude ranking and counter columns'
);

select ok(
  has_column_privilege('authenticated', 'public.beatmap_tags', 'tag', 'INSERT')
    and has_column_privilege('authenticated', 'public.beatmap_tags', 'tag', 'UPDATE')
    and not has_column_privilege('authenticated', 'public.beatmap_tags', 'beatmap_id', 'UPDATE')
    and has_table_privilege('authenticated', 'public.beatmap_tags', 'DELETE'),
  'beatmap tag writes cannot move tags between maps'
);

select ok(
  has_column_privilege('authenticated', 'public.comments', 'body', 'INSERT')
    and has_column_privilege('authenticated', 'public.comments', 'body', 'UPDATE')
    and not has_column_privilege('authenticated', 'public.comments', 'votes', 'INSERT')
    and not has_column_privilege('authenticated', 'public.comments', 'votes', 'UPDATE')
    and has_table_privilege('authenticated', 'public.comments', 'DELETE'),
  'comment writes exclude server-owned vote totals'
);

select ok(
  has_column_privilege('authenticated', 'public.comment_votes', 'vote', 'INSERT')
    and has_column_privilege('authenticated', 'public.comment_votes', 'vote', 'UPDATE')
    and not has_column_privilege('authenticated', 'public.comment_votes', 'comment_id', 'UPDATE')
    and not has_column_privilege('authenticated', 'public.comment_votes', 'created_at', 'INSERT')
    and has_table_privilege('authenticated', 'public.comment_votes', 'DELETE'),
  'vote writes cannot move votes or forge timestamps'
);

select ok(
  has_column_privilege('authenticated', 'public.user_2fa_config', 'totp_enabled', 'SELECT')
    and not has_column_privilege('authenticated', 'public.user_2fa_config', 'totp_secret', 'SELECT')
    and not has_table_privilege('authenticated', 'public.user_2fa_config', 'INSERT')
    and not has_table_privilege('authenticated', 'public.user_2fa_config', 'UPDATE'),
  'authenticated can read only non-secret 2FA state'
);

select ok(
  not has_table_privilege('anon', 'public.user_webauthn_credentials', 'SELECT')
    and not has_table_privilege('authenticated', 'public.user_webauthn_credentials', 'SELECT')
    and not has_table_privilege('anon', 'public.auth_handoffs', 'SELECT')
    and not has_table_privilege('authenticated', 'public.auth_handoffs', 'SELECT'),
  'WebAuthn credentials and auth handoffs are not client-readable'
);

select ok(
  has_table_privilege('service_role', 'public.profiles', 'SELECT')
    and has_table_privilege('service_role', 'public.user_2fa_config', 'UPDATE')
    and has_table_privilege('service_role', 'public.user_webauthn_credentials', 'INSERT')
    and has_table_privilege('service_role', 'public.auth_handoffs', 'DELETE')
    and has_table_privilege('service_role', 'public.beatmap_scores', 'INSERT'),
  'service role retains required trusted backend access'
);

select ok(
  not exists (
    select 1
    from pg_proc p
    join pg_namespace n on n.oid = p.pronamespace
    where n.nspname = 'public'
      and p.prosecdef
      and has_function_privilege('anon', p.oid, 'EXECUTE')
  ),
  'anon cannot execute public security-definer functions'
);

select ok(
  not exists (
    select 1
    from pg_proc p
    join pg_namespace n on n.oid = p.pronamespace
    cross join lateral aclexplode(coalesce(p.proacl, acldefault('f', p.proowner))) privilege
    where n.nspname = 'public'
      and p.prosecdef
      and privilege.grantee = 0
      and privilege.privilege_type = 'EXECUTE'
  ),
  'PUBLIC cannot execute public security-definer functions'
);

select ok(
  has_function_privilege('authenticated', 'public.update_last_seen()', 'EXECUTE')
    and has_function_privilege(
      'authenticated',
      'public.apply_profile_restriction(uuid, text, timestamp with time zone)',
      'EXECUTE'
    )
    and has_function_privilege('authenticated', 'public.can_current_user_comment()', 'EXECUTE')
    and has_function_privilege('authenticated', 'public.current_user_is_admin()', 'EXECUTE')
    and not has_function_privilege('authenticated', 'public.cleanup_auth_handoffs()', 'EXECUTE')
    and not has_function_privilege('authenticated', 'public.recalculate_ranks()', 'EXECUTE'),
  'authenticated can execute only intended application RPCs'
);

select ok(
  not exists (
    select 1
    from pg_proc p
    join pg_namespace n on n.oid = p.pronamespace
    where n.nspname = 'public'
      and p.prosecdef
      and not exists (
        select 1
        from unnest(coalesce(p.proconfig, array[]::text[])) setting
        where setting like 'search_path=%'
      )
  ),
  'all public security-definer functions fix their search path'
);

select ok(
  not exists (
    select 1
    from pg_default_acl d
    join pg_namespace n on n.oid = d.defaclnamespace
    cross join lateral aclexplode(d.defaclacl) privilege
    where d.defaclrole = (select oid from pg_roles where rolname = current_user)
      and n.nspname = 'public'
      and (
        privilege.grantee = 0
        or pg_get_userbyid(privilege.grantee) in ('anon', 'authenticated', 'service_role')
      )
  ),
  'migration owner has no broad API-role default privileges'
);

select ok(
  not exists (
    select 1
    from pg_class c
    join pg_namespace n on n.oid = c.relnamespace
    cross join lateral aclexplode(coalesce(c.relacl, acldefault('r', c.relowner))) privilege
    where n.nspname = 'public'
      and c.relkind in ('r', 'p')
      and privilege.grantee = 0
  ),
  'PUBLIC has no privileges on public tables'
);

select ok(
  not exists (
    select 1
    from pg_class c
    join pg_namespace n on n.oid = c.relnamespace
    cross join lateral aclexplode(
      coalesce(c.relacl, acldefault('S', c.relowner))
    ) privilege
    where n.nspname = 'public'
      and c.relkind = 'S'
      and pg_get_userbyid(privilege.grantee) = 'anon'
      and privilege.privilege_type = 'USAGE'
  ),
  'anon has no public sequence usage'
);

select * from finish();
rollback;
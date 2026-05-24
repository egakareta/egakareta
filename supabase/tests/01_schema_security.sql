begin;
set local search_path = public, extensions;
select plan(33);
select ok(
        exists (
            select 1
            from pg_proc p
                join pg_namespace n on n.oid = p.pronamespace
            where n.nspname = 'public'
                and p.proname = 'apply_profile_restriction'
        ),
        'apply_profile_restriction function exists'
    );
select ok(
        exists (
            select 1
            from pg_class c
                join pg_namespace n on n.oid = c.relnamespace
            where n.nspname = 'public'
                and c.relname = 'profiles'
                and c.relrowsecurity
        ),
        'RLS is enabled on public.profiles'
    );
select ok(
        exists (
            select 1
            from pg_class c
                join pg_namespace n on n.oid = c.relnamespace
            where n.nspname = 'public'
                and c.relname = 'user_2fa_config'
                and c.relrowsecurity
        ),
        'RLS is enabled on public.user_2fa_config'
    );
select ok(
        exists (
            select 1
            from pg_class c
                join pg_namespace n on n.oid = c.relnamespace
            where n.nspname = 'public'
                and c.relname = 'user_webauthn_credentials'
                and c.relrowsecurity
        ),
        'RLS is enabled on public.user_webauthn_credentials'
    );
select ok(
        exists (
            select 1
            from pg_policies
            where schemaname = 'public'
                and tablename = 'profiles'
                and policyname = 'Users can update own profile, except for admin status.'
        ),
        'hardened profile update policy exists'
    );
select ok(
        position(
            'is_admin' in coalesce(
                (
                    select with_check
                    from pg_policies
                    where schemaname = 'public'
                        and tablename = 'profiles'
                        and policyname = 'Users can update own profile, except for admin status.'
                ),
                ''
            )
        ) > 0,
        'profile update policy prevents self-admin escalation with with check'
    );
select ok(
        exists (
            select 1
            from pg_policies
            where schemaname = 'public'
                and tablename = 'user_2fa_config'
                and policyname = 'Users can view their own 2FA config.'
        ),
        '2FA select policy exists'
    );
select ok(
        exists (
            select 1
            from pg_policies
            where schemaname = 'public'
                and tablename = 'user_2fa_config'
                and policyname = 'Users can update their own 2FA config.'
        ),
        '2FA update policy exists'
    );
select ok(
        exists (
            select 1
            from pg_policies
            where schemaname = 'public'
                and tablename = 'user_2fa_config'
                and policyname = 'Users can insert their own 2FA config.'
        ),
        '2FA insert policy exists'
    );
select ok(
        exists (
            select 1
            from pg_policies
            where schemaname = 'public'
                and tablename = 'user_webauthn_credentials'
                and policyname = 'Users can manage their own WebAuthn credentials.'
        ),
        'WebAuthn manage policy exists'
    );
select ok(
        position(
            'auth.jwt()' in (
                select p.prosrc
                from pg_proc p
                    join pg_namespace n on n.oid = p.pronamespace
                where n.nspname = 'public'
                    and p.proname = 'apply_profile_restriction'
                limit 1
            )
        ) > 0, 'admin RPC authorization reads auth.jwt app_metadata'
    );
select ok(
        has_function_privilege(
            'authenticated',
            'public.apply_profile_restriction(uuid, text, timestamp with time zone)',
            'EXECUTE'
        ),
        'authenticated can execute apply_profile_restriction'
    );
select ok(
        has_function_privilege(
            'service_role',
            'public.apply_profile_restriction(uuid, text, timestamp with time zone)',
            'EXECUTE'
        ),
        'service_role can execute apply_profile_restriction'
    );
select ok(
        not exists (
            select 1
            from information_schema.role_routine_grants
            where routine_schema = 'public'
                and routine_name = 'apply_profile_restriction'
                and privilege_type = 'EXECUTE'
                and grantee = 'anon'
        ),
        'anon has no direct execute grant on apply_profile_restriction'
    );
select ok(
        not exists (
            select 1
            from information_schema.role_routine_grants
            where routine_schema = 'public'
                and routine_name = 'apply_profile_restriction'
                and privilege_type = 'EXECUTE'
                and grantee = 'PUBLIC'
        ),
        'PUBLIC has no direct execute grant on apply_profile_restriction'
    );
select ok(
        exists (
            select 1
            from pg_attribute a
                join pg_class c on c.oid = a.attrelid
                join pg_namespace n on n.oid = c.relnamespace
            where n.nspname = 'public'
                and c.relname = 'user_2fa_config'
                and a.attname = 'current_challenge'
                and not a.attisdropped
        ),
        'user_2fa_config.current_challenge exists'
    );
select ok(
        exists (
            select 1
            from pg_trigger t
                join pg_class c on c.oid = t.tgrelid
                join pg_namespace n on n.oid = c.relnamespace
            where n.nspname = 'auth'
                and c.relname = 'users'
                and t.tgname = 'on_auth_user_created'
                and not t.tgisinternal
        ),
        'on_auth_user_created trigger exists on auth.users'
    );
select ok(
        exists (
            select 1
            from pg_proc p
                join pg_namespace n on n.oid = p.pronamespace
            where n.nspname = 'public'
                and p.proname = 'handle_new_user'
        ),
        'handle_new_user function exists'
    );
select ok(
        exists (
            select 1
            from pg_proc p
                join pg_namespace n on n.oid = p.pronamespace
            where n.nspname = 'public'
                and p.proname = 'recalculate_ranks'
        ),
        'recalculate_ranks function exists'
    );
select ok(
        (
            select (
                    length(p.prosrc) - length(replace(p.prosrc, 'perform public.recalculate_ranks();', ''))
                ) / length('perform public.recalculate_ranks();')
            from pg_proc p
                join pg_namespace n on n.oid = p.pronamespace
            where n.nspname = 'public'
                and p.proname = 'apply_profile_restriction'
            limit 1
        ) = 1,
        'apply_profile_restriction invokes recalculate_ranks exactly once'
    );
select ok(
        not has_function_privilege(
            'anon',
            'public.resolve_username_to_email(text)',
            'EXECUTE'
        ),
        'anon cannot directly execute username email resolver'
    );
select ok(
        not has_function_privilege(
            'authenticated',
            'public.resolve_username_to_email(text)',
            'EXECUTE'
        ),
        'authenticated cannot directly execute username email resolver'
    );
select ok(
        has_function_privilege(
            'service_role',
            'public.resolve_username_to_email(text)',
            'EXECUTE'
        ),
        'service_role can execute username email resolver'
    );
select ok(
        not has_function_privilege(
            'anon',
            'public.check_if_email_exists(text)',
            'EXECUTE'
        ),
        'anon cannot directly execute email existence checker'
    );
select ok(
        not has_function_privilege(
            'authenticated',
            'public.check_if_email_exists(text)',
            'EXECUTE'
        ),
        'authenticated cannot directly execute email existence checker'
    );
select ok(
        has_function_privilege(
            'service_role',
            'public.check_if_email_exists(text)',
            'EXECUTE'
        ),
        'service_role can execute email existence checker'
    );
select ok(
        not has_function_privilege(
            'authenticated',
            'public.recalculate_ranks()',
            'EXECUTE'
        ),
        'authenticated cannot directly recalculate ranks'
    );
select ok(
        not has_function_privilege(
            'authenticated',
            'public.capture_daily_rank_pp_snapshots()',
            'EXECUTE'
        ),
        'authenticated cannot directly capture rank snapshots'
    );
select ok(
        not has_function_privilege(
            'anon',
            'public.increment_beatmap_downloads(bigint)',
            'EXECUTE'
        ),
        'anon cannot directly increment beatmap downloads'
    );
select ok(
        not has_function_privilege(
            'authenticated',
            'public.increment_beatmap_downloads(bigint)',
            'EXECUTE'
        ),
        'authenticated cannot directly increment beatmap downloads'
    );
select ok(
        has_function_privilege(
            'service_role',
            'public.increment_beatmap_downloads(bigint)',
            'EXECUTE'
        ),
        'service_role can increment beatmap downloads'
    );
select ok(
        has_function_privilege(
            'authenticated',
            'public.update_last_seen()',
            'EXECUTE'
        ),
        'authenticated can update own last_seen through RPC'
    );
select ok(
        not has_function_privilege(
            'anon',
            'public.update_last_seen()',
            'EXECUTE'
        ),
        'anon cannot update last_seen through RPC'
    );
select *
from finish();
rollback;
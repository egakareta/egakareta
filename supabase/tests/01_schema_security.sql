begin;
select plan(20);
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
            from information_schema.columns
            where table_schema = 'public'
                and table_name = 'user_2fa_config'
                and column_name = 'current_challenge'
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
select *
from finish();
rollback;
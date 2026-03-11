begin;

select plan(12);

create or replace function public._test_capture_error(sql_to_run text)
returns text
language plpgsql
as $$
begin
  execute sql_to_run;
  return 'NO_ERROR';
exception
  when others then
    return sqlstate || ': ' || sqlerrm;
end;
$$;

insert into auth.users (
  instance_id,
  id,
  aud,
  role,
  email,
  encrypted_password,
  email_confirmed_at,
  raw_app_meta_data,
  raw_user_meta_data,
  created_at,
  updated_at,
  confirmation_token,
  email_change,
  email_change_token_new,
  recovery_token
)
values
  (
    '00000000-0000-0000-0000-000000000000',
    'aaaaaaaa-1111-1111-1111-111111111111',
    'authenticated',
    'authenticated',
    'dbtest-a@example.com',
    crypt('asdasd', gen_salt('bf')),
    now(),
    '{"provider":"email","providers":["email"]}',
    '{"username":"dbtest_a"}',
    now(),
    now(),
    '',
    '',
    '',
    ''
  ),
  (
    '00000000-0000-0000-0000-000000000000',
    'bbbbbbbb-2222-2222-2222-222222222222',
    'authenticated',
    'authenticated',
    'dbtest-b@example.com',
    crypt('asdasd', gen_salt('bf')),
    now(),
    '{"provider":"email","providers":["email"]}',
    '{"username":"dbtest_b"}',
    now(),
    now(),
    '',
    '',
    '',
    ''
  );

select ok(
  exists (
    select 1 from public.profiles where id = 'aaaaaaaa-1111-1111-1111-111111111111'
  ),
  'handle_new_user creates profile for user A'
);

select ok(
  exists (
    select 1 from public.profiles where id = 'bbbbbbbb-2222-2222-2222-222222222222'
  ),
  'handle_new_user creates profile for user B'
);

select ok(
  exists (
    select 1 from public.user_2fa_config where user_id = 'aaaaaaaa-1111-1111-1111-111111111111'
  ),
  'handle_new_user creates 2FA config for user A'
);

select ok(
  exists (
    select 1 from public.user_2fa_config where user_id = 'bbbbbbbb-2222-2222-2222-222222222222'
  ),
  'handle_new_user creates 2FA config for user B'
);

update public.profiles
set bio = 'target-original'
where id = 'bbbbbbbb-2222-2222-2222-222222222222';

set local role authenticated;
select set_config('request.jwt.claim.role', 'authenticated', true);
select set_config('request.jwt.claim.sub', 'aaaaaaaa-1111-1111-1111-111111111111', true);

update public.profiles
set bio = 'updated-by-owner'
where id = 'aaaaaaaa-1111-1111-1111-111111111111';

select is(
  (select bio from public.profiles where id = 'aaaaaaaa-1111-1111-1111-111111111111'),
  'updated-by-owner',
  'authenticated user can update own profile fields'
);

select is(
  public._test_capture_error($$
    update public.profiles
    set is_admin = true
    where id = 'aaaaaaaa-1111-1111-1111-111111111111'
  $$),
  '42501: new row violates row-level security policy for table "profiles"',
  'authenticated user cannot self-promote to admin'
);

update public.profiles
set bio = 'cross-user-mutation'
where id = 'bbbbbbbb-2222-2222-2222-222222222222';

select is(
  (select bio from public.profiles where id = 'bbbbbbbb-2222-2222-2222-222222222222'),
  'target-original',
  'authenticated user cannot update another user profile'
);

select is(
  (select count(*)::int from public.user_2fa_config where user_id = 'aaaaaaaa-1111-1111-1111-111111111111'),
  1,
  'authenticated user can view own 2FA config'
);

select is(
  (select count(*)::int from public.user_2fa_config where user_id = 'bbbbbbbb-2222-2222-2222-222222222222'),
  0,
  'authenticated user cannot view another user 2FA config'
);

select is(
  public._test_capture_error($$
    insert into public.user_2fa_config (user_id, totp_enabled)
    values ('bbbbbbbb-2222-2222-2222-222222222222', true)
  $$),
  '42501: new row violates row-level security policy for table "user_2fa_config"',
  'authenticated user cannot insert another user 2FA row'
);

reset role;
set local role anon;
select set_config('request.jwt.claim.role', 'anon', true);
select set_config('request.jwt.claim.sub', '', true);

select is(
  (select count(*)::int from public.user_2fa_config),
  0,
  'anon cannot read user_2fa_config rows'
);

select is(
  (select count(*)::int from public.user_webauthn_credentials),
  0,
  'anon cannot read user_webauthn_credentials rows'
);

select * from finish();
rollback;

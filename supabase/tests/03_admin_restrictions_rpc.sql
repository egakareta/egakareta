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
    'cccccccc-3333-3333-3333-333333333333',
    'authenticated',
    'authenticated',
    'dbtest-actor@example.com',
    crypt('asdasd', gen_salt('bf')),
    now(),
    '{"provider":"email","providers":["email"],"is_admin":false}',
    '{"username":"dbtest_actor"}',
    now(),
    now(),
    '',
    '',
    '',
    ''
  ),
  (
    '00000000-0000-0000-0000-000000000000',
    'dddddddd-4444-4444-4444-444444444444',
    'authenticated',
    'authenticated',
    'dbtest-target@example.com',
    crypt('asdasd', gen_salt('bf')),
    now(),
    '{"provider":"email","providers":["email"]}',
    '{"username":"dbtest_target"}',
    now(),
    now(),
    '',
    '',
    '',
    ''
  );

insert into public.profile_stats (profile_id, total_pp)
values ('dddddddd-4444-4444-4444-444444444444', 500)
on conflict (profile_id) do nothing;

update public.profiles
set is_admin = true
where id = 'cccccccc-3333-3333-3333-333333333333';

set local role authenticated;
select set_config('request.jwt.claim.role', 'authenticated', true);
select set_config('request.jwt.claim.sub', 'cccccccc-3333-3333-3333-333333333333', true);
select set_config(
  'request.jwt.claims',
  '{"role":"authenticated","sub":"cccccccc-3333-3333-3333-333333333333","app_metadata":{"is_admin":false}}',
  true
);

select is(
  public._test_capture_error($$
    select public.apply_profile_restriction(
      'dddddddd-4444-4444-4444-444444444444',
      'mute',
      timezone('utc', now()) + interval '1 day'
    )
  $$),
  'P0001: Only admins can restrict users',
  'profile.is_admin=true does not authorize without admin app_metadata claim'
);

select set_config(
  'request.jwt.claims',
  '{"role":"authenticated","sub":"cccccccc-3333-3333-3333-333333333333","app_metadata":{"is_admin":true}}',
  true
);

select lives_ok(
  $$
    select public.apply_profile_restriction(
      'dddddddd-4444-4444-4444-444444444444',
      'mute',
      timezone('utc', now()) + interval '2 days'
    )
  $$,
  'admin app_metadata claim can mute target user'
);

select ok(
  (select muted_until is not null from public.profiles where id = 'dddddddd-4444-4444-4444-444444444444'),
  'mute sets muted_until'
);

select lives_ok(
  $$
    select public.apply_profile_restriction(
      'dddddddd-4444-4444-4444-444444444444',
      'unmute',
      null
    )
  $$,
  'admin app_metadata claim can unmute target user'
);

select ok(
  (select muted_until is null from public.profiles where id = 'dddddddd-4444-4444-4444-444444444444'),
  'unmute clears muted_until'
);

select lives_ok(
  $$
    select public.apply_profile_restriction(
      'dddddddd-4444-4444-4444-444444444444',
      'ban',
      timezone('utc', now()) + interval '2 days'
    )
  $$,
  'admin app_metadata claim can ban target user'
);

select ok(
  (select banned_until is not null from public.profiles where id = 'dddddddd-4444-4444-4444-444444444444'),
  'ban sets banned_until'
);

select lives_ok(
  $$
    select public.apply_profile_restriction(
      'dddddddd-4444-4444-4444-444444444444',
      'unban',
      null
    )
  $$,
  'admin app_metadata claim can unban target user'
);

select ok(
  (select banned_until is null from public.profiles where id = 'dddddddd-4444-4444-4444-444444444444'),
  'unban clears banned_until'
);

select is(
  public._test_capture_error($$
    select public.apply_profile_restriction(
      'dddddddd-4444-4444-4444-444444444444',
      'invalid-restriction',
      null
    )
  $$),
  'P0001: Invalid restriction type',
  'invalid restriction type is rejected'
);

select is(
  public._test_capture_error($$
    select public.apply_profile_restriction(
      'eeeeeeee-5555-5555-5555-555555555555',
      'mute',
      timezone('utc', now()) + interval '1 day'
    )
  $$),
  'P0001: Target user not found',
  'missing target user is rejected'
);

reset role;
set local role anon;
select set_config(
  'request.jwt.claims',
  '{"role":"anon"}',
  true
);

select is(
  public._test_capture_error($$
    select public.apply_profile_restriction(
      'dddddddd-4444-4444-4444-444444444444',
      'mute',
      timezone('utc', now()) + interval '1 day'
    )
  $$),
  '42501: permission denied for function apply_profile_restriction',
  'anon role cannot execute apply_profile_restriction'
);

select * from finish();
rollback;

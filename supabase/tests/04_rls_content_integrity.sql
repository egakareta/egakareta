begin;
set local search_path = public, extensions;

select plan(11);

create or replace function pg_temp._test_capture_error(sql_to_run text)
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
    'eeeeeeee-5555-5555-5555-555555555555',
    'authenticated',
    'authenticated',
    'dbtest-content@example.com',
    crypt('asdasd', gen_salt('bf')),
    now(),
    '{"provider":"email","providers":["email"]}',
    '{"username":"dbtest_content"}',
    now(),
    now(),
    '',
    '',
    '',
    ''
  );

set local role authenticated;
select set_config('request.jwt.claim.role', 'authenticated', true);
select set_config('request.jwt.claim.sub', 'eeeeeeee-5555-5555-5555-555555555555', true);

select ok(
  pg_temp._test_capture_error($$
    insert into public.comments (resource_type, resource_id, profile_id, body, votes)
    values ('beatmap', 'security-map', 'eeeeeeee-5555-5555-5555-555555555555', 'forged score', 999)
  $$) like '42501:%',
  'authenticated user cannot forge comment vote count on insert'
);

insert into public.comments (resource_type, resource_id, profile_id, body)
values ('beatmap', 'security-map', 'eeeeeeee-5555-5555-5555-555555555555', 'valid comment');

select is(
  (select count(*)::int from public.comments where body = 'valid comment'),
  1,
  'authenticated user can insert a normal own comment'
);

update public.comments
set body = 'edited comment'
where profile_id = 'eeeeeeee-5555-5555-5555-555555555555'
  and body = 'valid comment';

select is(
  (select count(*)::int from public.comments where body = 'edited comment'),
  1,
  'authenticated user can edit own comment body'
);

select ok(
  pg_temp._test_capture_error($$
    update public.comments
    set votes = 999
    where profile_id = 'eeeeeeee-5555-5555-5555-555555555555'
  $$) like '42501:%',
  'authenticated user cannot forge comment vote count on update'
);

insert into public.comments (resource_type, resource_id, profile_id, body)
values ('beatmap', 'security-map', 'eeeeeeee-5555-5555-5555-555555555555', 'second comment');

insert into public.comment_votes (comment_id, user_id, vote)
select id, 'eeeeeeee-5555-5555-5555-555555555555', 1
from public.comments
where body = 'edited comment';

select is(
  (select votes from public.comments where body = 'edited comment'),
  1,
  'comment vote trigger increments score for valid vote'
);

update public.comment_votes
set vote = -1
where user_id = 'eeeeeeee-5555-5555-5555-555555555555';

select is(
  (select votes from public.comments where body = 'edited comment'),
  -1,
  'authenticated user can change vote value without corrupting tally'
);

select ok(
  pg_temp._test_capture_error($$
    update public.comment_votes
    set comment_id = (
      select id from public.comments where body = 'second comment'
    )
    where user_id = 'eeeeeeee-5555-5555-5555-555555555555'
  $$) like '42501:%',
  'authenticated user cannot move a vote to another comment'
);

insert into public.beatmaps (
  name,
  title,
  artist,
  mapper_id,
  audio_url,
  data_url
)
values (
  'Security Map',
  'Security Song',
  'Security Artist',
  'eeeeeeee-5555-5555-5555-555555555555',
  'audio/security.ogg',
  'data/security.egz'
);

select is(
  (select count(*)::int from public.beatmaps where name = 'Security Map'),
  1,
  'authenticated mapper can insert own unranked beatmap without server counters'
);

select ok(
  pg_temp._test_capture_error($$
    insert into public.beatmaps (
      name,
      title,
      artist,
      mapper_id,
      audio_url,
      data_url,
      downloads
    )
    values (
      'Forged Download Map',
      'Security Song',
      'Security Artist',
      'eeeeeeee-5555-5555-5555-555555555555',
      'audio/forged.ogg',
      'data/forged.egz',
      1000000
    )
  $$) like '42501:%',
  'authenticated mapper cannot forge beatmap downloads on insert'
);

update public.beatmaps
set title = 'Edited Security Song'
where mapper_id = 'eeeeeeee-5555-5555-5555-555555555555'
  and name = 'Security Map';

select is(
  (select title from public.beatmaps where name = 'Security Map'),
  'Edited Security Song',
  'authenticated mapper can edit own beatmap metadata'
);

select ok(
  pg_temp._test_capture_error($$
    update public.beatmaps
    set downloads = 1000000
    where mapper_id = 'eeeeeeee-5555-5555-5555-555555555555'
  $$) like '42501:%',
  'authenticated mapper cannot forge beatmap downloads on update'
);

select * from finish();
rollback;
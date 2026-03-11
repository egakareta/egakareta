-- Seed demo Auth users for local development.
-- Credentials (all users): asdasd
insert into auth.users (
        instance_id,
        id,
        aud,
        role,
        email,
        encrypted_password,
        email_confirmed_at,
        recovery_sent_at,
        last_sign_in_at,
        raw_app_meta_data,
        raw_user_meta_data,
        created_at,
        updated_at,
        confirmation_token,
        email_change,
        email_change_token_new,
        recovery_token
    )
values (
        '00000000-0000-0000-0000-000000000000',
        '11111111-1111-1111-1111-111111111111',
        'authenticated',
        'authenticated',
        'nova@example.com',
        crypt('asdasd', gen_salt('bf')),
        now(),
        now(),
        now(),
        '{"provider":"email","providers":["email"],"is_admin":true}',
        '{"username":"nova"}',
        now(),
        now(),
        '',
        '',
        '',
        ''
    ),
    (
        '00000000-0000-0000-0000-000000000000',
        '22222222-2222-2222-2222-222222222222',
        'authenticated',
        'authenticated',
        'iris@example.com',
        crypt('asdasd', gen_salt('bf')),
        now(),
        now(),
        now(),
        '{"provider":"email","providers":["email"]}',
        '{"username":"iris"}',
        now(),
        now(),
        '',
        '',
        '',
        ''
    ),
    (
        '00000000-0000-0000-0000-000000000000',
        '33333333-3333-3333-3333-333333333333',
        'authenticated',
        'authenticated',
        'zen@example.com',
        crypt('asdasd', gen_salt('bf')),
        now(),
        now(),
        now(),
        '{"provider":"email","providers":["email"]}',
        '{"username":"zen"}',
        now(),
        now(),
        '',
        '',
        '',
        ''
    ),
    (
        '00000000-0000-0000-0000-000000000000',
        '44444444-4444-4444-4444-444444444444',
        'authenticated',
        'authenticated',
        'lyra@example.com',
        crypt('asdasd', gen_salt('bf')),
        now(),
        now(),
        now(),
        '{"provider":"email","providers":["email"]}',
        '{"username":"lyra"}',
        now(),
        now(),
        '',
        '',
        '',
        ''
    ) on conflict (id) do
update
set email = excluded.email,
    encrypted_password = excluded.encrypted_password,
    email_confirmed_at = excluded.email_confirmed_at,
    recovery_sent_at = excluded.recovery_sent_at,
    last_sign_in_at = excluded.last_sign_in_at,
    raw_app_meta_data = excluded.raw_app_meta_data,
    raw_user_meta_data = excluded.raw_user_meta_data,
    confirmation_token = excluded.confirmation_token,
    email_change = excluded.email_change,
    email_change_token_new = excluded.email_change_token_new,
    recovery_token = excluded.recovery_token,
    updated_at = now();
insert into auth.identities (
        id,
        user_id,
        identity_data,
        provider,
        provider_id,
        last_sign_in_at,
        created_at,
        updated_at
    )
values (
        'aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
        '11111111-1111-1111-1111-111111111111',
        jsonb_build_object(
            'sub',
            '11111111-1111-1111-1111-111111111111',
            'email',
            'nova@example.com',
            'email_verified',
            true
        ),
        'email',
        'nova@example.com',
        now(),
        now(),
        now()
    ),
    (
        'bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb',
        '22222222-2222-2222-2222-222222222222',
        jsonb_build_object(
            'sub',
            '22222222-2222-2222-2222-222222222222',
            'email',
            'iris@example.com',
            'email_verified',
            true
        ),
        'email',
        'iris@example.com',
        now(),
        now(),
        now()
    ),
    (
        'cccccccc-cccc-cccc-cccc-cccccccccccc',
        '33333333-3333-3333-3333-333333333333',
        jsonb_build_object(
            'sub',
            '33333333-3333-3333-3333-333333333333',
            'email',
            'zen@example.com',
            'email_verified',
            true
        ),
        'email',
        'zen@example.com',
        now(),
        now(),
        now()
    ),
    (
        'dddddddd-dddd-dddd-dddd-dddddddddddd',
        '44444444-4444-4444-4444-444444444444',
        jsonb_build_object(
            'sub',
            '44444444-4444-4444-4444-444444444444',
            'email',
            'lyra@example.com',
            'email_verified',
            true
        ),
        'email',
        'lyra@example.com',
        now(),
        now(),
        now()
    ) on conflict (id) do
update
set provider_id = excluded.provider_id,
    identity_data = excluded.identity_data,
    last_sign_in_at = excluded.last_sign_in_at,
    updated_at = now();
insert into public.profiles (
        id,
        username,
        avatar_url,
        bio,
        country,
        joined_at,
        follower_count,
        mapper_tier,
        is_admin,
        favorite_mods,
        updated_at
    )
values (
        '11111111-1111-1111-1111-111111111111',
        'nova',
        'http://127.0.0.1:54321/storage/v1/object/public/avatars/nova.webp',
        'Speed-focused mapper and tester. Building flow maps with strict rhythm readability.',
        'CA',
        '2025-07-11',
        1231,
        'Ranked Mapper',
        true,
        array ['HD', 'HR'],
        now()
    ),
    (
        '22222222-2222-2222-2222-222222222222',
        'iris',
        'http://127.0.0.1:54321/storage/v1/object/public/avatars/iris.webp',
        'Editor artist. I design scenic maps that still feel clean to read at speed.',
        'SE',
        '2025-09-03',
        809,
        'Nominator',
        false,
        array ['NM', 'EZ'],
        now()
    ),
    (
        '33333333-3333-3333-3333-333333333333',
        'zen',
        'http://127.0.0.1:54321/storage/v1/object/public/avatars/zen.webp',
        'Competitive grinder. Chasing consistency and route memory optimization.',
        'US',
        '2025-06-14',
        2010,
        'New Mapper',
        false,
        array ['HD', 'DT'],
        now()
    ),
    (
        '44444444-4444-4444-4444-444444444444',
        'lyra',
        'http://127.0.0.1:54321/storage/v1/object/public/avatars/lyra.webp',
        'Top-rank consistency player focused on dense rhythm control.',
        'DE',
        '2025-08-04',
        1562,
        'New Mapper',
        false,
        array ['NM', 'HR'],
        now()
    ) on conflict (id) do
update
set username = excluded.username,
    avatar_url = excluded.avatar_url,
    bio = excluded.bio,
    country = excluded.country,
    joined_at = excluded.joined_at,
    follower_count = excluded.follower_count,
    mapper_tier = excluded.mapper_tier,
    is_admin = excluded.is_admin,
    favorite_mods = excluded.favorite_mods,
    updated_at = now();
insert into public.profile_stats (
        profile_id,
        total_pp,
        ranked_score,
        maps_cleared,
        global_rank,
        country_rank,
        updated_at
    )
values (
        '33333333-3333-3333-3333-333333333333',
        15432,
        942118211,
        938,
        1,
        1,
        now()
    ),
    (
        '44444444-4444-4444-4444-444444444444',
        15112,
        903401833,
        887,
        2,
        1,
        now()
    ),
    (
        '11111111-1111-1111-1111-111111111111',
        14621,
        841221985,
        812,
        4,
        1,
        now()
    ),
    (
        '22222222-2222-2222-2222-222222222222',
        11220,
        501222904,
        602,
        19,
        1,
        now()
    ) on conflict (profile_id) do
update
set total_pp = excluded.total_pp,
    ranked_score = excluded.ranked_score,
    maps_cleared = excluded.maps_cleared,
    global_rank = excluded.global_rank,
    country_rank = excluded.country_rank,
    updated_at = now();
insert into public.profile_rank_pp_snapshots (
        profile_id,
        snapshot_date,
        global_rank,
        country_rank,
        total_pp
    )
values -- Zen: Recent rank climb/stability
    (
        '33333333-3333-3333-3333-333333333333',
        (now() - interval '6 days')::date,
        1,
        1,
        15400
    ),
    (
        '33333333-3333-3333-3333-333333333333',
        (now() - interval '5 days')::date,
        1,
        1,
        15410
    ),
    (
        '33333333-3333-3333-3333-333333333333',
        (now() - interval '4 days')::date,
        2,
        1,
        15410
    ),
    (
        '33333333-3333-3333-3333-333333333333',
        (now() - interval '3 days')::date,
        2,
        1,
        15420
    ),
    (
        '33333333-3333-3333-3333-333333333333',
        (now() - interval '2 days')::date,
        1,
        1,
        15432
    ),
    (
        '33333333-3333-3333-3333-333333333333',
        (now() - interval '1 days')::date,
        1,
        1,
        15432
    ),
    (
        '33333333-3333-3333-3333-333333333333',
        now()::date,
        1,
        1,
        15432
    ),
    -- Lyra: Brief period at rank 1
    (
        '44444444-4444-4444-4444-444444444444',
        (now() - interval '6 days')::date,
        2,
        1,
        15050
    ),
    (
        '44444444-4444-4444-4444-444444444444',
        (now() - interval '5 days')::date,
        2,
        1,
        15060
    ),
    (
        '44444444-4444-4444-4444-444444444444',
        (now() - interval '4 days')::date,
        1,
        1,
        15112
    ),
    (
        '44444444-4444-4444-4444-444444444444',
        (now() - interval '3 days')::date,
        1,
        1,
        15112
    ),
    (
        '44444444-4444-4444-4444-444444444444',
        (now() - interval '2 days')::date,
        2,
        1,
        15112
    ),
    (
        '44444444-4444-4444-4444-444444444444',
        (now() - interval '1 days')::date,
        2,
        1,
        15112
    ),
    (
        '44444444-4444-4444-4444-444444444444',
        now()::date,
        2,
        1,
        15112
    ),
    -- Nova: Consistent top 5
    (
        '11111111-1111-1111-1111-111111111111',
        (now() - interval '6 days')::date,
        4,
        1,
        14600
    ),
    (
        '11111111-1111-1111-1111-111111111111',
        (now() - interval '3 days')::date,
        5,
        1,
        14600
    ),
    (
        '11111111-1111-1111-1111-111111111111',
        now()::date,
        4,
        1,
        14621
    ),
    -- Iris: Steady progress
    (
        '22222222-2222-2222-2222-222222222222',
        (now() - interval '6 days')::date,
        20,
        1,
        11000
    ),
    (
        '22222222-2222-2222-2222-222222222222',
        (now() - interval '1 days')::date,
        19,
        1,
        11220
    ),
    (
        '22222222-2222-2222-2222-222222222222',
        now()::date,
        19,
        1,
        11220
    ) on conflict (profile_id, snapshot_date) do
update
set global_rank = excluded.global_rank,
    country_rank = excluded.country_rank,
    total_pp = excluded.total_pp;
insert into public.beatmaps (
        id,
        name,
        title,
        artist,
        mapper_id,
        description,
        audio_url,
        data_url,
        status,
        plays,
        downloads,
        likes,
        length_seconds,
        bpm,
        nominator_stars,
        community_stars,
        created_at,
        updated_at,
        ranked_at
    )
values (
        1,
        'Railbreaker',
        'Railbreaker',
        'Haru Komaki',
        '11111111-1111-1111-1111-111111111111',
        'Fast lane changes with narrow wall patterns built around vocal chops and kick accents.',
        'https://example.com/audio/railbreaker.mp3',
        'https://example.com/data/railbreaker.json',
        'RANKED',
        68021,
        10234,
        1234,
        148,
        174,
        5.80,
        5.40,
        '2026-02-02',
        now(),
        '2026-02-20'
    ),
    (
        2,
        'Neon World',
        'Neon Archive',
        'Yume Circuit',
        '22222222-2222-2222-2222-222222222222',
        'A scenic route map featuring ramps, moving hazards, and layered rhythm cues.',
        'https://example.com/audio/neon-archive.mp3',
        'https://example.com/data/neon-archive.json',
        'UNRANKED',
        29911,
        5190,
        496,
        185,
        160,
        4.30,
        4.70,
        '2026-02-14',
        now(),
        null
    ),
    (
        3,
        'The First Light',
        'First Light',
        'egakareta',
        '33333333-3333-3333-3333-333333333333',
        'Core mechanics tutorial introducing turns, speed portals, and dash orbs.',
        'https://example.com/audio/first-light.mp3',
        'https://example.com/data/first-light.json',
        'OFFICIAL',
        110341,
        23540,
        5006,
        112,
        142,
        2.10,
        2.00,
        '2026-01-21',
        now(),
        '2026-01-30'
    );

select setval(
        pg_get_serial_sequence('public.beatmaps', 'id'),
        coalesce((select max(id) from public.beatmaps), 1),
        true
    );

insert into public.beatmap_tags (beatmap_id, tag)
values (1, 'speed'),
    (1, 'precision'),
    (1, 'sync'),
    (2, 'flow'),
    (2, 'showcase'),
    (2, 'diff-spike'),
    (3, 'tutorial'),
    (3, 'intro'),
    (3, 'official') on conflict (beatmap_id, tag) do nothing;
insert into public.beatmap_scores (
        beatmap_id,
        profile_id,
        player_handle,
        score,
        pp,
        mods,
        played_at
    )
values (
        1,
        '33333333-3333-3333-3333-333333333333',
        'zen',
        1254870,
        312,
        array ['HD'],
        '2026-02-21'
    ),
    (
        1,
        '44444444-4444-4444-4444-444444444444',
        'lyra',
        1249034,
        307,
        array []::text [],
        '2026-02-20'
    ),
    (
        1,
        null,
        'atlas',
        1242110,
        301,
        array ['HR'],
        '2026-02-19'
    ),
    (
        2,
        null,
        'kairo',
        1132222,
        264,
        array []::text [],
        '2026-02-22'
    ),
    (
        2,
        null,
        'mira',
        1120100,
        252,
        array ['EZ'],
        '2026-02-21'
    ),
    (
        2,
        null,
        'taro',
        1113803,
        243,
        array []::text [],
        '2026-02-20'
    ),
    (
        3,
        '11111111-1111-1111-1111-111111111111',
        'nova',
        980000,
        58,
        array []::text [],
        '2026-02-16'
    ),
    (
        3,
        null,
        'ray',
        976100,
        56,
        array []::text [],
        '2026-02-16'
    ),
    (
        3,
        null,
        'sol',
        970820,
        53,
        array ['DT'],
        '2026-02-17'
    ),
    (
        1,
        '33333333-3333-3333-3333-333333333333',
        'zen',
        1242500,
        298,
        array ['HD', 'DT'],
        '2026-02-18'
    ),
    (
        2,
        '33333333-3333-3333-3333-333333333333',
        'zen',
        1129500,
        278,
        array ['DT'],
        '2026-02-23'
    ),
    (
        3,
        '33333333-3333-3333-3333-333333333333',
        'zen',
        975430,
        91,
        array ['HD'],
        '2026-02-24'
    ),
    (
        1,
        '44444444-4444-4444-4444-444444444444',
        'lyra',
        1238800,
        289,
        array ['HR'],
        '2026-02-19'
    ),
    (
        2,
        '44444444-4444-4444-4444-444444444444',
        'lyra',
        1126400,
        273,
        array ['HD'],
        '2026-02-23'
    ),
    (
        3,
        '44444444-4444-4444-4444-444444444444',
        'lyra',
        973920,
        86,
        array []::text [],
        '2026-02-25'
    ),
    (
        1,
        '11111111-1111-1111-1111-111111111111',
        'nova',
        1221040,
        271,
        array ['HD'],
        '2026-02-22'
    ),
    (
        2,
        '11111111-1111-1111-1111-111111111111',
        'nova',
        1118200,
        248,
        array []::text [],
        '2026-02-20'
    ),
    (
        3,
        '11111111-1111-1111-1111-111111111111',
        'nova',
        982210,
        63,
        array ['HD', 'HR'],
        '2026-02-27'
    ),
    (
        1,
        '22222222-2222-2222-2222-222222222222',
        'iris',
        1219000,
        255,
        array ['EZ'],
        '2026-02-20'
    ),
    (
        2,
        '22222222-2222-2222-2222-222222222222',
        'iris',
        1117300,
        241,
        array []::text [],
        '2026-02-22'
    ),
    (
        3,
        '22222222-2222-2222-2222-222222222222',
        'iris',
        968020,
        59,
        array ['EZ', 'HD'],
        '2026-02-26'
    );
insert into public.comments (
        id,
        resource_type,
        resource_id,
        profile_id,
        body,
        votes,
        created_at
    )
values (
        'c1111111-1111-1111-1111-111111111111',
        'beatmap',
        '1',
        '44444444-4444-4444-4444-444444444444',
        'Great sync charting. Last drop could use one more visual warning before the rapid split.',
        18,
        '2026-02-21'
    ),
    (
        'c2222222-2222-2222-2222-222222222222',
        'beatmap',
        '1',
        null,
        'Nominator stars feel fair; community vote might settle around 5.5.',
        11,
        '2026-02-20'
    ),
    (
        'c3333333-3333-3333-3333-333333333333',
        'beatmap',
        '2',
        null,
        'Loved the camera-friendly route design. One corner after chorus needs clearer telegraphing.',
        9,
        '2026-02-22'
    ),
    (
        'c4444444-4444-4444-4444-444444444444',
        'beatmap',
        '3',
        null,
        'Perfect tutorial pacing. I finally understood dash orb timing.',
        24,
        '2026-02-18'
    ) on conflict (id) do
update
set body = excluded.body,
    votes = excluded.votes,
    created_at = excluded.created_at;

insert into public.comments (
        id,
        resource_type,
        resource_id,
        profile_id,
        parent_id,
        body,
        votes,
        created_at
    )
values (
        'c5555555-5555-5555-5555-555555555555',
        'beatmap',
        '1',
        '11111111-1111-1111-1111-111111111111',
        null,
        'Replay readability is excellent. The pre-chorus lane swap is my favorite section.',
        14,
        '2026-02-23'
    ),
    (
        'c6666666-6666-6666-6666-666666666666',
        'beatmap',
        '1',
        '22222222-2222-2222-2222-222222222222',
        'c5555555-5555-5555-5555-555555555555',
        'Agreed. That lane swap teaches the rhythm pattern without feeling forced.',
        7,
        '2026-02-23'
    ),
    (
        'c7777777-7777-7777-7777-777777777777',
        'beatmap',
        '2',
        '33333333-3333-3333-3333-333333333333',
        null,
        'The ending stream is slightly overtuned for this star rating, but still fun.',
        6,
        '2026-02-24'
    ),
    (
        'c8888888-8888-8888-8888-888888888888',
        'beatmap',
        '3',
        '11111111-1111-1111-1111-111111111111',
        null,
        'Clean beginner map. Maybe add one extra visual cue before the first jump.',
        16,
        '2026-02-25'
    ),
    (
        'c9999999-9999-9999-9999-999999999999',
        'news_post',
        'the-first-post',
        '44444444-4444-4444-4444-444444444444',
        null,
        'Great kickoff post. Looking forward to weekly ranked highlights.',
        10,
        '2026-02-26'
    ),
    (
        'caaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa',
        'news_post',
        'the-first-post',
        '22222222-2222-2222-2222-222222222222',
        'c9999999-9999-9999-9999-999999999999',
        'Same here. A short dev diary section each week would be awesome too.',
        8,
        '2026-02-26'
    ),
    (
        'cbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb',
        'news_post',
        'the-first-post',
        null,
        null,
        'Love the direction of the project. Thanks for sharing roadmap details.',
        5,
        '2026-02-27'
    ),
    (
        'cccccccc-cccc-cccc-cccc-cccccccccccc',
        'news_post',
        'the-first-post',
        '33333333-3333-3333-3333-333333333333',
        null,
        'Could we get a community Q&A post next month? That would be helpful.',
        4,
        '2026-02-28'
    ) on conflict (id) do
update
set parent_id = excluded.parent_id,
    body = excluded.body,
    votes = excluded.votes,
    created_at = excluded.created_at;

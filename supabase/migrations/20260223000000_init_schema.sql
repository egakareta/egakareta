-- Create a table for public profiles
create table profiles (
  id uuid references auth.users on delete cascade not null primary key,
  updated_at timestamp with time zone,
  username text unique,
  full_name text,
  avatar_url text,
  website text,

  constraint username_length check (char_length(username) >= 3)
);

-- Set up Row Level Security (RLS)
-- See https://supabase.com/docs/guides/auth/row-level-security for more details.
alter table profiles enable row level security;

create policy "Public profiles are viewable by everyone." on profiles
  for select using (true);

create policy "Users can insert their own profile." on profiles
  for insert with check (auth.uid() = id);

create policy "Users can update own profile." on profiles
  for update using (auth.uid() = id);

-- This triggers a profile creation on signup
create function public.handle_new_user()
returns trigger as $$
begin
  insert into public.profiles (id, full_name, avatar_url, username)
  values (new.id, new.raw_user_meta_data->>'full_name', new.raw_user_meta_data->>'avatar_url', new.raw_user_meta_data->>'username');
  return new;
end;
$$ language plpgsql security definer;

create trigger on_auth_user_created
  after insert on auth.users
  for each row execute procedure public.handle_new_user();

-- Create a table for beatmaps
create table beatmaps (
  id uuid default gen_random_uuid() primary key,
  created_at timestamp with time zone default timezone('utc'::text, now()) not null,
  updated_at timestamp with time zone default timezone('utc'::text, now()) not null,
  title text not null,
  artist text not null,
  mapper_id uuid references profiles(id) not null,
  description text,
  audio_url text not null, -- URL to storage
  data_url text not null, -- URL to storage (json content)
  status text default 'UNRANKED' check (status in ('UNRANKED', 'RANKED', 'OFFICIAL')),
  difficulty_stars numeric(4, 2) default 0,
  plays integer default 0,
  downloads integer default 0
);

alter table beatmaps enable row level security;

create policy "Beatmaps are viewable by everyone." on beatmaps
  for select using (true);

create policy "Users can upload their own beatmaps." on beatmaps
  for insert with check (auth.uid() = mapper_id);

create policy "Users can update their own beatmaps." on beatmaps
  for update using (auth.uid() = mapper_id);

-- Storage buckets
insert into storage.buckets (id, name, public) values ('beatmap-audio', 'beatmap-audio', true);
insert into storage.buckets (id, name, public) values ('beatmap-data', 'beatmap-data', true);

create policy "Audio is accessible by public" on storage.objects for select using (bucket_id = 'beatmap-audio');
create policy "Data is accessible by public" on storage.objects for select using (bucket_id = 'beatmap-data');

create policy "Users can upload audio" on storage.objects for insert with check (bucket_id = 'beatmap-audio' and auth.uid() = owner);
create policy "Users can upload data" on storage.objects for insert with check (bucket_id = 'beatmap-data' and auth.uid() = owner);

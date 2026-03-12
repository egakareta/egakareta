-- Create a bucket for avatars
insert into storage.buckets (id, name, public)
values ('avatars', 'avatars', true) on conflict (id) do nothing;
-- Allow public access to avatars
create policy "Avatar images are publicly accessible." on storage.objects for
select using (bucket_id = 'avatars');
-- Allow users to upload their own avatar
-- We'll store avatars as `avatars/<user_id>/<filename>`
create policy "Users can upload their own avatar." on storage.objects for
insert with check (
        bucket_id = 'avatars'
        and (
            select auth.uid()
        )::text = (storage.foldername(name)) [1]
    );
-- Allow users to update their own avatar
create policy "Users can update their own avatar." on storage.objects for
update using (
        bucket_id = 'avatars'
        and (
            select auth.uid()
        )::text = (storage.foldername(name)) [1]
    );
-- Allow users to delete their own avatar
create policy "Users can delete their own avatar." on storage.objects for delete using (
    bucket_id = 'avatars'
    and (
        select auth.uid()
    )::text = (storage.foldername(name)) [1]
);
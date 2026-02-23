import Link from "next/link";
import { notFound } from "next/navigation";
import { beatmaps, getProfile, ppLeaderboard, profiles } from "@/lib/mock-data";

type ProfilePageProps = {
    params: Promise<{ handle: string }>;
};

export function generateStaticParams() {
    return profiles.map((profile) => ({ handle: profile.handle }));
}

export default async function ProfilePage({ params }: ProfilePageProps) {
    const { handle } = await params;
    const profile = getProfile(handle);

    if (!profile) {
        notFound();
    }

    const publishedMaps = beatmaps.filter(
        (beatmap) => beatmap.mapperHandle === handle,
    );
    const ppRow = ppLeaderboard.find((entry) => entry.handle === handle);

    return (
        <main className="mx-auto w-[min(980px,calc(100%-1.3rem))] py-8 sm:w-[min(980px,calc(100%-2.4rem))] sm:py-14">
            <section className="rounded-2xl border border-sky-200/25 bg-slate-950/70 p-5 sm:p-8">
                <p className="text-xs font-bold tracking-[0.14em] text-cyan-300 uppercase">
                    Player Profile
                </p>
                <h1 className="font-display mt-2 text-3xl font-semibold sm:text-4xl">
                    {profile.displayName}
                </h1>
                <p className="mt-3 text-slate-300">{profile.bio}</p>

                <div className="mt-5 grid gap-3 sm:grid-cols-2">
                    <div className="rounded-xl border border-cyan-100/20 bg-slate-900/65 p-4">
                        <p className="text-sm text-slate-400">Handle</p>
                        <p className="font-semibold">@{profile.handle}</p>
                        <p className="mt-2 text-sm text-slate-400">Country</p>
                        <p className="font-semibold">{profile.country}</p>
                        <p className="mt-2 text-sm text-slate-400">Joined</p>
                        <p className="font-semibold">{profile.joinedAt}</p>
                    </div>
                    <div className="rounded-xl border border-cyan-100/20 bg-slate-900/65 p-4">
                        <p className="text-sm text-slate-400">Mapper tier</p>
                        <p className="font-semibold">{profile.mapperTier}</p>
                        <p className="mt-2 text-sm text-slate-400">Followers</p>
                        <p className="font-semibold">
                            {profile.followerCount.toLocaleString()}
                        </p>
                        <p className="mt-2 text-sm text-slate-400">
                            Favorite mods
                        </p>
                        <p className="font-semibold">
                            {profile.favoriteMods.join(" · ")}
                        </p>
                    </div>
                </div>

                <div className="mt-5 rounded-xl border border-cyan-100/20 bg-slate-900/65 p-4">
                    <h2 className="font-display text-xl font-semibold">
                        Competitive snapshot
                    </h2>
                    <p className="mt-2 text-slate-300">
                        {ppRow
                            ? `Global #${ppRow.rank} with ${ppRow.pp.toLocaleString()} PP.`
                            : `Global #${profile.globalRank} with ${profile.totalPp.toLocaleString()} PP.`}
                    </p>
                    <Link
                        href="/leaderboards/pp"
                        className="mt-3 inline-block text-cyan-300 hover:text-cyan-200"
                    >
                        Open PP leaderboard
                    </Link>
                </div>

                <div className="mt-5 rounded-xl border border-cyan-100/20 bg-slate-900/65 p-4">
                    <h2 className="font-display text-xl font-semibold">
                        Published beatmaps
                    </h2>
                    {publishedMaps.length === 0 ? (
                        <p className="mt-2 text-slate-400">
                            No published maps yet.
                        </p>
                    ) : (
                        <ul className="mt-3 grid gap-2">
                            {publishedMaps.map((beatmap) => (
                                <li
                                    key={beatmap.slug}
                                    className="flex flex-wrap items-center justify-between gap-3 rounded-lg border border-cyan-100/15 px-3 py-2"
                                >
                                    <div>
                                        <p className="font-semibold">
                                            {beatmap.title}
                                        </p>
                                        <p className="text-sm text-slate-400">
                                            {beatmap.length} · ★
                                            {beatmap.nominatorStars.toFixed(1)}{" "}
                                            / ★
                                            {beatmap.communityStars.toFixed(1)}
                                        </p>
                                    </div>
                                    <Link
                                        href={`/beatmaps/${beatmap.slug}`}
                                        className="text-sm text-cyan-300"
                                    >
                                        Open
                                    </Link>
                                </li>
                            ))}
                        </ul>
                    )}
                </div>
            </section>
        </main>
    );
}

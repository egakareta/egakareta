import Link from "next/link";
import { notFound } from "next/navigation";
import { beatmaps, getBeatmap, mapLeaderboards } from "@/lib/mock-data";

type BeatmapLeaderboardPageProps = {
    params: Promise<{ slug: string }>;
};

export function generateStaticParams() {
    return beatmaps.map((beatmap) => ({ slug: beatmap.slug }));
}

export default async function BeatmapLeaderboardPage({
    params,
}: BeatmapLeaderboardPageProps) {
    const { slug } = await params;
    const beatmap = getBeatmap(slug);

    if (!beatmap) {
        notFound();
    }

    const entries = mapLeaderboards[slug] ?? [];

    return (
        <main className="mx-auto w-[min(980px,calc(100%-1.3rem))] py-8 sm:w-[min(980px,calc(100%-2.4rem))] sm:py-14">
            <section className="rounded-2xl border border-sky-200/25 bg-slate-950/70 p-5 sm:p-8">
                <p className="text-xs font-bold tracking-[0.14em] text-cyan-300 uppercase">
                    Beatmap Leaderboard
                </p>
                <h1 className="font-display mt-2 text-3xl font-semibold sm:text-4xl">
                    {beatmap.title}
                </h1>
                <p className="mt-3 text-slate-300">
                    Top scores for this map. PP shown reflects current formula
                    snapshot.
                </p>

                <div className="mt-6 overflow-x-auto rounded-xl border border-sky-100/15">
                    <table className="min-w-full text-left text-sm">
                        <thead className="bg-slate-900/90 text-slate-300">
                            <tr>
                                <th className="px-4 py-3">#</th>
                                <th className="px-4 py-3">Player</th>
                                <th className="px-4 py-3">Score</th>
                                <th className="px-4 py-3">PP</th>
                                <th className="px-4 py-3">Mods</th>
                                <th className="px-4 py-3">Played</th>
                            </tr>
                        </thead>
                        <tbody>
                            {entries.map((entry) => (
                                <tr
                                    key={`${slug}-${entry.rank}-${entry.playerHandle}`}
                                    className="border-t border-sky-100/10 bg-slate-950/80"
                                >
                                    <td className="px-4 py-3 font-semibold text-cyan-300">
                                        {entry.rank}
                                    </td>
                                    <td className="px-4 py-3">
                                        <Link
                                            href={`/profiles/${entry.playerHandle}`}
                                            className="hover:text-cyan-300"
                                        >
                                            {entry.playerHandle}
                                        </Link>
                                    </td>
                                    <td className="px-4 py-3">{entry.score}</td>
                                    <td className="px-4 py-3">{entry.pp}</td>
                                    <td className="px-4 py-3">{entry.mods}</td>
                                    <td className="px-4 py-3 text-slate-400">
                                        {entry.playedAt}
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>

                <div className="mt-5 flex flex-wrap gap-3 text-sm">
                    <Link
                        href={`/beatmaps/${beatmap.slug}/comments`}
                        className="rounded-full border border-cyan-200/35 px-4 py-2 text-cyan-200"
                    >
                        View comments
                    </Link>
                    <Link
                        href={`/beatmaps/${beatmap.slug}`}
                        className="rounded-full border border-cyan-200/20 px-4 py-2 text-slate-300"
                    >
                        Back to map details
                    </Link>
                </div>
            </section>
        </main>
    );
}

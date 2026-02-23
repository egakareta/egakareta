import Link from "next/link";
import { notFound } from "next/navigation";
import { beatmaps, getBeatmap } from "@/lib/mock-data";

type BeatmapPageProps = {
    params: Promise<{ slug: string }>;
};

export function generateStaticParams() {
    return beatmaps.map((beatmap) => ({ slug: beatmap.slug }));
}

export default async function BeatmapDetailPage({ params }: BeatmapPageProps) {
    const { slug } = await params;
    const beatmap = getBeatmap(slug);

    if (!beatmap) {
        notFound();
    }

    return (
        <main className="mx-auto w-[min(980px,calc(100%-1.3rem))] py-8 sm:w-[min(980px,calc(100%-2.4rem))] sm:py-14">
            <section className="rounded-2xl border border-sky-200/25 bg-slate-950/70 p-5 sm:p-8">
                <p className="text-xs font-bold tracking-[0.14em] text-cyan-300 uppercase">
                    Beatmap Details
                </p>
                <h1 className="font-display mt-2 text-3xl font-semibold sm:text-5xl">
                    {beatmap.title}
                </h1>
                <p className="mt-3 text-slate-300">
                    {beatmap.artist} · mapped by{" "}
                    <Link
                        href={`/profiles/${beatmap.mapperHandle}`}
                        className="text-cyan-300"
                    >
                        {beatmap.mapper}
                    </Link>
                </p>

                <p className="mt-4 text-slate-300">{beatmap.description}</p>

                <div className="mt-5 grid gap-3 sm:grid-cols-2">
                    <div className="rounded-xl border border-cyan-100/20 bg-slate-900/65 p-4">
                        <h2 className="font-display text-lg font-semibold">
                            Difficulty model
                        </h2>
                        <p className="mt-2 text-sm text-slate-300">
                            Nominator: ★{beatmap.nominatorStars.toFixed(1)}
                        </p>
                        <p className="mt-1 text-sm text-slate-300">
                            Community: ★{beatmap.communityStars.toFixed(1)}
                        </p>
                        <p className="mt-2 text-sm text-slate-400">
                            Combined difficulty feeds ranked eligibility and PP
                            weighting.
                        </p>
                    </div>

                    <div className="rounded-xl border border-cyan-100/20 bg-slate-900/65 p-4">
                        <h2 className="font-display text-lg font-semibold">
                            Map metadata
                        </h2>
                        <p className="mt-2 text-sm text-slate-300">
                            Length: {beatmap.length}
                        </p>
                        <p className="mt-1 text-sm text-slate-300">
                            BPM: {beatmap.bpm}
                        </p>
                        <p className="mt-1 text-sm text-slate-300">
                            Uploaded: {beatmap.uploadedAt}
                        </p>
                        <p className="mt-1 text-sm text-slate-300">
                            Downloads: {beatmap.downloads.toLocaleString()}
                        </p>
                    </div>
                </div>

                <div className="mt-5 flex flex-wrap gap-3">
                    <Link
                        href={`/beatmaps/${beatmap.slug}/leaderboard`}
                        className="rounded-full bg-linear-to-r from-cyan-300 to-sky-200 px-4 py-2.5 text-sm font-bold text-sky-950"
                    >
                        View leaderboard
                    </Link>
                    <Link
                        href={`/beatmaps/${beatmap.slug}/comments`}
                        className="rounded-full border border-cyan-200/40 px-4 py-2.5 text-sm font-bold text-cyan-100"
                    >
                        Open comments
                    </Link>
                    <Link
                        href="/beatmaps"
                        className="rounded-full border border-cyan-200/20 px-4 py-2.5 text-sm text-slate-300"
                    >
                        Back to browse
                    </Link>
                </div>
            </section>
        </main>
    );
}

import Link from "next/link";
import { beatmaps } from "@/lib/mock-data";

function statusStyle(status: "Unranked" | "Ranked" | "Official") {
    if (status === "Ranked") {
        return "border-emerald-400/50 bg-emerald-400/10 text-emerald-300";
    }
    if (status === "Official") {
        return "border-amber-300/50 bg-amber-300/10 text-amber-200";
    }
    return "border-sky-300/40 bg-sky-300/10 text-sky-200";
}

export default function BeatmapsPage() {
    return (
        <main className="mx-auto w-[min(1100px,calc(100%-1.3rem))] py-8 sm:w-[min(1100px,calc(100%-2.4rem))] sm:py-14">
            <section className="rounded-2xl border border-sky-200/25 bg-slate-950/70 p-5 sm:p-8">
                <p className="text-xs font-bold tracking-[0.14em] text-cyan-300 uppercase">
                    Community Maps
                </p>
                <h1 className="font-display mt-2 text-3xl font-semibold sm:text-4xl">
                    Beatmap browse
                </h1>
                <p className="mt-3 text-slate-300">
                    Sort by popularity, difficulty, date, and map status. This
                    is the Phase 3 discovery hub.
                </p>

                <div className="mt-6 grid gap-3 rounded-xl border border-sky-200/20 bg-slate-950/70 p-4 sm:grid-cols-4">
                    {[
                        "Sort: Popular",
                        "Difficulty: Any",
                        "Length: Any",
                        "Tags: All",
                    ].map((filter) => (
                        <div
                            key={filter}
                            className="rounded-lg border border-cyan-100/20 bg-slate-900/75 px-3 py-2 text-sm text-slate-300"
                        >
                            {filter}
                        </div>
                    ))}
                </div>

                <div className="mt-5 grid gap-3">
                    {beatmaps.map((beatmap) => (
                        <article
                            key={beatmap.slug}
                            className="rounded-xl border border-cyan-100/20 bg-slate-950/85 p-4"
                        >
                            <div className="flex flex-wrap items-start justify-between gap-3">
                                <div>
                                    <h2 className="font-display text-2xl font-semibold">
                                        {beatmap.title}
                                    </h2>
                                    <p className="text-sm text-slate-300">
                                        {beatmap.artist} · mapped by{" "}
                                        <Link
                                            href={`/profiles/${beatmap.mapperHandle}`}
                                            className="text-cyan-300"
                                        >
                                            {beatmap.mapper}
                                        </Link>
                                    </p>
                                </div>
                                <span
                                    className={`rounded-full border px-3 py-1 text-xs font-semibold ${statusStyle(beatmap.status)}`}
                                >
                                    {beatmap.status}
                                </span>
                            </div>

                            <p className="mt-3 text-slate-300">
                                {beatmap.description}
                            </p>

                            <div className="mt-3 flex flex-wrap gap-2">
                                {beatmap.tags.map((tag) => (
                                    <span
                                        key={`${beatmap.slug}-${tag}`}
                                        className="rounded-full border border-sky-200/20 px-2.5 py-1 text-xs text-slate-200"
                                    >
                                        {tag}
                                    </span>
                                ))}
                            </div>

                            <div className="mt-4 flex flex-wrap items-center justify-between gap-3 border-t border-sky-100/10 pt-3 text-sm text-slate-300">
                                <p>
                                    {beatmap.length} · {beatmap.bpm} BPM · ★
                                    {beatmap.nominatorStars.toFixed(1)} / ★
                                    {beatmap.communityStars.toFixed(1)}
                                </p>
                                <div className="flex gap-4">
                                    <span>
                                        {beatmap.downloads.toLocaleString()} dl
                                    </span>
                                    <span>
                                        {beatmap.rating.toFixed(1)} / 5.0
                                    </span>
                                    <Link
                                        href={`/beatmaps/${beatmap.slug}`}
                                        className="font-semibold text-cyan-300 hover:text-cyan-200"
                                    >
                                        View details
                                    </Link>
                                </div>
                            </div>
                        </article>
                    ))}
                </div>
            </section>
        </main>
    );
}

import Link from "next/link";

export default function LeaderboardsIndexPage() {
    return (
        <main className="mx-auto w-[min(900px,calc(100%-1.3rem))] py-8 sm:w-[min(900px,calc(100%-2.4rem))] sm:py-14">
            <section className="rounded-2xl border border-sky-200/25 bg-slate-950/70 p-5 sm:p-8">
                <p className="text-xs font-bold tracking-[0.14em] text-cyan-300 uppercase">
                    Leaderboards
                </p>
                <h1 className="font-display mt-2 text-3xl font-semibold sm:text-4xl">
                    Competitive dashboards
                </h1>
                <div className="mt-5 grid gap-3 sm:grid-cols-2">
                    <Link
                        href="/leaderboards/pp"
                        className="rounded-xl border border-cyan-100/20 bg-slate-900/70 p-4"
                    >
                        <h2 className="font-display text-xl font-semibold">
                            PP Leaderboard
                        </h2>
                        <p className="mt-2 text-slate-300">
                            Global performance progression across ranked maps.
                        </p>
                    </Link>
                    <Link
                        href="/beatmaps/railbreaker/leaderboard"
                        className="rounded-xl border border-cyan-100/20 bg-slate-900/70 p-4"
                    >
                        <h2 className="font-display text-xl font-semibold">
                            Beatmap Leaderboard
                        </h2>
                        <p className="mt-2 text-slate-300">
                            Per-map score race with accuracy and mods.
                        </p>
                    </Link>
                </div>
            </section>
        </main>
    );
}

import Link from "next/link";
import { ppLeaderboard } from "@/lib/mock-data";

export default function PpLeaderboardPage() {
    return (
        <main className="mx-auto w-[min(1100px,calc(100%-1.3rem))] py-8 sm:w-[min(1100px,calc(100%-2.4rem))] sm:py-14">
            <section className="rounded-2xl border border-sky-200/25 bg-slate-950/70 p-5 sm:p-8">
                <p className="text-xs font-bold tracking-[0.14em] text-cyan-300 uppercase">
                    Competitive
                </p>
                <h1 className="font-display mt-2 text-3xl font-semibold sm:text-4xl">
                    Global PP leaderboard
                </h1>
                <p className="mt-3 text-slate-300">
                    Performance points are derived from ranked map difficulty,
                    score quality, and active mods.
                </p>

                <div className="mt-6 overflow-x-auto rounded-xl border border-sky-100/15">
                    <table className="min-w-full text-left text-sm">
                        <thead className="bg-slate-900/90 text-slate-300">
                            <tr>
                                <th className="px-4 py-3">#</th>
                                <th className="px-4 py-3">Player</th>
                                <th className="px-4 py-3">Country</th>
                                <th className="px-4 py-3">PP</th>
                                <th className="px-4 py-3">Accuracy</th>
                                <th className="px-4 py-3">Ranked Score</th>
                                <th className="px-4 py-3">Clears</th>
                            </tr>
                        </thead>
                        <tbody>
                            {ppLeaderboard.map((entry) => (
                                <tr
                                    key={`pp-${entry.rank}-${entry.handle}`}
                                    className="border-t border-sky-100/10 bg-slate-950/80"
                                >
                                    <td className="px-4 py-3 font-semibold text-cyan-300">
                                        {entry.rank}
                                    </td>
                                    <td className="px-4 py-3">
                                        <Link
                                            href={`/profiles/${entry.handle}`}
                                            className="hover:text-cyan-300"
                                        >
                                            {entry.handle}
                                        </Link>
                                    </td>
                                    <td className="px-4 py-3">
                                        {entry.country}
                                    </td>
                                    <td className="px-4 py-3 font-semibold text-cyan-200">
                                        {entry.pp.toLocaleString()}
                                    </td>
                                    <td className="px-4 py-3">
                                        {entry.accuracy}
                                    </td>
                                    <td className="px-4 py-3">
                                        {entry.rankedScore}
                                    </td>
                                    <td className="px-4 py-3">
                                        {entry.mapsCleared}
                                    </td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                </div>

                <div className="mt-5 text-sm text-slate-400">
                    Formula version:{" "}
                    <span className="text-slate-200">v0.9.3</span>. This is
                    expected to evolve as Phase 4 calibrates ranked balance.
                </div>

                <div className="mt-5">
                    <Link
                        href="/beatmaps"
                        className="text-cyan-300 hover:text-cyan-200"
                    >
                        Browse ranked candidates
                    </Link>
                </div>
            </section>
        </main>
    );
}

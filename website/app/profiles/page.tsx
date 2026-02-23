import Link from "next/link";
import { profiles } from "@/lib/mock-data";

export default function ProfilesPage() {
    return (
        <main className="mx-auto w-[min(980px,calc(100%-1.3rem))] py-8 sm:w-[min(980px,calc(100%-2.4rem))] sm:py-14">
            <section className="rounded-2xl border border-sky-200/25 bg-slate-950/70 p-5 sm:p-8">
                <p className="text-xs font-bold tracking-[0.14em] text-cyan-300 uppercase">
                    Community
                </p>
                <h1 className="font-display mt-2 text-3xl font-semibold sm:text-4xl">
                    Player profiles
                </h1>

                <div className="mt-5 grid gap-3 sm:grid-cols-2">
                    {profiles.map((profile) => (
                        <Link
                            key={profile.handle}
                            href={`/profiles/${profile.handle}`}
                            className="rounded-xl border border-cyan-100/20 bg-slate-900/70 p-4"
                        >
                            <h2 className="font-display text-xl font-semibold">
                                {profile.displayName}
                            </h2>
                            <p className="mt-2 text-sm text-slate-300">
                                @{profile.handle} · {profile.country}
                            </p>
                            <p className="mt-2 text-sm text-slate-400">
                                {profile.mapperTier} ·{" "}
                                {profile.totalPp.toLocaleString()} PP
                            </p>
                        </Link>
                    ))}
                </div>
            </section>
        </main>
    );
}

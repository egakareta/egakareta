import Link from "next/link";

const phaseThreeFeatures = [
    {
        title: "Account System",
        description:
            "Sign in with email or Google, or stay in local guest mode and upgrade later.",
        href: "/auth/signup",
    },
    {
        title: "Beatmap Uploads",
        description:
            "Publish maps with metadata, audio files, and direct share links powered by cloud storage.",
        href: "/beatmaps",
    },
    {
        title: "Discover Feed",
        description:
            "Browse by popularity, difficulty, date, and tags to find maps worth grinding.",
        href: "/beatmaps",
    },
    {
        title: "Creator Profiles",
        description:
            "Follow mappers, track releases, and build reputation before ranked curation opens.",
        href: "/profiles/nova",
    },
];

const rankedRoadmap = [
    "Community nomination and quality review flow for ranked candidates.",
    "Dual difficulty display with nominator stars and community average.",
    "Performance points model that evolves with map data and mod usage.",
    "Leaderboards and score submission tuned for competitive consistency.",
];

const pillars = [
    {
        heading: "One Input, High Skill Ceiling",
        copy: "A single click controls 90 degree turns, but precision, route memory, and rhythm mastery separate players.",
    },
    {
        heading: "Editor-First Content",
        copy: "The in-game editor remains the center of the ecosystem: timeline tools, instant playtest, and fast iteration.",
    },
    {
        heading: "Web + Native Reach",
        copy: "Rust and wgpu keep gameplay logic shared across desktop and browser so the community grows in one place.",
    },
];

export default function Home() {
    return (
        <main className="mx-auto grid w-[min(1100px,calc(100%-1.3rem))] gap-5 py-6 sm:w-[min(1100px,calc(100%-2.4rem))] sm:py-14">
            <section className="animate-[rise_650ms_ease-out_both] rounded-2xl border border-sky-200/30 bg-slate-950/65 p-5 backdrop-blur-sm sm:p-9">
                <p className="m-0 text-xs font-bold tracking-[0.14em] text-cyan-300 uppercase">
                    Phase 3 Incoming
                </p>
                <h1 className="font-display mt-3 max-w-[16ch] text-4xl leading-[1.14] font-semibold sm:text-5xl md:text-6xl">
                    Line Dash is moving from a playable engine to a
                    creator-driven rhythm platform.
                </h1>
                <p className="mt-4 max-w-[65ch] text-base leading-7 text-slate-300 sm:text-lg sm:leading-8">
                    We already have a working core game and level editor. Next,
                    we are shipping the online layer: accounts, map sharing,
                    discovery, and creator identity.
                </p>
                <div className="mt-6 flex flex-wrap gap-3.5">
                    <Link
                        href="/beatmaps"
                        className="rounded-full bg-linear-to-r from-cyan-300 to-sky-200 px-4 py-2.5 text-sm font-bold text-sky-950 transition-transform duration-200 hover:-translate-y-0.5"
                    >
                        Browse community beatmaps
                    </Link>
                    <Link
                        href="/auth/signup"
                        className="rounded-full border border-sky-200/35 px-4 py-2.5 text-sm font-bold text-slate-100 transition-transform duration-200 hover:-translate-y-0.5"
                    >
                        Create account
                    </Link>
                </div>
            </section>

            <section
                className="grid gap-4 md:grid-cols-3"
                aria-label="Vision pillars"
            >
                {pillars.map((pillar) => (
                    <article
                        key={pillar.heading}
                        className="animate-[rise_700ms_ease-out_both] rounded-xl border border-cyan-100/20 bg-slate-950/80 p-5"
                    >
                        <h2 className="font-display m-0 text-xl leading-[1.14] font-semibold">
                            {pillar.heading}
                        </h2>
                        <p className="mt-3 text-base leading-7 text-slate-300">
                            {pillar.copy}
                        </p>
                    </article>
                ))}
            </section>

            <section
                id="phase-three"
                className="animate-[rise_650ms_ease-out_both] rounded-2xl border border-sky-200/30 bg-slate-950/65 p-5 backdrop-blur-sm sm:p-8"
            >
                <header>
                    <p className="m-0 text-xs font-bold tracking-[0.14em] text-cyan-300 uppercase">
                        Phase 3 Scope
                    </p>
                    <h2 className="font-display mt-2 text-3xl leading-[1.14] font-semibold sm:text-4xl">
                        Community shipping lane
                    </h2>
                    <p className="mt-4 max-w-[72ch] leading-7 text-slate-300">
                        This release turns local creations into shareable
                        content and prepares the data model for ranking,
                        curation, and long-term progression.
                    </p>
                </header>
                <div className="mt-5 grid gap-3.5 md:grid-cols-2">
                    {phaseThreeFeatures.map((feature) => (
                        <Link
                            key={feature.title}
                            href={feature.href}
                            className="rounded-xl border border-cyan-200/25 bg-slate-950/80 p-4"
                        >
                            <h3 className="font-display m-0 text-lg leading-[1.14] font-semibold">
                                {feature.title}
                            </h3>
                            <p className="mt-2.5 leading-7 text-slate-300">
                                {feature.description}
                            </p>
                        </Link>
                    ))}
                </div>
            </section>

            <section
                id="ranked"
                className="animate-[rise_650ms_ease-out_both] rounded-2xl border border-sky-200/30 bg-slate-950/65 p-5 backdrop-blur-sm sm:p-8"
            >
                <p className="m-0 text-xs font-bold tracking-[0.14em] text-cyan-300 uppercase">
                    After Phase 3
                </p>
                <h2 className="font-display mt-2 text-3xl leading-[1.14] font-semibold sm:text-4xl">
                    Ranked play foundation
                </h2>
                <ul className="mt-4 grid list-disc gap-2.5 pl-5 text-slate-300 marker:text-cyan-500">
                    {rankedRoadmap.map((item) => (
                        <li key={item}>{item}</li>
                    ))}
                </ul>
                <Link
                    href="/leaderboards/pp"
                    className="mt-5 inline-block rounded-full border border-cyan-200/40 px-4 py-2 text-sm font-semibold text-cyan-200 hover:text-cyan-100"
                >
                    View PP leaderboard prototype
                </Link>
            </section>
        </main>
    );
}

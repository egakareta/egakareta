import Link from "next/link";

const features = [
    {
        id: "MOD_01",
        title: "Account System",
        desc: "Persistent progression. Guest mode available.",
        link: "/auth/signup",
        status: "ACTIVE",
    },
    {
        id: "MOD_02",
        title: "Beatmap Cloud",
        desc: "Global database. Direct share links. Metadata sync.",
        link: "/beatmaps",
        status: "ONLINE",
    },
    {
        id: "MOD_03",
        title: "Discovery Feed",
        desc: "Algorithmic sorting. Tag filtration. Difficulty metrics.",
        link: "/beatmaps",
        status: "BETA",
    },
];

export default function Home() {
    return (
        <div className="relative isolate pt-14 dark:bg-slate-950">
            {/* Background Effects */}
            <div
                className="absolute inset-x-0 -top-40 -z-10 transform-gpu overflow-hidden blur-3xl sm:-top-80"
                aria-hidden="true"
            >
                <div className="relative left-[calc(50%-11rem)] aspect-[1155/678] w-[36.125rem] -translate-x-1/2 rotate-[30deg] bg-gradient-to-tr from-cyan-500 to-blue-500 opacity-20 sm:left-[calc(50%-30rem)] sm:w-[72.1875rem]"></div>
            </div>

            <div className="mx-auto max-w-7xl px-6 lg:px-8">
                {/* Hero Section */}
                <div className="mx-auto max-w-3xl py-24 sm:py-32 lg:pt-40">
                    <div className="hidden sm:mb-8 sm:flex sm:justify-center">
                        <div className="relative rounded-full px-3 py-1 text-sm leading-6 text-slate-400 ring-1 ring-cyan-500/20 hover:ring-cyan-500/40 transition-all">
                            <span className="font-mono text-cyan-400">
                                SYS_UPDATE:{" "}
                            </span>{" "}
                            Phase 3 Deployment{" "}
                            <a
                                href="#changelog"
                                className="font-semibold text-cyan-500"
                            >
                                <span
                                    className="absolute inset-0"
                                    aria-hidden="true"
                                ></span>
                                Read more <span aria-hidden="true">&rarr;</span>
                            </a>
                        </div>
                    </div>

                    <div className="text-center">
                        <h1 className="font-display text-5xl font-bold tracking-tight text-white sm:text-7xl uppercase bg-clip-text text-transparent bg-gradient-to-b from-white to-slate-500">
                            Rhythm Engine
                            <span className="block text-cyan-400 text-6xl sm:text-8xl mt-2 tracking-tighter drop-shadow-[0_0_15px_rgba(34,211,238,0.5)]">
                                Line Dash
                            </span>
                        </h1>

                        <p className="mt-6 text-lg leading-8 text-slate-400 max-w-2xl mx-auto border-l-2 border-cyan-500/30 pl-6 text-left font-mono">
                            {">"} INITIALIZING HIGH-PERFORMANCE WEBGL
                            RENDERER...
                            <br />
                            {">"} LOADING COMMUNITY ASSETS...
                            <br />
                            {">"} SYSTEM READY.
                        </p>

                        <div className="mt-10 flex items-center justify-center gap-x-6">
                            <Link
                                href="/play"
                                className="group relative px-8 py-3 bg-cyan-500 text-slate-950 font-bold uppercase tracking-widest hover:bg-cyan-400 transition-all skew-x-[-12deg]"
                            >
                                <span className="block skew-x-[12deg]">
                                    Initiate Sequence
                                </span>
                                <div className="absolute inset-0 border border-white/20 group-hover:scale-105 transition-transform"></div>
                            </Link>
                            <Link
                                href="/about"
                                className="text-sm font-semibold leading-6 text-white uppercase tracking-widest hover:text-cyan-400 transition-colors"
                            >
                                System Specs <span aria-hidden="true">→</span>
                            </Link>
                        </div>
                    </div>
                </div>

                {/* Dashboard Grid */}
                <div className="mx-auto max-w-7xl px-6 lg:px-8 pb-24">
                    <div className="grid grid-cols-1 gap-6 sm:grid-cols-2 lg:grid-cols-3">
                        {features.map((feature) => (
                            <div
                                key={feature.id}
                                className="relative group bg-slate-900/50 border border-slate-800 p-6 hover:border-cyan-500/50 transition-colors overflow-hidden"
                            >
                                {/* Tech Corner Markers */}
                                <div className="absolute top-0 left-0 w-2 h-2 border-t-2 border-l-2 border-slate-700 group-hover:border-cyan-500 transition-colors"></div>
                                <div className="absolute top-0 right-0 w-2 h-2 border-t-2 border-r-2 border-slate-700 group-hover:border-cyan-500 transition-colors"></div>
                                <div className="absolute bottom-0 left-0 w-2 h-2 border-b-2 border-l-2 border-slate-700 group-hover:border-cyan-500 transition-colors"></div>
                                <div className="absolute bottom-0 right-0 w-2 h-2 border-b-2 border-r-2 border-slate-700 group-hover:border-cyan-500 transition-colors"></div>

                                <div className="flex justify-between items-start mb-4">
                                    <span className="font-mono text-xs text-cyan-500/70">
                                        {feature.id}
                                    </span>
                                    <span className="font-mono text-[10px] bg-cyan-950 text-cyan-300 px-2 py-0.5 border border-cyan-900">
                                        {feature.status}
                                    </span>
                                </div>

                                <h3 className="font-display text-xl font-bold text-white mb-2 group-hover:text-cyan-400 transition-colors">
                                    <Link href={feature.link}>
                                        <span className="absolute inset-0"></span>
                                        {feature.title}
                                    </Link>
                                </h3>
                                <p className="text-slate-400 text-sm leading-relaxed font-mono">
                                    {feature.desc}
                                </p>
                            </div>
                        ))}
                    </div>
                </div>
            </div>
        </div>
    );
}

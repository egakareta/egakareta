import Link from "next/link";

const features = [
    {
        id: "MOD_01",
        title: "Account System",
        desc: "Persistent progression. Guest mode available.",
        link: "/auth/signup",
        status: "ACTIVE",
        color: "cyan",
    },
    {
        id: "MOD_02",
        title: "Beatmap Cloud",
        desc: "Global database. Direct share links. Metadata sync.",
        link: "/beatmaps",
        status: "ONLINE",
        color: "fuchsia",
    },
    {
        id: "MOD_03",
        title: "Discovery Feed",
        desc: "Algorithmic sorting. Tag filtration. Difficulty metrics.",
        link: "/beatmaps",
        status: "BETA",
        color: "indigo",
    },
];

export default function Home() {
    return (
        <div className="relative isolate pt-14 dark:bg-slate-950 overflow-hidden">
            {/* Background Effects */}
            <div
                className="absolute inset-x-0 -top-40 -z-10 transform-gpu overflow-hidden blur-3xl sm:-top-80"
                aria-hidden="true"
            >
                <div className="relative left-[calc(50%-11rem)] aspect-1155/678 w-144.5 -translate-x-1/2 rotate-30 bg-linear-to-tr from-cyan-500 via-violet-500 to-fuchsia-500 opacity-20 sm:left-[calc(50%-30rem)] sm:w-288.75"></div>
            </div>

            <div
                className="absolute inset-x-0 top-[calc(100%-13rem)] -z-10 transform-gpu overflow-hidden blur-3xl sm:top-[calc(100%-30rem)]"
                aria-hidden="true"
            >
                <div
                    className="relative left-[calc(50%+3rem)] aspect-1155/678 w-144.5 -translate-x-1/2 bg-linear-to-tr from-fuchsia-500 to-cyan-500 opacity-20 sm:left-[calc(50%+36rem)] sm:w-288.75"
                    style={{
                        clipPath:
                            "polygon(74.1% 44.1%, 100% 61.6%, 97.5% 26.9%, 85.5% 0.1%, 80.7% 2%, 72.5% 32.5%, 60.2% 62.4%, 52.4% 68.1%, 47.5% 58.3%, 45.2% 34.5%, 27.5% 76.7%, 0.1% 64.9%, 17.9% 100%, 27.6% 76.8%, 76.1% 97.7%, 74.1% 44.1%)",
                    }}
                ></div>
            </div>

            <div className="mx-auto max-w-7xl px-6 lg:px-8">
                {/* Hero Section */}
                <div className="mx-auto max-w-3xl py-24 sm:py-32 lg:pt-40">
                    <div className="text-center">
                        <h1 className="font-wordmark bg-clip-text text-transparent bg-linear-to-r from-cyan-400 via-violet-400 to-fuchsia-400 text-6xl sm:text-8xl mt-2 tracking-tighter drop-shadow-[0_0_35px_rgba(168,85,247,0.4)] animate-pulse-slow">
                            line dash
                        </h1>
                        <h2 className="font-display text-3xl tracking-tight sm:text-5xl bg-clip-text text-transparent bg-linear-to-b from-slate-200 to-slate-500">
                            Feel the beat, follow the line.
                        </h2>

                        <div className="mt-10 flex items-center justify-center gap-x-6">
                            <Link
                                href="/play"
                                className="group relative px-8 py-3 bg-linear-to-r from-cyan-500 to-fuchsia-600 text-white font-bold uppercase tracking-widest hover:from-cyan-400 hover:to-fuchsia-500 transition-all -skew-x-12 shadow-[0_0_20px_rgba(217,70,239,0.4)]"
                            >
                                <span className="block skew-x-12">
                                    play now
                                </span>
                                <div className="absolute inset-0 border border-white/20 group-hover:scale-105 transition-transform"></div>
                            </Link>
                            <Link
                                href="/auth/signup"
                                className="text-sm font-semibold leading-6 text-slate-300 uppercase tracking-widest hover:text-white transition-colors flex items-center gap-2"
                            >
                                <span className="w-2 h-2 rounded-full bg-fuchsia-500 animate-pulse"></span>
                                sign up <span aria-hidden="true">→</span>
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
                                className={`relative group bg-slate-900/40 border border-white/5 p-6 hover:bg-slate-900/60 transition-all overflow-hidden backdrop-blur-sm
                                ${
                                    feature.color === "cyan"
                                        ? "hover:border-cyan-500/50"
                                        : feature.color === "fuchsia"
                                          ? "hover:border-fuchsia-500/50"
                                          : "hover:border-indigo-500/50"
                                }`}
                            >
                                {/* Tech Corner Markers */}
                                <div
                                    className={`absolute top-0 left-0 w-2 h-2 border-t-2 border-l-2 border-slate-700 transition-colors
                                    ${
                                        feature.color === "cyan"
                                            ? "group-hover:border-cyan-500"
                                            : feature.color === "fuchsia"
                                              ? "group-hover:border-fuchsia-500"
                                              : "group-hover:border-indigo-500"
                                    }`}
                                ></div>

                                <div
                                    className={`absolute bottom-0 right-0 w-2 h-2 border-b-2 border-r-2 border-slate-700 transition-colors
                                    ${
                                        feature.color === "cyan"
                                            ? "group-hover:border-cyan-500"
                                            : feature.color === "fuchsia"
                                              ? "group-hover:border-fuchsia-500"
                                              : "group-hover:border-indigo-500"
                                    }`}
                                ></div>

                                <div className="flex justify-between items-start mb-4">
                                    <span
                                        className={`font-mono text-xs opacity-70
                                        ${
                                            feature.color === "cyan"
                                                ? "text-cyan-400"
                                                : feature.color === "fuchsia"
                                                  ? "text-fuchsia-400"
                                                  : "text-indigo-400"
                                        }`}
                                    >
                                        {feature.id}
                                    </span>
                                    <span
                                        className={`font-mono text-[10px] px-2 py-0.5 border rounded-sm
                                        ${
                                            feature.color === "cyan"
                                                ? "bg-cyan-950/50 text-cyan-300 border-cyan-900"
                                                : feature.color === "fuchsia"
                                                  ? "bg-fuchsia-950/50 text-fuchsia-300 border-fuchsia-900"
                                                  : "bg-indigo-950/50 text-indigo-300 border-indigo-900"
                                        }`}
                                    >
                                        {feature.status}
                                    </span>
                                </div>

                                <h3
                                    className={`font-display text-xl font-bold text-white mb-2 transition-colors
                                    ${
                                        feature.color === "cyan"
                                            ? "group-hover:text-cyan-400"
                                            : feature.color === "fuchsia"
                                              ? "group-hover:text-fuchsia-400"
                                              : "group-hover:text-indigo-400"
                                    }`}
                                >
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

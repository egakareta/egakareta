import Link from "next/link";

export default function SignupPage() {
    return (
        <main className="mx-auto w-[min(760px,calc(100%-1.3rem))] py-8 sm:w-[min(760px,calc(100%-2.4rem))] sm:py-14">
            <section className="rounded-2xl border border-sky-200/30 bg-slate-950/70 p-5 backdrop-blur-sm sm:p-8">
                <p className="text-xs font-bold tracking-[0.14em] text-cyan-300 uppercase">
                    Join The Community
                </p>
                <h1 className="font-display mt-2 text-3xl font-semibold sm:text-4xl">
                    Create your Line Dash account
                </h1>
                <p className="mt-3 text-slate-300">
                    Publish beatmaps, vote on difficulty, and climb the global
                    PP board.
                </p>

                <form className="mt-6 grid gap-4">
                    <label className="grid gap-2 text-sm">
                        Username
                        <input
                            type="text"
                            placeholder="your-handle"
                            className="rounded-lg border border-cyan-100/25 bg-slate-900/80 px-3 py-2.5 text-slate-100 outline-hidden ring-cyan-300/40 focus:ring-2"
                        />
                    </label>
                    <label className="grid gap-2 text-sm">
                        Email
                        <input
                            type="email"
                            placeholder="you@example.com"
                            className="rounded-lg border border-cyan-100/25 bg-slate-900/80 px-3 py-2.5 text-slate-100 outline-hidden ring-cyan-300/40 focus:ring-2"
                        />
                    </label>
                    <label className="grid gap-2 text-sm">
                        Password
                        <input
                            type="password"
                            placeholder="Create a strong password"
                            className="rounded-lg border border-cyan-100/25 bg-slate-900/80 px-3 py-2.5 text-slate-100 outline-hidden ring-cyan-300/40 focus:ring-2"
                        />
                    </label>
                    <button
                        type="button"
                        className="rounded-full bg-linear-to-r from-cyan-300 to-sky-200 px-4 py-2.5 text-sm font-bold text-sky-950"
                    >
                        Sign up
                    </button>
                </form>

                <p className="mt-5 text-sm text-slate-400">
                    Already have an account?{" "}
                    <Link
                        href="/auth/login"
                        className="text-cyan-300 hover:text-cyan-200"
                    >
                        Log in
                    </Link>
                    .
                </p>
            </section>
        </main>
    );
}

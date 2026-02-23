import Link from "next/link";
import { login } from "../actions";

export default async function LoginPage({
    searchParams,
}: {
    searchParams: Promise<{ message?: string; error?: string }>;
}) {
    const params = await searchParams;
    return (
        <main className="mx-auto w-[min(760px,calc(100%-1.3rem))] py-8 sm:w-[min(760px,calc(100%-2.4rem))] sm:py-14">
            <section className="rounded-2xl border border-sky-200/30 bg-slate-950/70 p-5 backdrop-blur-sm sm:p-8">
                <p className="text-xs font-bold tracking-[0.14em] text-cyan-300 uppercase">
                    Account Access
                </p>
                <h1 className="font-display mt-2 text-3xl font-semibold sm:text-4xl">
                    Log in to Line Dash
                </h1>
                <p className="mt-3 text-slate-300">
                    Continue your ranked grind, saved maps, and profile
                    progress.
                </p>

                {params.message && (
                    <div className="mt-4 rounded-md bg-cyan-900/50 p-3 text-sm text-cyan-200 border border-cyan-500/30">
                        {params.message}
                    </div>
                )}
                {params.error && (
                    <div className="mt-4 rounded-md bg-red-900/50 p-3 text-sm text-red-200 border border-red-500/30">
                        {params.error}
                    </div>
                )}

                <form className="mt-6 grid gap-4">
                    <label className="grid gap-2 text-sm">
                        Email
                        <input
                            name="email"
                            type="email"
                            placeholder="you@example.com"
                            required
                            className="rounded-lg border border-cyan-100/25 bg-slate-900/80 px-3 py-2.5 text-slate-100 outline-hidden ring-cyan-300/40 focus:ring-2"
                        />
                    </label>
                    <label className="grid gap-2 text-sm">
                        Password
                        <input
                            name="password"
                            type="password"
                            placeholder="Your password"
                            required
                            className="rounded-lg border border-cyan-100/25 bg-slate-900/80 px-3 py-2.5 text-slate-100 outline-hidden ring-cyan-300/40 focus:ring-2"
                        />
                    </label>
                    <button
                        formAction={login}
                        className="rounded-full bg-linear-to-r from-cyan-300 to-sky-200 px-4 py-2.5 text-sm font-bold text-sky-950 hover:opacity-90 transition-opacity"
                    >
                        Log in
                    </button>
                </form>

                <div className="mt-4 grid gap-2 text-sm text-slate-300">
                    <button
                        type="button"
                        className="rounded-full border border-sky-200/35 px-4 py-2.5"
                    >
                        Continue with Google
                    </button>
                    <button
                        type="button"
                        className="rounded-full border border-sky-200/35 px-4 py-2.5"
                    >
                        Play in Guest Mode
                    </button>
                </div>

                <p className="mt-5 text-sm text-slate-400">
                    No account yet?{" "}
                    <Link
                        href="/auth/signup"
                        className="text-cyan-300 hover:text-cyan-200"
                    >
                        Create one
                    </Link>
                    .
                </p>
            </section>
        </main>
    );
}

"use client";

import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useState, Suspense } from "react";
import { createClient } from "@/lib/supabase/client";

function LoginForm() {
    const router = useRouter();
    const searchParams = useSearchParams();
    const [error, setError] = useState<string | null>(
        searchParams.get("error"),
    );
    const [message, setMessage] = useState<string | null>(
        searchParams.get("message"),
    );
    const [loading, setLoading] = useState(false);

    const supabase = createClient();

    const handleLogin = async (e: React.SubmitEvent<HTMLFormElement>) => {
        e.preventDefault();
        setLoading(true);
        setError(null);
        setMessage(null);

        const formData = new FormData(e.currentTarget);
        const email = formData.get("email") as string;
        const password = formData.get("password") as string;

        const { error } = await supabase.auth.signInWithPassword({
            email,
            password,
        });

        if (error) {
            setError(error.message);
            setLoading(false);
            return;
        }

        router.push("/");
        router.refresh();
    };

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

                {message && (
                    <div className="mt-4 rounded-md bg-cyan-900/50 p-3 text-sm text-cyan-200 border border-cyan-500/30">
                        {message}
                    </div>
                )}
                {error && (
                    <div className="mt-4 rounded-md bg-red-900/50 p-3 text-sm text-red-200 border border-red-500/30">
                        {error}
                    </div>
                )}

                <form onSubmit={handleLogin} className="mt-6 grid gap-4">
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
                        disabled={loading}
                        className="rounded-full bg-linear-to-r from-cyan-300 to-sky-200 px-4 py-2.5 text-sm font-bold text-sky-950 hover:opacity-90 transition-opacity disabled:opacity-50"
                    >
                        {loading ? "Logging in..." : "Log in"}
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

export default function LoginPage() {
    return (
        <Suspense>
            <LoginForm />
        </Suspense>
    );
}

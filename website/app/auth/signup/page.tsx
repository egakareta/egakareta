"use client";

import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { useState, Suspense } from "react";
import { createClient } from "@/lib/supabase/client";

function SignupForm() {
    const router = useRouter();
    const searchParams = useSearchParams();
    const [error, setError] = useState<string | null>(
        searchParams.get("error"),
    );
    const [loading, setLoading] = useState(false);

    const supabase = createClient();

    const handleSignup = async (e: React.SubmitEvent<HTMLFormElement>) => {
        e.preventDefault();
        setLoading(true);
        setError(null);

        const formData = new FormData(e.currentTarget);
        const email = formData.get("email") as string;
        const password = formData.get("password") as string;
        const username = formData.get("username") as string;

        const { error } = await supabase.auth.signUp({
            email,
            password,
            options: {
                data: {
                    username,
                },
            },
        });

        if (error) {
            setError(error.message);
            setLoading(false);
            return;
        }

        router.push(
            "/auth/login?message=Check email to continue sign in process",
        );
        router.refresh();
    };

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

                {error && (
                    <div className="mt-4 rounded-md bg-red-900/50 p-3 text-sm text-red-200 border border-red-500/30">
                        {error}
                    </div>
                )}

                <form onSubmit={handleSignup} className="mt-6 grid gap-4">
                    <label className="grid gap-2 text-sm">
                        Username
                        <input
                            name="username"
                            type="text"
                            placeholder="your-handle"
                            required
                            minLength={3}
                            className="rounded-lg border border-cyan-100/25 bg-slate-900/80 px-3 py-2.5 text-slate-100 outline-hidden ring-cyan-300/40 focus:ring-2"
                        />
                    </label>
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
                            placeholder="Create a strong password"
                            required
                            minLength={6}
                            className="rounded-lg border border-cyan-100/25 bg-slate-900/80 px-3 py-2.5 text-slate-100 outline-hidden ring-cyan-300/40 focus:ring-2"
                        />
                    </label>
                    <button
                        disabled={loading}
                        className="rounded-full bg-linear-to-r from-cyan-300 to-sky-200 px-4 py-2.5 text-sm font-bold text-sky-950 hover:opacity-90 transition-opacity disabled:opacity-50"
                    >
                        {loading ? "Signing up..." : "Sign up"}
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

export default function SignupPage() {
    return (
        <Suspense>
            <SignupForm />
        </Suspense>
    );
}

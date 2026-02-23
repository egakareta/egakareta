import type { Metadata } from "next";
import { Sora, Unbounded } from "next/font/google";
import Link from "next/link";
import type { ReactNode } from "react";
import "./globals.css";

const sora = Sora({
    subsets: ["latin"],
    variable: "--font-sora",
    display: "swap",
});

const unbounded = Unbounded({
    subsets: ["latin"],
    variable: "--font-unbounded",
    display: "swap",
});

export const metadata: Metadata = {
    title: "Line Dash",
    description: "Feel the beat, follow the line.",
};

export default function RootLayout({
    children,
}: Readonly<{
    children: ReactNode;
}>) {
    return (
        <html lang="en" className={`${sora.variable} ${unbounded.variable}`}>
            <head>
                <link rel="icon" href="/assets/favicon.png" type="image/png" />
            </head>
            <body className="font-sans">
                <div className="mx-auto min-h-screen w-[min(1200px,calc(100%-1.2rem))] sm:w-[min(1200px,calc(100%-2rem))]">
                    <header className="sticky top-2 z-20 mt-2 rounded-2xl border border-sky-200/20 bg-slate-950/70 px-4 py-3 backdrop-blur-md sm:px-6">
                        <nav className="flex flex-wrap items-center justify-between gap-3">
                            <Link
                                href="/"
                                className="font-display text-lg font-semibold tracking-wide text-cyan-200"
                            >
                                LINE DASH
                            </Link>
                            <div className="flex flex-wrap gap-2 text-sm text-slate-200 sm:gap-4">
                                <Link
                                    href="/beatmaps"
                                    className="hover:text-cyan-300"
                                >
                                    Beatmaps
                                </Link>
                                <Link
                                    href="/leaderboards/pp"
                                    className="hover:text-cyan-300"
                                >
                                    PP Leaderboard
                                </Link>
                                <Link
                                    href="/profiles"
                                    className="hover:text-cyan-300"
                                >
                                    Profiles
                                </Link>
                                <Link
                                    href="/auth/login"
                                    className="rounded-full border border-cyan-200/40 px-3 py-1.5 hover:text-cyan-300"
                                >
                                    Log In
                                </Link>
                            </div>
                        </nav>
                    </header>
                    {children}
                    <footer className="mb-5 mt-8 border-t border-sky-200/15 py-4 text-center text-xs text-slate-400">
                        Phase 3 web portal prototype for Line Dash community
                        features.
                    </footer>
                </div>
            </body>
        </html>
    );
}

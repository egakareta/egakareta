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
            <body className="font-sans relative">
                {/* Background Grid - already in CSS body, adding a subtle vignette here */}
                <div className="fixed inset-0 pointer-events-none bg-[radial-gradient(circle_at_center,transparent_0%,#020617_90%)] z-0" />

                <div className="relative z-10 flex flex-col min-h-screen">
                    <header className="sticky top-0 z-50 border-b border-cyan-900/30 bg-slate-950/80 backdrop-blur-md">
                        <nav className="mx-auto flex h-16 max-w-7xl items-center justify-between px-4 sm:px-6 lg:px-8">
                            <div className="flex items-center gap-4">
                                <Link
                                    href="/"
                                    className="group flex items-center gap-2"
                                >
                                    <div className="h-6 w-1 bg-cyan-400 group-hover:bg-cyan-300 transition-colors" />
                                    <span className="font-display text-lg font-bold tracking-widest text-slate-100 group-hover:text-cyan-400 transition-colors">
                                        LINE DASH
                                    </span>
                                </Link>
                                <span className="hidden sm:inline-block px-2 py-0.5 text-[10px] font-mono font-medium text-cyan-500 bg-cyan-950/30 border border-cyan-900/50 rounded-sm uppercase tracking-wider">
                                    V 0.3.0 // BETA
                                </span>
                            </div>

                            <div className="flex items-center gap-1 sm:gap-6 text-sm font-medium">
                                <Link
                                    href="/beatmaps"
                                    className="hidden sm:block px-3 py-1 text-slate-400 hover:text-cyan-400 transition-colors uppercase tracking-wide text-xs"
                                >
                                    Database
                                </Link>
                                <Link
                                    href="/leaderboards/pp"
                                    className="hidden sm:block px-3 py-1 text-slate-400 hover:text-cyan-400 transition-colors uppercase tracking-wide text-xs"
                                >
                                    Rankings
                                </Link>
                                <Link
                                    href="/auth/login"
                                    className="ml-2 flex items-center gap-2 border border-cyan-500/50 bg-cyan-500/10 px-4 py-1.5 text-cyan-400 hover:bg-cyan-500/20 hover:border-cyan-400 transition-all uppercase tracking-wider text-xs font-bold skew-x-[-10deg]"
                                >
                                    <span className="skew-x-[10deg]">
                                        Connect
                                    </span>
                                </Link>
                            </div>
                        </nav>

                        {/* Technical decorative line */}
                        <div className="absolute bottom-0 left-0 h-[1px] w-full bg-gradient-to-r from-transparent via-cyan-500/50 to-transparent opacity-50" />
                    </header>

                    <main className="flex-1">{children}</main>

                    <footer className="border-t border-slate-800 bg-slate-950 py-8 text-center text-xs text-slate-500 font-mono uppercase tracking-widest">
                        <div className="mx-auto max-w-7xl px-6 flex flex-col sm:flex-row justify-between items-center gap-4">
                            <span>System Status: Operational</span>
                            <span>
                                © 2026 Line Dash Systems // All Rights Reserved
                            </span>
                        </div>
                    </footer>
                </div>
            </body>
        </html>
    );
}

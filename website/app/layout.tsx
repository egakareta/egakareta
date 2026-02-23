import type { Metadata } from "next";
import { Sora, Unbounded } from "next/font/google";
import type { ReactNode } from "react";
import SkylineNav from "@/components/SkylineNav";
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
                {/* Background Grid */}
                <div className="fixed inset-0 pointer-events-none bg-[radial-gradient(circle_at_center,transparent_0%,#020617_90%)] z-0" />

                <div className="relative z-10 flex flex-col min-h-screen">
                    {/* New Skyline Navigation */}
                    <SkylineNav />

                    {/* Content padding adjusted for new fixed header */}
                    <main className="flex-1 pt-32">{children}</main>

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

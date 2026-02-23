"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import AuthStatus from "./AuthStatus";

export default function SkylineNav() {
    // Generate a deterministic pattern of heights for the "skyline" blocks
    // We'll use a large enough array to cover most screen widths
    const [blocks, setBlocks] = useState<number[]>([]);

    useEffect(() => {
        // Generate blocks only on client to match hydration
        // A simple pseudo-random pattern that looks like a skyline
        const newBlocks = [];
        const pattern = [2, 5, 3, 6, 2, 8, 4, 7, 3, 5, 2, 4, 6];
        for (let i = 0; i < 100; i++) {
            newBlocks.push(...pattern.map((h) => h + (Math.random() * 2 - 1))); // Add slight variance
        }
        setBlocks(newBlocks);
    }, []);

    return (
        <header className="fixed top-0 left-0 right-0 z-50 pointer-events-none">
            {/* Main Black Bar */}
            <div className="bg-slate-950 pointer-events-auto relative z-20 px-6 sm:px-8 h-20 flex items-center justify-between shadow-2xl shadow-black/50 w-full">
                {/* Left: Logo */}
                <div className="flex items-center gap-8">
                    <Link href="/" className="group flex items-center gap-3">
                        <p className="font-wordmark relative w-48 text-4xl text-center">
                            line dash
                        </p>
                    </Link>

                    {/* Desktop Nav */}
                    <nav className="hidden md:flex items-center gap-8 ml-4">
                        <NavLink href="/play">play</NavLink>
                        <NavLink href="/beatmaps">beatmaps</NavLink>
                        <NavLink href="/leaderboards/pp">rankings</NavLink>
                        <NavLink href="/about">about</NavLink>
                    </nav>
                </div>

                {/* Right: Auth & Tools */}
                <div className="flex items-center gap-6">
                    <AuthStatus />
                </div>
            </div>

            {/* The "Skyline" Bottom Edge - Dripping Blocks */}
            <div className="relative z-20 flex w-full h-12 overflow-hidden pointer-events-none items-start">
                {blocks.length === 0 ? (
                    // Server-side / Initial render placeholder to prevent layout shift
                    <div className="w-full h-4 bg-slate-950" />
                ) : (
                    blocks.map((height, i) => (
                        <div
                            key={i}
                            className="bg-slate-950 shrink-0 w-2 sm:w-4 transition-all duration-1000 ease-in-out"
                            style={{
                                height: `${height * 3}px`, // Scale height
                            }}
                        />
                    ))
                )}
            </div>
        </header>
    );
}

function NavLink({
    href,
    children,
}: {
    href: string;
    children: React.ReactNode;
}) {
    return (
        <Link
            href={href}
            className="text-sm font-bold tracking-widest text-slate-400 hover:text-white transition-colors font-display"
        >
            {children}
        </Link>
    );
}

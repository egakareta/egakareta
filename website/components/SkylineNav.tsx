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
                        <div className="relative w-48 h-10">
                            <img
                                src="/assets/wordmark.png"
                                alt="Line Dash"
                                className="w-full h-full object-contain"
                            />
                        </div>
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
                    <div className="hidden sm:flex items-center gap-4 text-slate-500">
                        {/* Discord Icon (Simple SVG) */}
                        <a
                            href="#"
                            className="opacity-50 hover:opacity-100 hover:text-white transition-all"
                        >
                            <svg
                                width="20"
                                height="20"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path d="M20.317 4.54101C18.7873 3.82774 17.147 3.30224 15.4319 3.00126C15.4007 2.99545 15.3695 3.00997 15.3534 3.039C15.1424 3.4203 14.9087 3.91774 14.7451 4.30873C12.9004 4.02808 11.0652 4.02808 9.25832 4.30873C9.09465 3.90905 8.85248 3.4203 8.64057 3.039C8.62448 3.01094 8.59328 2.99642 8.56205 3.00126C6.84791 3.30128 5.20756 3.82678 3.67693 4.54101C3.66368 4.54681 3.65233 4.5565 3.64479 4.56907C0.533392 9.29283 -0.31895 13.9005 0.0991801 18.451C0.101072 18.4733 0.11337 18.4946 0.130398 18.5081C2.18321 20.0401 4.17171 20.9701 6.12328 21.5866C6.15451 21.5963 6.18761 21.5847 6.20748 21.5585C6.66913 20.9179 7.08064 20.2424 7.43348 19.532C7.4543 19.4904 7.43442 19.441 7.39186 19.4246C6.73913 19.173 6.1176 18.8662 5.51973 18.5178C5.47244 18.4897 5.46865 18.421 5.51216 18.3881C5.63797 18.2923 5.76382 18.1926 5.88396 18.0919C5.90569 18.0736 5.93598 18.0697 5.96153 18.0813C9.88928 19.9036 14.1415 19.9036 18.023 18.0813C18.0485 18.0687 18.0788 18.0726 18.1015 18.091C18.2216 18.1916 18.3475 18.2923 18.4742 18.3881C18.5177 18.421 18.5149 18.4897 18.4676 18.5178C17.8697 18.8729 17.2482 19.173 16.5945 19.4236C16.552 19.4401 16.533 19.4904 16.5538 19.532C16.9143 20.2414 17.3258 20.9169 17.7789 21.5576C17.7978 21.5847 17.8319 21.5963 17.8631 21.5866C19.8241 20.9701 21.8126 20.0401 23.8654 18.5081C23.8834 18.4946 23.8948 18.4742 23.8967 18.452C24.3971 13.1911 23.0585 8.6212 20.3482 4.57004C20.3416 4.5565 20.3303 4.54681 20.317 4.54101ZM8.02002 15.6802C6.8375 15.6802 5.86313 14.577 5.86313 13.222C5.86313 11.8671 6.8186 10.7639 8.02002 10.7639C9.23087 10.7639 10.1958 11.8768 10.1769 13.222C10.1769 14.577 9.22141 15.6802 8.02002 15.6802ZM15.9947 15.6802C14.8123 15.6802 13.8379 14.577 13.8379 13.222C13.8379 11.8671 14.7933 10.7639 15.9947 10.7639C17.2056 10.7639 18.1705 11.8768 18.1516 13.222C18.1516 14.577 17.2056 15.6802 15.9947 15.6802Z" />
                            </svg>
                        </a>
                        {/* GitHub Icon (Simple SVG) */}
                        <a
                            href="#"
                            className="opacity-50 hover:opacity-100 hover:text-white transition-all"
                        >
                            <svg
                                width="20"
                                height="20"
                                viewBox="0 0 24 24"
                                fill="currentColor"
                            >
                                <path d="M12 2A10 10 0 0 0 2 12c0 4.42 2.87 8.17 6.84 9.5.5.08.66-.23.66-.5v-1.69c-2.77.6-3.36-1.34-3.36-1.34-.46-1.16-1.11-1.47-1.11-1.47-.91-.62.07-.6.07-.6 1 .07 1.53 1.03 1.53 1.03.87 1.52 2.34 1.07 2.91.83.09-.65.35-1.09.63-1.34-2.22-.25-4.55-1.11-4.55-4.92 0-1.11.38-2 1.03-2.71-.1-.25-.45-1.29.1-2.64 0 0 .84-.27 2.75 1.02a9.56 9.56 0 0 1 5 0c1.91-1.29 2.75-1.02 2.75-1.02.55 1.35.2 2.39.1 2.64.65.71 1.03 1.6 1.03 2.71 0 3.82-2.34 4.66-4.57 4.91.36.31.69.92.69 1.85V21c0 .27.16.59.67.5C19.14 20.16 22 16.42 22 12A10 10 0 0 0 12 2z" />
                            </svg>
                        </a>
                    </div>

                    <div className="h-8 w-px bg-slate-800 hidden sm:block" />

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

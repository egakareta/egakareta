"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { createClient } from "@/lib/supabase/client";
import { useRouter } from "next/navigation";
import type { User } from "@supabase/supabase-js";

export default function AuthStatus() {
    const [user, setUser] = useState<User | null>(null);
    const [loading, setLoading] = useState(true);
    const router = useRouter();
    const supabase = createClient();

    useEffect(() => {
        const getUser = async () => {
            const {
                data: { user },
            } = await supabase.auth.getUser();
            setUser(user);
            setLoading(false);
        };
        getUser();

        const {
            data: { subscription },
        } = supabase.auth.onAuthStateChange((_event, session) => {
            setUser(session?.user ?? null);
            setLoading(false);
        });

        return () => subscription.unsubscribe();
    }, [supabase]);

    const handleSignOut = async () => {
        await supabase.auth.signOut();
        router.refresh();
        router.push("/auth/login");
    };

    // While loading, we render nothing (or a small placeholder) to avoid flash of content
    // Since this is in the navbar, a small delay is acceptable, or we can render the "Connect" button by default?
    // Better to render nothing until we know.
    if (loading) return null;

    return (
        <>
            {user ? (
                <div className="flex items-center gap-4">
                    <Link
                        href={`/profiles/${user.user_metadata.username}`}
                        className="hidden sm:block text-slate-400 hover:text-cyan-400 transition-colors uppercase tracking-wide text-xs"
                    >
                        {user.user_metadata?.username || "User"}
                    </Link>
                    <button
                        onClick={handleSignOut}
                        className="flex items-center gap-2 border border-fuchsia-500/50 bg-gradient-to-r from-fuchsia-500/10 to-purple-500/10 px-4 py-1.5 text-fuchsia-400 hover:text-white hover:from-fuchsia-500/80 hover:to-purple-500/80 hover:border-white/50 transition-all uppercase tracking-wider text-xs font-bold skew-x-[-10deg]"
                    >
                        <span className="skew-x-[10deg]">Log Out</span>
                    </button>
                </div>
            ) : (
                <Link
                    href="/auth/login"
                    className="ml-2 flex items-center gap-2 border border-cyan-500/50 bg-gradient-to-r from-cyan-500/10 to-fuchsia-500/10 px-4 py-1.5 text-cyan-400 hover:text-white hover:from-cyan-500/80 hover:to-fuchsia-500/80 hover:border-white/50 transition-all uppercase tracking-wider text-xs font-bold skew-x-[-10deg]"
                >
                    <span className="skew-x-[10deg]">Connect</span>
                </Link>
            )}
        </>
    );
}

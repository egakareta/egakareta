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
                        className="hidden sm:block text-slate-300 hover:text-cyan-400 transition-colors uppercase tracking-wide text-xs font-bold font-display"
                    >
                        {user.user_metadata?.username || "User"}
                    </Link>
                    <button
                        onClick={handleSignOut}
                        className="text-fuchsia-400 hover:text-fuchsia-300 transition-colors uppercase tracking-wider text-xs font-bold font-display"
                    >
                        Log Out
                    </button>
                </div>
            ) : (
                <Link
                    href="/auth/login"
                    className="flex items-center gap-2 text-cyan-400 hover:text-cyan-300 transition-colors tracking-wider text-xs font-bold font-display"
                >
                    <span>login</span>
                </Link>
            )}
        </>
    );
}

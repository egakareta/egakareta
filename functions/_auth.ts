/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
import type { SupabaseClient, User } from "@supabase/supabase-js";

import type { Database } from "./_supabase_types";

export const USERNAME_PATTERN = /^[a-zA-Z0-9_]{3,24}$/;
const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

type AuthSession = {
    access_token: string;
    refresh_token: string;
    expires_at: number | null;
    token_type: string;
};

type AuthProfile = {
    id: string;
    username: string | null;
    avatar_url: string | null;
    country: string;
};

export type AuthPayload = {
    session: AuthSession;
    user: {
        id: string;
        email: string | null;
    };
    profile: AuthProfile | null;
};

type TurnstileSiteverifyResponse = {
    success?: boolean;
    "error-codes"?: string[];
};

const TURNSTILE_TEST_SITE_KEY = "1x00000000000000000000AA";
const TURNSTILE_TEST_SECRET_KEY = "1x0000000000000000000000000000000AA";

export function isEmail(value: string): boolean {
    return EMAIL_PATTERN.test(value);
}

export function normalizeIdentifier(value: unknown): string {
    return typeof value === "string" ? value.trim() : "";
}

export function normalizeEmail(value: unknown): string {
    return typeof value === "string" ? value.trim().toLowerCase() : "";
}

export function normalizeUsername(value: unknown): string {
    return typeof value === "string" ? value.trim() : "";
}

export function validateSignupInput(input: {
    username: unknown;
    email: unknown;
    password: unknown;
}): { username: string; email: string; password: string } | { error: string } {
    const username = normalizeUsername(input.username);
    const email = normalizeEmail(input.email);
    const password = typeof input.password === "string" ? input.password : "";

    if (!USERNAME_PATTERN.test(username)) {
        return {
            error: "Username must be 3-24 characters and only use letters, numbers, or underscores.",
        };
    }

    if (!isEmail(email)) {
        return { error: "Enter a valid email address." };
    }

    if (password.length < 8) {
        return { error: "Password must be at least 8 characters." };
    }

    return { username, email, password };
}

export function jsonResponse(body: unknown, init?: ResponseInit) {
    return new Response(JSON.stringify(body), {
        ...init,
        headers: {
            "Content-Type": "application/json",
            ...init?.headers,
        },
    });
}

export async function readRequestBody(
    request: Request,
): Promise<Record<string, unknown>> {
    const contentType = request.headers.get("Content-Type") ?? "";

    try {
        if (contentType.includes("application/json")) {
            const body = await request.json();
            return body && typeof body === "object"
                ? (body as Record<string, unknown>)
                : {};
        }

        const formData = await request.formData();
        return Object.fromEntries(formData.entries());
    } catch {
        return {};
    }
}

export function turnstileToken(body: Record<string, unknown>) {
    const token = body["cf-turnstile-response"] ?? body.turnstileToken;
    return typeof token === "string" ? token.trim() : "";
}

export async function verifyTurnstileToken(
    env: Cloudflare.Env,
    token: string,
    request: Request,
): Promise<string | null> {
    if (!env.TURNSTILE_SITE_KEY) {
        return null;
    }

    if (!token) {
        return "Complete the verification challenge before continuing.";
    }

    const secret =
        env.TURNSTILE_SECRET_KEY ??
        (env.TURNSTILE_SITE_KEY === TURNSTILE_TEST_SITE_KEY
            ? TURNSTILE_TEST_SECRET_KEY
            : "");
    if (!secret) {
        return "Verification is not configured. Please try again later.";
    }

    const formData = new FormData();
    formData.set("secret", secret);
    formData.set("response", token);
    const ip = request.headers.get("CF-Connecting-IP");
    if (ip) {
        formData.set("remoteip", ip);
    }

    try {
        const response = await fetch(
            "https://challenges.cloudflare.com/turnstile/v0/siteverify",
            { method: "POST", body: formData },
        );
        const result = (await response.json()) as TurnstileSiteverifyResponse;
        return result.success
            ? null
            : "Complete the verification challenge before continuing.";
    } catch (error) {
        console.error("turnstile verification failed", error);
        return "Verification is temporarily unavailable. Please try again.";
    }
}

export async function resolveIdentifierToEmail(
    supabase: SupabaseClient<Database>,
    identifier: string,
): Promise<string | null> {
    if (isEmail(identifier)) {
        return identifier.toLowerCase();
    }

    const { data, error } = await supabase.rpc("resolve_username_to_email", {
        username_to_resolve: identifier,
    });

    if (error) {
        throw error;
    }

    return data ?? null;
}

export async function fetchAuthProfile(
    supabase: SupabaseClient<Database>,
    userId: string,
): Promise<AuthProfile | null> {
    const { data, error } = await supabase
        .from("profiles")
        .select("id, username, avatar_url, country")
        .eq("id", userId)
        .maybeSingle();

    if (error) {
        throw error;
    }

    return data;
}

export async function buildAuthPayload(
    supabase: SupabaseClient<Database>,
    user: User,
    session: {
        access_token: string;
        refresh_token: string;
        expires_at?: number;
        token_type: string;
    },
): Promise<AuthPayload> {
    const profile = await fetchAuthProfile(supabase, user.id);
    return {
        session: {
            access_token: session.access_token,
            refresh_token: session.refresh_token,
            expires_at: session.expires_at ?? null,
            token_type: session.token_type,
        },
        user: {
            id: user.id,
            email: user.email ?? null,
        },
        profile,
    };
}

export function isUnconfirmedEmailError(message: string): boolean {
    return /email.*not.*confirmed|confirm.*email/i.test(message);
}

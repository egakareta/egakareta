/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
import {
    buildAuthPayload,
    type AuthPayload,
    isUnconfirmedEmailError,
    normalizeIdentifier,
    readRequestBody,
    resolveIdentifierToEmail,
    turnstileToken,
    verifyTurnstileToken,
} from "./_auth";
import { createSupabaseAdminClient, createSupabaseClient } from "./_supabase";
import type { Session, User } from "@supabase/supabase-js";

const UUID_PATTERN =
    /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

function escapeHtml(value: string) {
    return value.replace(
        /[&<>"']/g,
        (character) =>
            ({
                "&": "&amp;",
                "<": "&lt;",
                ">": "&gt;",
                '"': "&quot;",
                "'": "&#39;",
            })[character] ?? character,
    );
}

function safeScriptJson(value: unknown) {
    return JSON.stringify(value).replace(
        /[<>&]/g,
        (character) =>
            ({
                "<": "\\u003c",
                ">": "\\u003e",
                "&": "\\u0026",
            })[character] ?? character,
    );
}

function htmlResponse(body: string, status = 200) {
    return new Response(body, {
        status,
        headers: { "Content-Type": "text/html; charset=utf-8" },
    });
}

function signInErrorResponse(
    message: string,
    env: Cloudflare.Env,
    handoffId: string,
    status: number,
) {
    return htmlResponse(page(message, true, env, handoffId), status);
}

function page(
    message = "",
    isError = false,
    env: Cloudflare.Env,
    handoffId = "",
) {
    const statusClass = isError ? "error" : "success";
    const safeMessage = escapeHtml(message);
    const safeHandoffId = escapeHtml(handoffId);
    const safeSiteKey = escapeHtml(
        env.TURNSTILE_SITE_KEY ?? "1x00000000000000000000AA",
    );

    return `<!doctype html>
<html lang="en">
    <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>sign in</title>
        <link rel="icon" type="image/png" href="/assets/favicon.png" />
        <script src="https://challenges.cloudflare.com/turnstile/v0/api.js" async defer></script>
        <style>
            :root { color-scheme: dark; }
            body {
                margin: 0;
                min-height: 100vh;
                display: grid;
                place-items: center;
                background: #050607;
                color: #f5f7fb;
                font-family: "Segoe UI", system-ui, sans-serif;
            }
            main {
                width: min(100% - 32px, 420px);
                padding: 28px;
                border: 1px solid rgba(255, 255, 255, 0.16);
                background: rgba(255, 255, 255, 0.06);
                box-shadow: 0 24px 80px rgba(0, 0, 0, 0.35);
            }
            h1 { margin: 0 0 8px; font-size: 28px; }
            p { margin: 0 0 20px; color: #bec7d4; line-height: 1.5; }
            label { display: grid; gap: 7px; margin: 14px 0; color: #dce4ef; }
            input {
                box-sizing: border-box;
                width: 100%;
                padding: 12px 13px;
                border: 1px solid rgba(255, 255, 255, 0.22);
                background: rgba(0, 0, 0, 0.35);
                color: inherit;
                font: inherit;
            }
            button,
            a.button {
                box-sizing: border-box;
                display: block;
                width: 100%;
                margin-top: 10px;
                padding: 12px 14px;
                border: 0;
                background: #f5f7fb;
                color: #0a0d12;
                font: inherit;
                font-weight: 700;
                text-align: center;
                text-decoration: none;
                cursor: pointer;
            }
            .turnstile-wrap { min-height: 65px; margin: 18px 0 6px; }
            .status { margin: 0 0 18px; padding: 11px 12px; }
            .success { background: rgba(64, 202, 140, 0.16); color: #bff7d9; }
            .error { background: rgba(255, 91, 91, 0.16); color: #ffd0d0; }
            .secondary { margin-top: 12px; color: #bec7d4; text-align: center; }
            .secondary a { color: #f5f7fb; }
        </style>
    </head>
    <body>
        <main>
            <h1>sign in</h1>
            <p>${safeHandoffId ? "Complete sign-in here, then return to the game." : "Sign in to continue playing."}</p>
            ${safeMessage ? `<div class="status ${statusClass}">${safeMessage}</div>` : ""}
            <form method="post" action="/signin">
                ${safeHandoffId ? `<input type="hidden" name="handoff_id" value="${safeHandoffId}" />` : ""}
                <label>username or email<input name="identifier" autocomplete="username" required /></label>
                <label>password<input type="password" name="password" autocomplete="current-password" required /></label>
                <div class="turnstile-wrap"><div class="cf-turnstile" data-sitekey="${safeSiteKey}"></div></div>
                <button type="submit">sign in</button>
            </form>
            <div class="secondary"><a href="/signup">create account</a></div>
        </main>
    </body>
</html>`;
}

function browserCompletionPage(payload: unknown) {
    return `<!doctype html>
<html lang="en">
    <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>signed in</title>
        <link rel="icon" type="image/png" href="/assets/favicon.png" />
        <style>
            :root { color-scheme: dark; }
            body {
                margin: 0;
                min-height: 100vh;
                display: grid;
                place-items: center;
                background: #050607;
                color: #f5f7fb;
                font-family: "Segoe UI", system-ui, sans-serif;
            }
            main { width: min(100% - 32px, 420px); }
            h1 { margin: 0 0 8px; font-size: 28px; }
            p { margin: 0; color: #bec7d4; line-height: 1.5; }
            .error { color: #ffd0d0; }
        </style>
    </head>
    <body>
        <main>
            <h1>signed in</h1>
            <p id="status">Returning to the game.</p>
        </main>
        <script>
            const payload = ${safeScriptJson(payload)};
            const status = document.getElementById("status");
            const request = indexedDB.open("egakareta-settings", 1);
            request.onupgradeneeded = () => {
                const db = request.result;
                if (!db.objectStoreNames.contains("settings")) {
                    db.createObjectStore("settings");
                }
            };
            request.onerror = () => {
                status.className = "error";
                status.textContent = "Could not save the session in this browser.";
            };
            request.onsuccess = () => {
                const db = request.result;
                const tx = db.transaction("settings", "readwrite");
                tx.objectStore("settings").put(JSON.stringify(payload), "auth_session");
                tx.oncomplete = () => { window.location.href = "/"; };
                tx.onerror = () => {
                    status.className = "error";
                    status.textContent = "Could not save the session in this browser.";
                };
            };
        </script>
    </body>
</html>`;
}

function nativeCompletionPage() {
    return `<!doctype html>
<html lang="en">
    <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>signed in</title>
        <link rel="icon" type="image/png" href="/assets/favicon.png" />
        <style>
            :root { color-scheme: dark; }
            body {
                margin: 0;
                min-height: 100vh;
                display: grid;
                place-items: center;
                background: #050607;
                color: #f5f7fb;
                font-family: "Segoe UI", system-ui, sans-serif;
            }
            main { width: min(100% - 32px, 420px); }
            h1 { margin: 0 0 8px; font-size: 28px; }
            p { margin: 0; color: #bec7d4; line-height: 1.5; }
        </style>
    </head>
    <body>
        <main>
            <h1>signed in</h1>
            <p>You can return to the game now.</p>
        </main>
    </body>
</html>`;
}

async function validateHandoff(env: Cloudflare.Env, handoffId: string) {
    if (!UUID_PATTERN.test(handoffId)) {
        return "Invalid sign-in handoff.";
    }

    const supabase = createSupabaseAdminClient(env);
    const { data, error } = await supabase
        .from("auth_handoffs")
        .select("expires_at, claimed_at")
        .eq("id", handoffId)
        .maybeSingle();

    if (error) {
        return error.message;
    }
    if (!data) {
        return "Sign-in handoff was not found.";
    }
    if (data.claimed_at || Date.parse(data.expires_at) <= Date.now()) {
        return "Sign-in handoff has expired.";
    }
    return null;
}

type SignInForm = {
    identifier: string;
    password: string;
    captchaToken: string;
    handoffId: string;
};

type SupabaseRequestClient = ReturnType<typeof createSupabaseClient>;

function readSignInForm(body: Record<string, unknown>): SignInForm {
    return {
        identifier: normalizeIdentifier(body.identifier),
        password: typeof body.password === "string" ? body.password : "",
        captchaToken: turnstileToken(body),
        handoffId: typeof body.handoff_id === "string" ? body.handoff_id : "",
    };
}

function validateSignInForm(
    form: SignInForm,
    env: Cloudflare.Env,
): Response | null {
    if (!form.identifier || !form.password) {
        return signInErrorResponse(
            "Enter your username or email and password.",
            env,
            form.handoffId,
            400,
        );
    }

    return null;
}

async function validateSignInCaptcha(
    request: Request,
    env: Cloudflare.Env,
    form: SignInForm,
): Promise<Response | null> {
    const captchaError = await verifyTurnstileToken(
        env,
        form.captchaToken,
        request,
    );
    if (!captchaError) {
        return null;
    }

    return signInErrorResponse(
        captchaError ===
            "Complete the verification challenge before continuing."
            ? "Complete the verification challenge before signing in."
            : captchaError,
        env,
        form.handoffId,
        400,
    );
}

async function validatePostedHandoff(
    env: Cloudflare.Env,
    handoffId: string,
): Promise<Response | null> {
    if (!handoffId) {
        return null;
    }

    const handoffError = await validateHandoff(env, handoffId);
    return handoffError
        ? signInErrorResponse(handoffError, env, "", 400)
        : null;
}

async function resolveSignInEmail(
    supabase: SupabaseRequestClient,
    identifier: string,
    env: Cloudflare.Env,
    handoffId: string,
): Promise<string | Response> {
    try {
        const email = await resolveIdentifierToEmail(supabase, identifier);
        return (
            email ??
            signInErrorResponse(
                "Invalid login credentials.",
                env,
                handoffId,
                401,
            )
        );
    } catch (error) {
        const message =
            error instanceof Error ? error.message : "Login lookup failed.";
        return signInErrorResponse(message, env, handoffId, 500);
    }
}

function signInFailureMessage(message: string) {
    return isUnconfirmedEmailError(message)
        ? "Check your email to confirm this account before signing in."
        : message;
}

async function signInWithEmail(
    supabase: SupabaseRequestClient,
    form: SignInForm,
    email: string,
    env: Cloudflare.Env,
): Promise<{ session: Session; user: User } | Response> {
    const { data, error } = await supabase.auth.signInWithPassword({
        email,
        password: form.password,
        options: {
            captchaToken: form.captchaToken || undefined,
        },
    });

    if (error) {
        return signInErrorResponse(
            signInFailureMessage(error.message),
            env,
            form.handoffId,
            401,
        );
    }

    if (!data.session || !data.user) {
        return signInErrorResponse(
            "Sign-in did not return a usable session.",
            env,
            form.handoffId,
            401,
        );
    }

    return { session: data.session, user: data.user };
}

async function buildSignInPayload(
    supabase: SupabaseRequestClient,
    user: User,
    session: Session,
    env: Cloudflare.Env,
    handoffId: string,
): Promise<AuthPayload | Response> {
    try {
        return await buildAuthPayload(supabase, user, session);
    } catch (error) {
        const message =
            error instanceof Error ? error.message : "Profile lookup failed.";
        return signInErrorResponse(message, env, handoffId, 500);
    }
}

async function completeNativeHandoff(
    env: Cloudflare.Env,
    handoffId: string,
    payload: AuthPayload,
) {
    const admin = createSupabaseAdminClient(env);
    const { data, error } = await admin
        .from("auth_handoffs")
        .update({ auth_payload: payload, profile_id: payload.user.id })
        .eq("id", handoffId)
        .is("claimed_at", null)
        .gt("expires_at", new Date().toISOString())
        .select("id")
        .maybeSingle();

    if (error) {
        console.error("native handoff completion failed", error);
        return htmlResponse(
            page(
                "Could not complete sign-in right now. Please try again.",
                true,
                env,
            ),
            500,
        );
    }

    if (!data) {
        return htmlResponse(
            page("Sign-in handoff has expired.", true, env),
            400,
        );
    }

    return htmlResponse(nativeCompletionPage());
}

function completeBrowserSignIn(payload: AuthPayload) {
    return htmlResponse(browserCompletionPage(payload));
}

function isResponse(value: unknown): value is Response {
    return value instanceof Response;
}

export const onRequestGet: PagesFunction<Cloudflare.Env> = async ({
    request,
    env,
}) => {
    const url = new URL(request.url);
    const handoffId = url.searchParams.get("handoff_id")?.trim() ?? "";
    if (handoffId) {
        const error = await validateHandoff(env, handoffId);
        if (error) {
            return htmlResponse(page(error, true, env), 400);
        }
    }

    return htmlResponse(page("", false, env, handoffId));
};

export const onRequestPost: PagesFunction<Cloudflare.Env> = async ({
    request,
    env,
}) => {
    const form = readSignInForm(await readRequestBody(request));
    const formError = validateSignInForm(form, env);
    if (formError) return formError;

    const captchaError = await validateSignInCaptcha(request, env, form);
    if (captchaError) return captchaError;

    const handoffError = await validatePostedHandoff(env, form.handoffId);
    if (handoffError) return handoffError;

    const supabase = createSupabaseClient(env, request);
    const email = await resolveSignInEmail(
        supabase,
        form.identifier,
        env,
        form.handoffId,
    );
    if (isResponse(email)) return email;

    const signIn = await signInWithEmail(supabase, form, email, env);
    if (isResponse(signIn)) return signIn;

    const payload = await buildSignInPayload(
        supabase,
        signIn.user,
        signIn.session,
        env,
        form.handoffId,
    );
    if (isResponse(payload)) return payload;

    return form.handoffId
        ? completeNativeHandoff(env, form.handoffId, payload)
        : completeBrowserSignIn(payload);
};

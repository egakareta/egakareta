/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
import {
    buildAuthPayload,
    isUnconfirmedEmailError,
    normalizeIdentifier,
    readRequestBody,
    resolveIdentifierToEmail,
} from "./_auth";
import { createSupabaseAdminClient, createSupabaseClient } from "./_supabase";

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

function turnstileToken(body: Record<string, unknown>) {
    const token = body["cf-turnstile-response"] ?? body.turnstileToken;
    return typeof token === "string" ? token.trim() : "";
}

function htmlResponse(body: string, status = 200) {
    return new Response(body, {
        status,
        headers: { "Content-Type": "text/html; charset=utf-8" },
    });
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
    const body = await readRequestBody(request);
    const identifier = normalizeIdentifier(body.identifier);
    const password = typeof body.password === "string" ? body.password : "";
    const captchaToken = turnstileToken(body);
    const handoffId =
        typeof body.handoff_id === "string" ? body.handoff_id : "";

    if (!identifier || !password) {
        return htmlResponse(
            page(
                "Enter your username or email and password.",
                true,
                env,
                handoffId,
            ),
            400,
        );
    }
    if (env.TURNSTILE_SITE_KEY && !captchaToken) {
        return htmlResponse(
            page(
                "Complete the verification challenge before signing in.",
                true,
                env,
                handoffId,
            ),
            400,
        );
    }
    if (handoffId) {
        const handoffError = await validateHandoff(env, handoffId);
        if (handoffError) {
            return htmlResponse(page(handoffError, true, env), 400);
        }
    }

    const supabase = createSupabaseClient(env, request);
    let email: string | null;

    try {
        email = await resolveIdentifierToEmail(supabase, identifier);
    } catch (error) {
        const message =
            error instanceof Error ? error.message : "Login lookup failed.";
        return htmlResponse(page(message, true, env, handoffId), 500);
    }

    if (!email) {
        return htmlResponse(
            page(
                "No account was found for that username or email.",
                true,
                env,
                handoffId,
            ),
            401,
        );
    }

    const { data, error } = await supabase.auth.signInWithPassword({
        email,
        password,
        options: {
            captchaToken: captchaToken || undefined,
        },
    });

    if (error) {
        const message = isUnconfirmedEmailError(error.message)
            ? "Check your email to confirm this account before signing in."
            : error.message;
        return htmlResponse(page(message, true, env, handoffId), 401);
    }

    if (!data.session || !data.user) {
        return htmlResponse(
            page(
                "Sign-in did not return a usable session.",
                true,
                env,
                handoffId,
            ),
            401,
        );
    }

    let payload: Awaited<ReturnType<typeof buildAuthPayload>>;
    try {
        payload = await buildAuthPayload(supabase, data.user, data.session);
    } catch (error) {
        const message =
            error instanceof Error ? error.message : "Profile lookup failed.";
        return htmlResponse(page(message, true, env, handoffId), 500);
    }

    if (handoffId) {
        const admin = createSupabaseAdminClient(env);
        const { error: updateError } = await admin
            .from("auth_handoffs")
            .update({ auth_payload: payload, profile_id: payload.user.id })
            .eq("id", handoffId)
            .is("claimed_at", null)
            .gt("expires_at", new Date().toISOString());

        if (updateError) {
            return htmlResponse(page(updateError.message, true, env), 500);
        }

        return htmlResponse(nativeCompletionPage());
    }

    return htmlResponse(browserCompletionPage(payload));
};

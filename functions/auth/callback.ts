/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
import type { EmailOtpType } from "@supabase/supabase-js";

import { normalizeEmail, readRequestBody } from "../_auth";
import { createSupabaseClient } from "../_supabase";

const OTP_TYPE: EmailOtpType = "email";

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

function htmlResponse(body: string, status = 200) {
    return new Response(body, {
        status,
        headers: { "Content-Type": "text/html; charset=utf-8" },
    });
}

function page(message = "", isError = false, email = "") {
    const statusClass = isError ? "error" : "success";
    const safeMessage = escapeHtml(message);
    const safeEmail = escapeHtml(email);
    return `<!doctype html>
<html lang="en">
    <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>confirm email</title>
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
            .status { margin: 0 0 18px; padding: 11px 12px; }
            .success { background: rgba(64, 202, 140, 0.16); color: #bff7d9; }
            .error { background: rgba(255, 91, 91, 0.16); color: #ffd0d0; }
        </style>
    </head>
    <body>
        <main>
            <h1>confirm email</h1>
            <p>After confirmation, return to the game and sign in.</p>
            ${safeMessage ? `<div class="status ${statusClass}">${safeMessage}</div>` : ""}
            ${
                isError || !safeMessage
                    ? `<form method="post" action="/auth/callback">
                <label>email<input type="email" name="email" autocomplete="email" value="${safeEmail}" required /></label>
                <label>code<input name="token" inputmode="numeric" autocomplete="one-time-code" minlength="6" maxlength="6" required /></label>
                <button type="submit">confirm email</button>
            </form>`
                    : `<a class="button" href="/">open game</a>`
            }
        </main>
    </body>
</html>`;
}

async function verifyTokenHash(
    request: Request,
    env: Cloudflare.Env,
    tokenHash: string,
) {
    const supabase = createSupabaseClient(env, request);
    const { error } = await supabase.auth.verifyOtp({
        token_hash: tokenHash,
        type: OTP_TYPE,
    });

    if (error) {
        return htmlResponse(page(error.message, true), 400);
    }

    return htmlResponse(page("Email confirmed. You can sign in now."));
}

export const onRequestGet: PagesFunction<Cloudflare.Env> = async ({
    request,
    env,
}) => {
    const url = new URL(request.url);
    const tokenHash = url.searchParams.get("token_hash")?.trim();
    if (tokenHash) {
        return verifyTokenHash(request, env, tokenHash);
    }

    return htmlResponse(
        page("", false, normalizeEmail(url.searchParams.get("email"))),
    );
};

export const onRequestPost: PagesFunction<Cloudflare.Env> = async ({
    request,
    env,
}) => {
    const body = await readRequestBody(request);
    const email = normalizeEmail(body.email);
    const token = typeof body.token === "string" ? body.token.trim() : "";

    if (!email || !token) {
        return htmlResponse(
            page("Enter your email and confirmation code.", true, email),
            400,
        );
    }

    const supabase = createSupabaseClient(env, request);
    const { error } = await supabase.auth.verifyOtp({
        email,
        token,
        type: OTP_TYPE,
    });

    if (error) {
        return htmlResponse(page(error.message, true, email), 400);
    }

    return htmlResponse(page("Email confirmed. You can sign in now."));
};

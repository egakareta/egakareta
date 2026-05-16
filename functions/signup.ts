/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
import { jsonResponse, readRequestBody, validateSignupInput } from "./_auth";
import { createSupabaseClient } from "./_supabase";
import { badRequest, serverError } from "./_utils";

function escapeHtml(value: string) {
    return value.replace(
        /[&<>"]/g,
        (character) =>
            ({
                "&": "&amp;",
                "<": "&lt;",
                ">": "&gt;",
                '"': "&quot;",
            })[character] ?? character,
    );
}

function turnstileToken(body: Record<string, unknown>) {
    const token = body["cf-turnstile-response"] ?? body.turnstileToken;
    return typeof token === "string" ? token.trim() : "";
}

function page(message = "", isError = false, turnstileSiteKey = "") {
    const statusClass = isError ? "error" : "success";
    const safeMessage = escapeHtml(message);
    const safeSiteKey = escapeHtml(turnstileSiteKey);
    return `<!doctype html>
<html lang="en">
    <head>
        <meta charset="utf-8" />
        <meta name="viewport" content="width=device-width, initial-scale=1" />
        <title>signup</title>
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
            button {
                width: 100%;
                margin-top: 10px;
                padding: 12px 14px;
                border: 0;
                background: #f5f7fb;
                color: #0a0d12;
                font: inherit;
                font-weight: 700;
                cursor: pointer;
            }
            .turnstile-wrap { min-height: 65px; margin: 18px 0 6px; }
            .status { margin: 0 0 18px; padding: 11px 12px; }
            .success { background: rgba(64, 202, 140, 0.16); color: #bff7d9; }
            .error { background: rgba(255, 91, 91, 0.16); color: #ffd0d0; }
            small { color: #8f9aaa; }
        </style>
    </head>
    <body>
        <main>
            <h1>create account</h1>
            <p>confirm your email then sign in from the game</p>
            ${safeMessage ? `<div class="status ${statusClass}">${safeMessage}</div>` : ""}
            <form method="post" action="/signup">
                <label>username<input name="username" autocomplete="username" minlength="3" maxlength="24" pattern="[a-zA-Z0-9_]+" required /><small>letters, numbers, and underscores only</small></label>
                <label>email<input type="email" name="email" autocomplete="email" required /></label>
                <label>password<input type="password" name="password" autocomplete="new-password" minlength="8" required /></label>
                <div class="turnstile-wrap"><div class="cf-turnstile" data-sitekey="${safeSiteKey}"></div></div>
                <button type="submit">create account</button>
            </form>
        </main>
    </body>
</html>`;
}

function htmlResponse(body: string, status = 200) {
    return new Response(body, {
        status,
        headers: { "Content-Type": "text/html; charset=utf-8" },
    });
}

export const onRequestGet: PagesFunction<Cloudflare.Env> = async ({ env }) => {
    return htmlResponse(page("", false, env.TURNSTILE_SITE_KEY));
};

export const onRequestPost: PagesFunction<Cloudflare.Env> = async ({
    request,
    env,
}) => {
    const body = await readRequestBody(request);
    const captchaToken = turnstileToken(body);
    const validated = validateSignupInput({
        username: body.username,
        email: body.email,
        password: body.password,
    });

    const wantsJson = request.headers
        .get("Accept")
        ?.includes("application/json");

    if ("error" in validated) {
        return wantsJson
            ? badRequest(validated.error)
            : htmlResponse(
                  page(validated.error, true, env.TURNSTILE_SITE_KEY),
                  400,
              );
    }

    if (env.TURNSTILE_SITE_KEY && !captchaToken) {
        const message =
            "Complete the verification challenge before signing up.";
        return wantsJson
            ? badRequest(message)
            : htmlResponse(page(message, true, env.TURNSTILE_SITE_KEY), 400);
    }

    const supabase = createSupabaseClient(env, request);

    const { data: usernameTaken, error: usernameError } = await supabase
        .from("profiles")
        .select("id")
        .eq("username", validated.username)
        .maybeSingle();

    if (usernameError) {
        return wantsJson
            ? serverError(usernameError.message)
            : htmlResponse(
                  page(usernameError.message, true, env.TURNSTILE_SITE_KEY),
                  500,
              );
    }

    if (usernameTaken) {
        const message = "That username is already taken.";
        return wantsJson
            ? badRequest(message)
            : htmlResponse(page(message, true, env.TURNSTILE_SITE_KEY), 400);
    }

    const { data: emailExists, error: emailError } = await supabase.rpc(
        "check_if_email_exists",
        { email_to_check: validated.email },
    );

    if (emailError) {
        return wantsJson
            ? serverError(emailError.message)
            : htmlResponse(
                  page(emailError.message, true, env.TURNSTILE_SITE_KEY),
                  500,
              );
    }

    if (emailExists) {
        const message = "An account already exists for that email.";
        return wantsJson
            ? badRequest(message)
            : htmlResponse(page(message, true, env.TURNSTILE_SITE_KEY), 400);
    }

    const { error } = await supabase.auth.signUp({
        email: validated.email,
        password: validated.password,
        options: {
            captchaToken: captchaToken || undefined,
            data: { username: validated.username },
        },
    });

    if (error) {
        return wantsJson
            ? badRequest(error.message)
            : htmlResponse(
                  page(error.message, true, env.TURNSTILE_SITE_KEY),
                  400,
              );
    }

    const message =
        "Account created. Check your email to confirm it before signing in.";
    return wantsJson
        ? jsonResponse({ ok: true, message })
        : htmlResponse(page(message, false, env.TURNSTILE_SITE_KEY));
};

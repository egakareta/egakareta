/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
import {
    jsonResponse,
    readRequestBody,
    turnstileToken,
    validateSignupInput,
    verifyTurnstileToken,
} from "./_auth";
import { createSupabaseAdminClient, createSupabaseClient } from "./_supabase";
import { badRequest, serverError } from "./_utils";

type SignupInput = {
    username: string;
    email: string;
    password: string;
};

type SupabaseRequestClient = ReturnType<typeof createSupabaseClient>;

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

function page(message = "", isError = false, env: Cloudflare.Env) {
    const statusClass = isError ? "error" : "success";
    const safeMessage = escapeHtml(message);
    const safeSiteKey = escapeHtml(
        env.TURNSTILE_SITE_KEY ?? "1x00000000000000000000AA",
    );
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

function wantsJsonResponse(request: Request) {
    return request.headers.get("Accept")?.includes("application/json") ?? false;
}

function signupErrorResponse(
    message: string,
    status: number,
    wantsJson: boolean,
    env: Cloudflare.Env,
) {
    if (wantsJson) {
        return status >= 500 ? serverError(message) : badRequest(message);
    }

    return htmlResponse(page(message, true, env), status);
}

function signupSuccessResponse(
    message: string,
    wantsJson: boolean,
    env: Cloudflare.Env,
) {
    return wantsJson
        ? jsonResponse({ ok: true, message })
        : htmlResponse(page(message, false, env));
}

function validateSignupRequest(
    body: Record<string, unknown>,
    wantsJson: boolean,
    env: Cloudflare.Env,
): SignupInput | Response {
    const validated = validateSignupInput({
        username: body.username,
        email: body.email,
        password: body.password,
    });

    if ("error" in validated) {
        return signupErrorResponse(validated.error, 400, wantsJson, env);
    }

    return validated;
}

async function validateSignupCaptcha(
    request: Request,
    env: Cloudflare.Env,
    captchaToken: string,
    wantsJson: boolean,
): Promise<Response | null> {
    const captchaError = await verifyTurnstileToken(env, captchaToken, request);
    if (!captchaError) {
        return null;
    }

    return signupErrorResponse(
        captchaError ===
            "Complete the verification challenge before continuing."
            ? "Complete the verification challenge before signing up."
            : captchaError,
        400,
        wantsJson,
        env,
    );
}

async function ensureUsernameAvailable(
    supabase: SupabaseRequestClient,
    username: string,
    wantsJson: boolean,
    env: Cloudflare.Env,
): Promise<Response | null> {
    const { data, error } = await supabase
        .from("profiles")
        .select("id")
        .eq("username", username)
        .maybeSingle();

    if (error) {
        console.error("signup username availability check failed", error);
        return signupErrorResponse(
            "Sign up is temporarily unavailable. Please try again.",
            500,
            wantsJson,
            env,
        );
    }

    return data
        ? signupErrorResponse(
              "That username is already taken.",
              400,
              wantsJson,
              env,
          )
        : null;
}

async function ensureEmailAvailable(
    supabase: SupabaseRequestClient,
    email: string,
    wantsJson: boolean,
    env: Cloudflare.Env,
): Promise<Response | null> {
    const { data, error } = await supabase.rpc("check_if_email_exists", {
        email_to_check: email,
    });

    if (error) {
        console.error("signup email availability check failed", error);
        return signupErrorResponse(
            "Sign up is temporarily unavailable. Please try again.",
            500,
            wantsJson,
            env,
        );
    }

    return data
        ? signupErrorResponse(
              "An account already exists for that email.",
              400,
              wantsJson,
              env,
          )
        : null;
}

async function createSignupAccount(
    supabase: SupabaseRequestClient,
    input: SignupInput,
    captchaToken: string,
    wantsJson: boolean,
    env: Cloudflare.Env,
): Promise<Response | null> {
    const { error } = await supabase.auth.signUp({
        email: input.email,
        password: input.password,
        options: {
            captchaToken: captchaToken || undefined,
            data: { username: input.username },
        },
    });

    return error
        ? signupErrorResponse(error.message, 400, wantsJson, env)
        : null;
}

function isResponse(value: unknown): value is Response {
    return value instanceof Response;
}

export const onRequestGet: PagesFunction<Cloudflare.Env> = async ({ env }) => {
    return htmlResponse(page("", false, env));
};

export const onRequestPost: PagesFunction<Cloudflare.Env> = async ({
    request,
    env,
}) => {
    const body = await readRequestBody(request);
    const captchaToken = turnstileToken(body);
    const wantsJson = wantsJsonResponse(request);
    const input = validateSignupRequest(body, wantsJson, env);
    if (isResponse(input)) return input;

    const captchaError = await validateSignupCaptcha(
        request,
        env,
        captchaToken,
        wantsJson,
    );
    if (captchaError) return captchaError;

    const supabase = createSupabaseClient(env, request);
    const admin = createSupabaseAdminClient(env);
    const usernameError = await ensureUsernameAvailable(
        supabase,
        input.username,
        wantsJson,
        env,
    );
    if (usernameError) return usernameError;

    const emailError = await ensureEmailAvailable(
        admin,
        input.email,
        wantsJson,
        env,
    );
    if (emailError) return emailError;

    const signupError = await createSignupAccount(
        supabase,
        input,
        captchaToken,
        wantsJson,
        env,
    );
    if (signupError) return signupError;

    const message =
        "Account created. Check your email to confirm it before signing in.";
    return signupSuccessResponse(message, wantsJson, env);
};

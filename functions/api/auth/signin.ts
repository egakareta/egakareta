/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
import {
    buildAuthPayload,
    isUnconfirmedEmailError,
    jsonResponse,
    normalizeIdentifier,
    readRequestBody,
    resolveIdentifierToEmail,
    turnstileToken,
    verifyTurnstileToken,
} from "../../_auth";
import {
    createSupabaseAdminClient,
    createSupabaseClient,
} from "../../_supabase";
import { badRequest, serverError, unauthorized } from "../../_utils";

export const onRequestPost: PagesFunction<Cloudflare.Env> = async ({
    request,
    env,
}) => {
    const body = await readRequestBody(request);
    const identifier = normalizeIdentifier(body.identifier);
    const password = typeof body.password === "string" ? body.password : "";
    const captchaToken = turnstileToken(body);

    if (!identifier || !password) {
        return badRequest("Enter your username or email and password.");
    }
    const captchaError = await verifyTurnstileToken(env, captchaToken, request);
    if (captchaError) {
        return badRequest(
            captchaError ===
                "Complete the verification challenge before continuing."
                ? "Complete the verification challenge before signing in."
                : captchaError,
        );
    }

    const supabase = createSupabaseClient(env, request);
    const admin = createSupabaseAdminClient(env);
    let email: string | null;

    try {
        email = await resolveIdentifierToEmail(admin, identifier);
    } catch (error) {
        const message =
            error instanceof Error ? error.message : "Login lookup failed.";
        return serverError(message);
    }

    if (!email) {
        return unauthorized("Invalid login credentials.");
    }

    const { data, error } = await supabase.auth.signInWithPassword({
        email,
        password,
        options: {
            captchaToken: captchaToken || undefined,
        },
    });

    if (error) {
        if (isUnconfirmedEmailError(error.message)) {
            return jsonResponse(
                {
                    error: "Check your email to confirm this account before signing in.",
                    code: "email_not_confirmed",
                },
                { status: 403 },
            );
        }
        return unauthorized(error.message);
    }

    if (!data.session || !data.user) {
        return unauthorized("Sign-in did not return a usable session.");
    }

    try {
        return jsonResponse(
            await buildAuthPayload(supabase, data.user, data.session),
        );
    } catch (error) {
        const message =
            error instanceof Error ? error.message : "Profile lookup failed.";
        return serverError(message);
    }
};

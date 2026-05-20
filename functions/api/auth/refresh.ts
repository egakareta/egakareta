/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
import { buildAuthPayload, jsonResponse, readRequestBody } from "../../_auth";
import { createSupabaseClient } from "../../_supabase";
import { badRequest, serverError, unauthorized } from "../../_utils";

export const onRequestPost: PagesFunction<Cloudflare.Env> = async ({
    request,
    env,
}) => {
    const body = await readRequestBody(request);
    const refreshToken =
        typeof body.refresh_token === "string" ? body.refresh_token : "";

    if (!refreshToken) {
        return badRequest("Missing refresh token.");
    }

    const supabase = createSupabaseClient(env, request);
    const { data, error } = await supabase.auth.refreshSession({
        refresh_token: refreshToken,
    });

    if (error) {
        return unauthorized(error.message);
    }

    if (!data.session || !data.user) {
        return unauthorized("Refresh did not return a usable session.");
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

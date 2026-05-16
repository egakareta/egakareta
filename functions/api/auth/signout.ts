/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
import { jsonResponse } from "../../_auth";
import { createSupabaseClient } from "../../_supabase";
import { serverError } from "../../_utils";

export const onRequestPost: PagesFunction<Cloudflare.Env> = async ({
    request,
    env,
}) => {
    const supabase = createSupabaseClient(env, request);
    const { error } = await supabase.auth.signOut();

    if (error) {
        return serverError(error.message);
    }

    return jsonResponse({ ok: true });
};

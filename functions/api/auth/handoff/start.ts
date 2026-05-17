/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
import { jsonResponse } from "../../../_auth";
import { createSupabaseAdminClient } from "../../../_supabase";
import { serverError } from "../../../_utils";

function randomSecret() {
    const bytes = new Uint8Array(32);
    crypto.getRandomValues(bytes);
    let binary = "";
    for (const byte of bytes) {
        binary += String.fromCharCode(byte);
    }
    return btoa(binary)
        .replace(/\+/g, "-")
        .replace(/\//g, "_")
        .replace(/=+$/g, "");
}

async function sha256Hex(value: string) {
    const digest = await crypto.subtle.digest(
        "SHA-256",
        new TextEncoder().encode(value),
    );
    return Array.from(new Uint8Array(digest), (byte) =>
        byte.toString(16).padStart(2, "0"),
    ).join("");
}

export const onRequestPost: PagesFunction<Cloudflare.Env> = async ({
    request,
    env,
}) => {
    const handoffId = crypto.randomUUID();
    const handoffSecret = randomSecret();
    const secretHash = await sha256Hex(handoffSecret);
    const supabase = createSupabaseAdminClient(env);
    const { error } = await supabase.from("auth_handoffs").insert({
        id: handoffId,
        secret_hash: secretHash,
    });

    if (error) {
        return serverError(error.message);
    }

    const signinUrl = new URL("/signin", request.url);
    signinUrl.searchParams.set("handoff_id", handoffId);

    return jsonResponse({
        handoff_id: handoffId,
        handoff_secret: handoffSecret,
        signin_url: signinUrl.toString(),
    });
};

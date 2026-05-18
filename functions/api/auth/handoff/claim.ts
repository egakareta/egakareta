/*

* Copyright (c) egakareta <team@egakareta.com>.
* Licensed under the GNU AGPLv3 or a proprietary Commercial License.
* See LICENSE and COMMERCIAL.md for details.

*/
import { jsonResponse, readRequestBody } from "../../../_auth";
import { createSupabaseAdminClient } from "../../../_supabase";
import { badRequest, serverError, unauthorized } from "../../../_utils";

const UUID_PATTERN =
    /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;

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
    const body = await readRequestBody(request);
    const handoffId =
        typeof body.handoff_id === "string" ? body.handoff_id : "";
    const handoffSecret =
        typeof body.handoff_secret === "string" ? body.handoff_secret : "";

    if (!UUID_PATTERN.test(handoffId) || !handoffSecret) {
        return badRequest("Missing or invalid sign-in handoff.");
    }

    const supabase = createSupabaseAdminClient(env);
    const { data, error } = await supabase
        .from("auth_handoffs")
        .select("id, secret_hash, auth_payload, expires_at, claimed_at")
        .eq("id", handoffId)
        .maybeSingle();

    if (error) {
        return serverError(error.message);
    }
    if (!data) {
        return unauthorized("Sign-in handoff was not found.");
    }
    if (data.claimed_at || Date.parse(data.expires_at) <= Date.now()) {
        return jsonResponse(
            { error: "Sign-in handoff has expired." },
            { status: 410 },
        );
    }

    const secretHash = await sha256Hex(handoffSecret);
    if (secretHash !== data.secret_hash) {
        return unauthorized("Sign-in handoff secret did not match.");
    }

    if (!data.auth_payload) {
        return jsonResponse({ pending: true }, { status: 202 });
    }

    const { data: claimedPayload, error: updateError } = await supabase.rpc(
        "claim_auth_handoff",
        {
            claim_id: handoffId,
            claim_secret_hash: secretHash,
        },
    );

    if (updateError) {
        return serverError(updateError.message);
    }

    if (!claimedPayload) {
        return jsonResponse(
            { error: "Sign-in handoff has expired." },
            { status: 410 },
        );
    }

    return jsonResponse(claimedPayload);
};

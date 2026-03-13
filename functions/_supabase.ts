import { createClient } from "@supabase/supabase-js";

/**
 * Creates a Supabase client instance for use in Cloudflare Workers.
 * This function reads the Supabase URL and anon key from environment variables,
 * and optionally includes the Authorization header from the incoming request.
 *
 * This allows you to authenticate as the user making the request if they have a valid session cookie or token.
 *
 * @param env The Cloudflare environment object containing environment variables.
 * @param request The incoming request object, used to extract the Authorization header if present.
 * @returns A Supabase client instance configured for use in Cloudflare Workers.
 * @throws An error if the required Supabase environment variables are missing.
 */
export function createSupabaseClient(env: Cloudflare.Env, request: Request) {
    const supabaseUrl = env.API_URL;
    const supabaseKey = env.PUBLISHABLE_KEY;

    if (!supabaseUrl || !supabaseKey) {
        throw new Error("Missing Supabase environment variables");
    }

    // In Cloudflare Workers, we can pass the authorization header manually if needed,
    // or use the cookies from the request.
    const authHeader = request.headers.get("Authorization");

    return createClient(supabaseUrl, supabaseKey, {
        global: {
            headers: authHeader ? { Authorization: authHeader } : undefined,
        },
    });
}

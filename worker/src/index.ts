export interface Env {
  RAILWAY_ORIGIN: string;
  ORIGIN_SECRET: string;
}

const SECURITY_HEADERS: Record<string, string> = {
  "X-Content-Type-Options": "nosniff",
  "Referrer-Policy": "strict-origin-when-cross-origin",
  "X-Frame-Options": "DENY",
};

export default {
  async fetch(
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ): Promise<Response> {
    const configError = validateConfig(env);
    if (configError) {
      return withSecurityHeaders(
        new Response(configError, {
          status: 500,
          headers: { "Content-Type": "text/plain; charset=utf-8" },
        }),
      );
    }

    const url = new URL(request.url);
    const origin = new URL(env.RAILWAY_ORIGIN);
    const upstreamUrl = new URL(url.pathname + url.search, origin);
    const cacheable = isCacheablePublicGet(request, url);
    const cache = caches.default;

    if (cacheable) {
      const cached = await cache.match(request);
      if (cached) {
        return withSecurityHeaders(cached);
      }
    }

    const headers = new Headers(request.headers);
    headers.set("X-Origin-Secret", env.ORIGIN_SECRET);
    headers.set("Host", origin.host);

    const upstreamRequest = new Request(upstreamUrl, {
      method: request.method,
      headers,
      body: request.body,
      redirect: "manual",
    });
    const upstreamResponse = await fetch(upstreamRequest);
    const response = withSecurityHeaders(upstreamResponse);

    if (cacheable && response.ok) {
      const cachedResponse = new Response(response.clone().body, response);
      cachedResponse.headers.set("Cache-Control", "public, max-age=60");
      ctx.waitUntil(cache.put(request, cachedResponse));
    }

    return response;
  },
};

function validateConfig(env: Env): string | null {
  if (!env.RAILWAY_ORIGIN) {
    return "Worker misconfigured: RAILWAY_ORIGIN is missing.";
  }
  if (!env.ORIGIN_SECRET) {
    return "Worker misconfigured: ORIGIN_SECRET is missing.";
  }

  try {
    const origin = new URL(env.RAILWAY_ORIGIN);
    if (origin.protocol !== "https:") {
      return "Worker misconfigured: RAILWAY_ORIGIN must start with https://.";
    }
  } catch {
    return "Worker misconfigured: RAILWAY_ORIGIN is not a valid URL.";
  }

  return null;
}

function isCacheablePublicGet(request: Request, url: URL): boolean {
  if (request.method !== "GET") {
    return false;
  }
  if (url.pathname.startsWith("/auth/")) {
    return false;
  }
  if (url.pathname === "/api/me" || url.pathname.startsWith("/api/admin/")) {
    return false;
  }
  return (
    url.pathname.startsWith("/api/courses") ||
    url.pathname.startsWith("/api/offerings") ||
    url.pathname.startsWith("/api/sections")
  );
}

function withSecurityHeaders(response: Response): Response {
  const next = new Response(response.body, response);
  for (const [name, value] of Object.entries(SECURITY_HEADERS)) {
    next.headers.set(name, value);
  }
  return next;
}

export interface Env {
  RAILWAY_ORIGIN: string;
  ORIGIN_SECRET: string;
}

type CachePolicy = {
  keyVersion: string;
  cacheControl: string;
};

const SECURITY_HEADERS: Record<string, string> = {
  "X-Content-Type-Options": "nosniff",
  "Referrer-Policy": "strict-origin-when-cross-origin",
  "X-Frame-Options": "DENY",
};

const STATIC_ASSET_CACHE: CachePolicy = {
  keyVersion: "assets-v1",
  cacheControl: "public, max-age=31536000, immutable",
};

const PUBLIC_API_CACHE: CachePolicy = {
  keyVersion: "public-api-v2",
  cacheControl: "public, max-age=30, stale-while-revalidate=120",
};

export default {
  async fetch(
    request: Request,
    env: Env,
    ctx: ExecutionContext,
  ): Promise<Response> {
    const configError = validateConfig(env);
    if (configError) {
      return finalizeResponse(
        new Response(configError, {
          status: 500,
          headers: { "Content-Type": "text/plain; charset=utf-8" },
        }),
        { cacheStatus: "BYPASS" },
      );
    }

    const url = new URL(request.url);
    const origin = new URL(env.RAILWAY_ORIGIN);
    const upstreamUrl = new URL(url.pathname + url.search, origin);
    const cachePolicy = cachePolicyFor(request, url);
    const cacheKey = cachePolicy
      ? buildCacheKey(request, url, cachePolicy)
      : null;
    const cache = caches.default;

    if (cacheKey && cachePolicy) {
      const cached = await cache.match(cacheKey);
      if (cached) {
        return finalizeResponse(cached, {
          cacheControl: cachePolicy.cacheControl,
          cacheStatus: "HIT",
        });
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
    const response = finalizeResponse(upstreamResponse, {
      cacheControl: cachePolicy?.cacheControl,
      cacheStatus: cachePolicy ? "MISS" : "BYPASS",
    });

    if (
      cacheKey &&
      cachePolicy &&
      response.ok &&
      !response.headers.has("Set-Cookie")
    ) {
      const cachedResponse = new Response(response.clone().body, response);
      cachedResponse.headers.set("Cache-Control", cachePolicy.cacheControl);
      cachedResponse.headers.delete("X-Open-Ntu-Cache");
      ctx.waitUntil(cache.put(cacheKey, cachedResponse));
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

function cachePolicyFor(request: Request, url: URL): CachePolicy | null {
  if (request.method !== "GET") {
    return null;
  }

  if (url.pathname.startsWith("/assets/")) {
    return STATIC_ASSET_CACHE;
  }

  if (url.pathname.startsWith("/auth/")) {
    return null;
  }
  if (url.pathname === "/api/me" || url.pathname.startsWith("/api/admin/")) {
    return null;
  }

  if (request.headers.has("Cookie")) {
    return null;
  }

  if (
    url.pathname.startsWith("/api/courses") ||
    url.pathname.startsWith("/api/offerings") ||
    url.pathname.startsWith("/api/sections")
  ) {
    return PUBLIC_API_CACHE;
  }

  return null;
}

function buildCacheKey(
  request: Request,
  url: URL,
  policy: CachePolicy,
): Request {
  const cacheUrl = new URL(url);
  cacheUrl.searchParams.set("__open_ntu_cache", policy.keyVersion);
  return new Request(cacheUrl.toString(), {
    method: request.method,
    headers: {
      Accept: request.headers.get("Accept") ?? "",
    },
  });
}

function finalizeResponse(
  response: Response,
  options: { cacheControl?: string; cacheStatus: "HIT" | "MISS" | "BYPASS" },
): Response {
  const next = new Response(response.body, response);
  if (options.cacheControl) {
    next.headers.set("Cache-Control", options.cacheControl);
  }
  next.headers.set("X-Open-Ntu-Cache", options.cacheStatus);
  for (const [name, value] of Object.entries(SECURITY_HEADERS)) {
    next.headers.set(name, value);
  }
  return next;
}

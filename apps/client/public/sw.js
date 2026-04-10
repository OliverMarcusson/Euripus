const SHELL_CACHE_NAME = "euripus-shell-v3";
const CHANNEL_ICON_CACHE_NAME = "euripus-channel-icons-v2";
const CHANNEL_ICON_METADATA_CACHE_NAME = "euripus-channel-icon-metadata-v1";
const OFFLINE_URL = "/offline.html";
const STATIC_ASSETS = [
  "/",
  OFFLINE_URL,
  "/manifest.webmanifest",
  "/icon.svg",
  "/icon-192.png",
  "/icon-512.png",
];
const IMAGE_CACHE_TTL_MS = 24 * 60 * 60 * 1000;
const IMAGE_CACHE_MAX_ENTRIES = 500;

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches.open(SHELL_CACHE_NAME).then((cache) => cache.addAll(STATIC_ASSETS)).then(() => self.skipWaiting()),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(
        keys
          .filter(
            (key) =>
              key !== SHELL_CACHE_NAME
              && key !== CHANNEL_ICON_CACHE_NAME
              && key !== CHANNEL_ICON_METADATA_CACHE_NAME,
          )
          .map((key) => caches.delete(key)),
      ),
    ).then(() => self.clients.claim()),
  );
});

self.addEventListener("fetch", (event) => {
  const request = event.request;
  const url = new URL(request.url);

  if (request.method !== "GET") {
    return;
  }

  if (request.mode === "navigate") {
    event.respondWith(handleNavigationRequest(request));
    return;
  }

  if (shouldHandleLogoImageRequest(request, url)) {
    event.respondWith(handleLogoImageRequest(event, request, url));
    return;
  }

  if (url.origin !== self.location.origin) {
    return;
  }

  if (isImmutableStaticAsset(url)) {
    event.respondWith(handleImmutableStaticAsset(request));
    return;
  }

  if (isShellAsset(url)) {
    event.respondWith(handleShellAsset(request));
  }
});

async function handleNavigationRequest(request) {
  try {
    const response = await fetch(request);
    if (response.ok) {
      await cacheShellAsset(request, response.clone());
    }
    return response;
  } catch {
    return caches.match(OFFLINE_URL);
  }
}

async function handleImmutableStaticAsset(request) {
  const cachedResponse = await caches.match(request);
  if (cachedResponse) {
    return cachedResponse;
  }

  const response = await fetch(request);
  if (response.ok) {
    await cacheShellAsset(request, response.clone());
  }
  return response;
}

async function handleShellAsset(request) {
  try {
    const response = await fetch(request);
    if (response.ok) {
      await cacheShellAsset(request, response.clone());
    }
    return response;
  } catch {
    const cachedResponse = await caches.match(request);
    if (cachedResponse) {
      return cachedResponse;
    }

    return errorResponse("Shell asset unavailable.");
  }
}

function shouldHandleLogoImageRequest(request, url) {
  if (request.destination !== "image") {
    return false;
  }

  if (url.origin === self.location.origin) {
    return url.pathname === "/api/relay/asset";
  }

  return url.protocol === "http:" || url.protocol === "https:";
}

async function handleLogoImageRequest(event, request, url) {
  const cache = await caches.open(CHANNEL_ICON_CACHE_NAME);
  const metadataCache = await caches.open(CHANNEL_ICON_METADATA_CACHE_NAME);
  const cacheKey = getImageCacheKey(request, url);
  const cachedResponse = await cache.match(cacheKey);
  const cachedAt = await readCachedAt(metadataCache, cacheKey);
  const cacheExpired =
    cachedResponse
    && (cachedAt === null || Date.now() - cachedAt > IMAGE_CACHE_TTL_MS);

  if (cachedResponse && !cacheExpired) {
    event.waitUntil(refreshImageCache(cache, metadataCache, cacheKey, request).catch(() => undefined));
    return cachedResponse;
  }

  try {
    const response = await fetch(request);
    if (isCacheableImageResponse(response)) {
      event.waitUntil(storeImageCacheEntry(cache, metadataCache, cacheKey, response.clone()));
    }
    return response;
  } catch (error) {
    if (cachedResponse) {
      return cachedResponse;
    }

    throw error;
  }
}

async function refreshImageCache(cache, metadataCache, cacheKey, request) {
  const response = await fetch(request);
  if (!isCacheableImageResponse(response)) {
    return;
  }

  await storeImageCacheEntry(cache, metadataCache, cacheKey, response.clone());
}

async function storeImageCacheEntry(cache, metadataCache, cacheKey, response) {
  await cache.put(cacheKey, response);
  await metadataCache.put(
    metadataKeyFor(cacheKey),
    new Response(JSON.stringify({ cachedAt: Date.now() }), {
      headers: { "Content-Type": "application/json" },
    }),
  );
  await trimImageCache(cache, metadataCache);
}

async function trimImageCache(cache, metadataCache) {
  const keys = await cache.keys();
  if (keys.length <= IMAGE_CACHE_MAX_ENTRIES) {
    return;
  }

  const entries = await Promise.all(
    keys.map(async (key) => ({
      key,
      cachedAt: (await readCachedAt(metadataCache, key)) ?? 0,
    })),
  );

  await Promise.all(
    entries
      .sort((left, right) => left.cachedAt - right.cachedAt)
      .slice(0, Math.max(0, entries.length - IMAGE_CACHE_MAX_ENTRIES))
      .flatMap(({ key }) => [
        cache.delete(key),
        metadataCache.delete(metadataKeyFor(key)),
      ]),
  );
}

async function readCachedAt(metadataCache, cacheKey) {
  const metadataResponse = await metadataCache.match(metadataKeyFor(cacheKey));
  if (!metadataResponse) {
    return null;
  }

  try {
    const payload = await metadataResponse.json();
    return typeof payload.cachedAt === "number" ? payload.cachedAt : null;
  } catch {
    return null;
  }
}

async function cacheShellAsset(request, response) {
  const cache = await caches.open(SHELL_CACHE_NAME);
  await cache.put(request, response);
}

function isCacheableImageResponse(response) {
  return response.ok || response.type === "opaque";
}

function isImmutableStaticAsset(url) {
  return url.pathname.startsWith("/assets/");
}

function isShellAsset(url) {
  return (
    url.pathname === OFFLINE_URL
    || url.pathname === "/manifest.webmanifest"
    || url.pathname === "/sw.js"
    || url.pathname === "/icon.svg"
    || url.pathname === "/icon-192.png"
    || url.pathname === "/icon-512.png"
    || url.pathname.endsWith(".html")
  );
}

function getImageCacheKey(request, url) {
  if (url.origin === self.location.origin && url.pathname === "/api/relay/asset") {
    const upstreamUrl = extractRelayAssetUpstreamUrl(url);
    if (upstreamUrl) {
      return `${self.location.origin}/__channel_icon_cache__/${hashString(upstreamUrl)}`;
    }
  }

  return request;
}

function metadataKeyFor(cacheKey) {
  return `${normalizeCacheKey(cacheKey)}::meta`;
}

function normalizeCacheKey(cacheKey) {
  if (typeof cacheKey === "string") {
    return cacheKey;
  }

  return cacheKey.url;
}

function extractRelayAssetUpstreamUrl(url) {
  const token = url.searchParams.get("token");
  if (!token) {
    return null;
  }

  const payload = decodeJwtPayload(token);
  if (!payload || typeof payload.url !== "string") {
    return null;
  }

  return payload.url;
}

function decodeJwtPayload(token) {
  const parts = token.split(".");
  if (parts.length < 2) {
    return null;
  }

  try {
    const normalized = parts[1].replace(/-/g, "+").replace(/_/g, "/");
    const padding = "=".repeat((4 - (normalized.length % 4)) % 4);
    return JSON.parse(atob(normalized + padding));
  } catch {
    return null;
  }
}

function hashString(value) {
  let hash = 2166136261;

  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }

  return (hash >>> 0).toString(16);
}

function errorResponse(message) {
  return new Response(message, {
    status: 503,
    headers: { "Content-Type": "text/plain" },
  });
}

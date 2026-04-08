const SHELL_CACHE_NAME = "euripus-shell-v2";
const CHANNEL_ICON_CACHE_NAME = "euripus-channel-icons-v1";
const OFFLINE_URL = "/offline.html";
const STATIC_ASSETS = ["/", OFFLINE_URL, "/manifest.webmanifest", "/icon.svg", "/icon-192.png", "/icon-512.png"];

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
          .filter((key) => key !== SHELL_CACHE_NAME && key !== CHANNEL_ICON_CACHE_NAME)
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
    event.respondWith(
      fetch(request).catch(() => caches.match(OFFLINE_URL)),
    );
    return;
  }

  if (shouldHandleImageRequest(request, url)) {
    event.respondWith(handleImageRequest(event, request, url));
    return;
  }

  if (url.origin !== self.location.origin) {
    return;
  }

  const isStaticAsset =
    url.pathname.endsWith(".js") ||
    url.pathname.endsWith(".css") ||
    url.pathname.endsWith(".html") ||
    url.pathname.endsWith(".svg") ||
    url.pathname.endsWith(".png") ||
    url.pathname.endsWith(".webmanifest") ||
    url.pathname.endsWith(".woff2");

  if (!isStaticAsset) {
    return;
  }

  event.respondWith(
    caches.match(request).then((cachedResponse) => {
      if (cachedResponse) {
        return cachedResponse;
      }

      return fetch(request).then((response) => {
        const copy = response.clone();
        caches.open(SHELL_CACHE_NAME).then((cache) => cache.put(request, copy));
        return response;
      });
    }),
  );
});

function shouldHandleImageRequest(request, url) {
  return request.destination === "image" && (url.protocol === "http:" || url.protocol === "https:");
}

async function handleImageRequest(event, request, url) {
  const cache = await caches.open(CHANNEL_ICON_CACHE_NAME);
  const cacheKey = getImageCacheKey(request, url);
  const cachedResponse = await cache.match(cacheKey);
  const networkResponsePromise = fetch(request).then((response) => {
    if (isCacheableImageResponse(response)) {
      event.waitUntil(cache.put(cacheKey, response.clone()).catch(() => undefined));
    }

    return response;
  });

  if (cachedResponse) {
    event.waitUntil(networkResponsePromise.catch(() => undefined));
    return cachedResponse;
  }

  return networkResponsePromise.catch((error) => {
    if (cachedResponse) {
      return cachedResponse;
    }

    throw error;
  });
}

function isCacheableImageResponse(response) {
  return response.ok || response.type === "opaque";
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

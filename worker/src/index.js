const encoder = new TextEncoder();

function json(body, status = 200, headers = {}) {
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      "Cache-Control": "no-store",
      "Content-Type": "application/json; charset=utf-8",
      ...headers,
    },
  });
}

function errorMessage(body) {
  return (
    body?.message ??
    body?.error?.message ??
    body?.error ??
    "Unable to connect the iCloud account."
  );
}

function base64Url(bytes) {
  return btoa(String.fromCharCode(...bytes))
    .replaceAll("+", "-")
    .replaceAll("/", "_")
    .replace(/=+$/, "");
}

function decodeBase64Url(value) {
  const padded = value
    .replaceAll("-", "+")
    .replaceAll("_", "/")
    .padEnd(Math.ceil(value.length / 4) * 4, "=");
  return Uint8Array.from(atob(padded), (character) => character.charCodeAt(0));
}

async function tokenKey(secret) {
  return crypto.subtle.importKey(
    "raw",
    encoder.encode(`shitou-icloud-mail-v1:${secret}`),
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign", "verify"],
  );
}

async function createAccessToken(grantId, secret) {
  const payload = base64Url(encoder.encode(JSON.stringify({ v: 1, grantId })));
  const signature = await crypto.subtle.sign(
    "HMAC",
    await tokenKey(secret),
    encoder.encode(payload),
  );
  return `${payload}.${base64Url(new Uint8Array(signature))}`;
}

async function grantIdFromRequest(request, secret) {
  const token = request.headers
    .get("Authorization")
    ?.match(/^Bearer (.+)$/)?.[1];
  if (!token) return null;
  const [payload, signature, extra] = token.split(".");
  if (!payload || !signature || extra) return null;
  try {
    const valid = await crypto.subtle.verify(
      "HMAC",
      await tokenKey(secret),
      decodeBase64Url(signature),
      encoder.encode(payload),
    );
    if (!valid) return null;
    const decoded = JSON.parse(
      new TextDecoder().decode(decodeBase64Url(payload)),
    );
    return decoded.v === 1 && typeof decoded.grantId === "string"
      ? decoded.grantId
      : null;
  } catch {
    return null;
  }
}

async function nylas(env, fetcher, path, init = {}) {
  const response = await fetcher(
    `${(env.NYLAS_API_URI || "https://api.us.nylas.com").replace(/\/$/, "")}${path}`,
    {
      ...init,
      headers: {
        Accept: "application/json",
        Authorization: `Bearer ${env.NYLAS_API_KEY}`,
        ...init.headers,
      },
    },
  );
  const body = await response.json().catch(() => ({}));
  if (!response.ok)
    throw { status: response.status, message: errorMessage(body) };
  return body.data;
}

function address(person) {
  if (!person) return "";
  if (typeof person === "string") return person;
  return person.name
    ? `${person.name} <${person.email}>`
    : (person.email ?? "");
}

function normalizeMessage(message, folderId) {
  return {
    id: message.id,
    folderId: folderId ?? message.folders?.[0] ?? "",
    sender: address(message.from?.[0]),
    recipients: (message.to ?? []).map(address).filter(Boolean),
    subject: message.subject ?? "(No subject)",
    preview: message.snippet ?? "",
    receivedAt: new Date(Number(message.date ?? 0) * 1000).toISOString(),
    hasAttachments: Boolean(message.attachments?.length),
    isUnread: Boolean(message.unread),
  };
}

async function connect(request, env, fetcher) {
  if (request.method !== "POST")
    return json({ error: "Method not allowed" }, 405, { Allow: "POST" });

  let input;
  try {
    input = await request.json();
  } catch {
    return json({ error: "Request body must be JSON" }, 400);
  }

  const email = typeof input.email === "string" ? input.email.trim() : "";
  const appPassword =
    typeof input.appPassword === "string" ? input.appPassword.trim() : "";
  if (!email.includes("@") || email.length > 254)
    return json({ error: "Enter a valid iCloud email address" }, 400);
  if (!appPassword || appPassword.length > 128)
    return json({ error: "Enter an Apple app-specific password" }, 400);

  // ponytail: public while desktop sign-in is skipped; add verified user tokens before production.
  let response;
  try {
    response = await fetcher(
      `${(env.NYLAS_API_URI || "https://api.us.nylas.com").replace(/\/$/, "")}/v3/connect/custom`,
      {
        method: "POST",
        headers: {
          Accept: "application/json",
          Authorization: `Bearer ${env.NYLAS_API_KEY}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({
          provider: "icloud",
          settings: { username: email, password: appPassword },
          scope: ["email.modify"],
        }),
      },
    );
  } catch {
    return json({ error: "Nylas is temporarily unavailable" }, 502);
  }

  const body = await response.json().catch(() => ({}));
  if (!response.ok) return json({ error: errorMessage(body) }, response.status);
  if (!body?.data?.id)
    return json({ error: "Nylas response omitted the grant ID" }, 502);

  return json({
    grantId: body.data.id,
    email: body.data.email ?? email,
    accessToken: await createAccessToken(body.data.id, env.NYLAS_API_KEY),
  });
}

async function syncMailbox(request, env, fetcher, grantId) {
  if (request.method !== "GET")
    return json({ error: "Method not allowed" }, 405, { Allow: "GET" });
  try {
    const rawFolders = await nylas(
      env,
      fetcher,
      `/v3/grants/${encodeURIComponent(grantId)}/folders?limit=200`,
    );
    const systemFolderNames = {
      "\\Inbox": "Inbox",
      "\\Trash": "Trash",
      "\\Junk": "Spam",
      "\\Sent": "Sent",
      "\\Drafts": "Drafts",
      "\\Archive": "Archive",
    };
    const commonFolderNames = {
      "deleted messages": "Trash",
      刪除的郵件: "Trash",
      junk: "Spam",
      垃圾郵件: "Spam",
      "sent messages": "Sent",
      寄件備份: "Sent",
    };
    const folders = (rawFolders ?? []).map((folder) => ({
      id: folder.id,
      name:
        Object.entries(systemFolderNames).find(([attribute]) =>
          folder.attributes?.includes(attribute),
        )?.[1] ??
        commonFolderNames[folder.name.toLowerCase()] ??
        folder.name,
      unreadCount: Number(folder.unread_count ?? 0),
      attributes: folder.attributes ?? [],
    }));
    const inbox = folders.find(
      (folder) =>
        folder.attributes.includes("\\Inbox") ||
        folder.name.toLowerCase() === "inbox",
    );
    const recent = await nylas(
      env,
      fetcher,
      `/v3/grants/${encodeURIComponent(grantId)}/messages?limit=50`,
    );
    const inboxMessages = inbox
      ? await nylas(
          env,
          fetcher,
          `/v3/grants/${encodeURIComponent(grantId)}/messages?in=${encodeURIComponent(inbox.id)}&limit=20&query_imap=true`,
        )
      : [];
    const messages = new Map(
      (recent ?? []).map((message) => [message.id, normalizeMessage(message)]),
    );
    for (const message of inboxMessages ?? [])
      messages.set(message.id, normalizeMessage(message, inbox.id));
    return json({
      folders: folders.map(({ attributes, ...folder }) => folder),
      messages: [...messages.values()],
    });
  } catch (error) {
    return json(
      { error: error?.message ?? "Nylas is temporarily unavailable" },
      error?.status ?? 502,
    );
  }
}

async function getMessage(request, env, fetcher, grantId, messageId) {
  if (request.method === "DELETE") {
    const hardDelete = new URL(request.url).searchParams.get("hard") === "true";
    try {
      const messagePath = `/v3/grants/${encodeURIComponent(grantId)}/messages/${encodeURIComponent(messageId)}`;
      if (hardDelete) {
        await nylas(env, fetcher, `${messagePath}?hard_delete=true`, {
          method: "DELETE",
        });
      } else {
        const folders = await nylas(
          env,
          fetcher,
          `/v3/grants/${encodeURIComponent(grantId)}/folders?limit=200`,
        );
        const trash = folders?.find(
          (folder) =>
            folder.attributes?.includes("\\Trash") ||
            ["trash", "deleted messages", "刪除的郵件"].includes(
              folder.name?.toLowerCase(),
            ),
        );
        if (!trash) throw { status: 409, message: "Trash folder not found" };
        await nylas(env, fetcher, messagePath, {
          method: "PUT",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ folders: [trash.id] }),
        });
      }
      return json({ deleted: true });
    } catch (error) {
      if (hardDelete && error?.status === 404) return json({ deleted: true });
      return json(
        { error: error?.message ?? "Nylas is temporarily unavailable" },
        error?.status ?? 502,
      );
    }
  }
  if (request.method !== "GET")
    return json({ error: "Method not allowed" }, 405, {
      Allow: "GET, DELETE",
    });
  try {
    const message = await nylas(
      env,
      fetcher,
      `/v3/grants/${encodeURIComponent(grantId)}/messages/${encodeURIComponent(messageId)}`,
    );
    const bodyHtml = message.body ?? "";
    return json({
      bodyHtml,
      bodyText: bodyHtml
        .replace(/<[^>]*>/g, " ")
        .replace(/\s+/g, " ")
        .trim(),
      attachments: (message.attachments ?? []).map((attachment) => ({
        id: attachment.id,
        fileName: attachment.filename ?? "attachment",
        mimeType: attachment.content_type ?? "application/octet-stream",
        byteSize: Number(attachment.size ?? 0),
      })),
    });
  } catch (error) {
    return json(
      { error: error?.message ?? "Nylas is temporarily unavailable" },
      error?.status ?? 502,
    );
  }
}

export async function handleRequest(request, env, fetcher = fetch) {
  if (!env.NYLAS_API_KEY)
    return json({ error: "Worker is missing NYLAS_API_KEY" }, 500);
  const url = new URL(request.url);
  if (url.pathname === "/icloud/connect") return connect(request, env, fetcher);

  const grantId = await grantIdFromRequest(request, env.NYLAS_API_KEY);
  if (!grantId) return json({ error: "Unauthorized" }, 401);
  if (url.pathname === "/icloud/sync")
    return syncMailbox(request, env, fetcher, grantId);
  const messageId = url.pathname.match(/^\/icloud\/messages\/(.+)$/)?.[1];
  if (messageId)
    return getMessage(
      request,
      env,
      fetcher,
      grantId,
      decodeURIComponent(messageId),
    );
  return json({ error: "Not found" }, 404);
}

export default {
  fetch(request, env) {
    return handleRequest(request, env);
  },
};

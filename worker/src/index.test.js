import assert from "node:assert/strict";
import test from "node:test";
import { handleRequest } from "./index.js";

test("creates a mail grant and uses its signed token to sync and delete", async () => {
  const calls = [];
  const fetcher = async (url, init = {}) => {
    calls.push({ url, init });
    if (url.endsWith("/v3/connect/custom"))
      return Response.json({
        data: { id: "grant-123", email: "reader@icloud.com" },
      });
    if (url.includes("/folders?"))
      return Response.json({
        data: [
          {
            id: "inbox-id",
            name: "Inbox",
            attributes: ["\\Inbox"],
            unread_count: 1,
          },
          {
            id: "trash-id",
            name: "Deleted Messages",
            attributes: [],
            unread_count: 0,
          },
        ],
      });
    if (url.includes("query_imap=true"))
      return Response.json({
        data: [
          {
            id: "message-1",
            folders: ["inbox-id"],
            from: [{ name: "Alice", email: "alice@example.com" }],
            to: [{ email: "reader@icloud.com" }],
            subject: "Hello",
            snippet: "Live mail",
            date: 1_700_000_000,
            unread: true,
          },
        ],
      });
    return Response.json({ data: [] });
  };
  const env = { NYLAS_API_KEY: "secret-key" };
  const connected = await handleRequest(
    new Request("https://worker.example/icloud/connect", {
      method: "POST",
      body: JSON.stringify({
        email: "reader@icloud.com",
        appPassword: "abcd-efgh-ijkl-mnop",
      }),
    }),
    env,
    fetcher,
  );
  const grant = await connected.json();
  assert.equal(grant.grantId, "grant-123");
  assert.ok(grant.accessToken);
  assert.equal(calls[0].init.headers.Authorization, "Bearer secret-key");
  assert.deepEqual(JSON.parse(calls[0].init.body).scope, ["email.modify"]);

  const synced = await handleRequest(
    new Request("https://worker.example/icloud/sync", {
      headers: { Authorization: `Bearer ${grant.accessToken}` },
    }),
    env,
    fetcher,
  );
  assert.equal(synced.status, 200);
  const syncedMailbox = await synced.json();
  assert.equal(syncedMailbox.folders[1].name, "Trash");
  assert.deepEqual(syncedMailbox.messages[0], {
    id: "message-1",
    folderId: "inbox-id",
    sender: "Alice <alice@example.com>",
    recipients: ["reader@icloud.com"],
    subject: "Hello",
    preview: "Live mail",
    receivedAt: "2023-11-14T22:13:20.000Z",
    hasAttachments: false,
    isUnread: true,
  });

  const deleted = await handleRequest(
    new Request("https://worker.example/icloud/messages/message-1", {
      method: "DELETE",
      headers: { Authorization: `Bearer ${grant.accessToken}` },
    }),
    env,
    fetcher,
  );
  assert.equal(deleted.status, 200);
  assert.equal(calls.at(-1).init.method, "PUT");
  assert.deepEqual(JSON.parse(calls.at(-1).init.body), {
    folders: ["trash-id"],
  });
});

test("rejects mailbox reads without a valid signed token", async () => {
  const response = await handleRequest(
    new Request("https://worker.example/icloud/sync", {
      headers: { Authorization: "Bearer invalid" },
    }),
    { NYLAS_API_KEY: "secret-key" },
    () => assert.fail("Nylas must not be called"),
  );
  assert.equal(response.status, 401);
});

test("rejects invalid credentials before calling Nylas", async () => {
  const response = await handleRequest(
    new Request("https://worker.example/icloud/connect", {
      method: "POST",
      body: JSON.stringify({ email: "invalid", appPassword: "" }),
    }),
    { NYLAS_API_KEY: "secret-key" },
    () => assert.fail("Nylas must not be called"),
  );
  assert.equal(response.status, 400);
});

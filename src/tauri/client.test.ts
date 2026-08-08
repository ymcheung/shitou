import assert from "node:assert/strict";
import test from "node:test";

import { createDesktopMailboxClient } from "./client.ts";
import { demoMailbox } from "./demo-mailbox.ts";

test("desktop mailbox actions do not mutate the demo adapter", async () => {
  demoMailbox.markMessagesUnread(["msg-2"]);
  const wasUnread = demoMailbox.getMessage("msg-2").isUnread;
  const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
  const client = createDesktopMailboxClient(async (command, args) => {
    calls.push({ command, args });
    return { count: 1 } as never;
  });

  await client.markMessagesRead(["msg-2"]);

  assert.equal(demoMailbox.getMessage("msg-2").isUnread, wasUnread);
  assert.deepEqual(calls, [
    { command: "mark_messages_read", args: { messageIds: ["msg-2"] } },
  ]);
});

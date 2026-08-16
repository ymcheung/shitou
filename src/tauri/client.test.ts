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

test("desktop notification actions use the notification command contract", async () => {
  const calls: Array<{ command: string; args?: Record<string, unknown> }> = [];
  const client = createDesktopMailboxClient(async (command, args) => {
    calls.push({ command, args });
    return {} as never;
  });

  await client.processNotifications(true);
  await client.listNotifications("account-1");
  await client.markNotificationsSeen(["notification-1"]);
  await client.dismissNotification("notification-1");
  await client.restoreNotification("notification-1");
  await client.getNotificationsSetting();
  await client.setNotificationsEnabled(false);

  assert.deepEqual(calls, [
    {
      command: "process_notifications",
      args: { summarizeSecurity: true },
    },
    { command: "list_notifications", args: { accountId: "account-1" } },
    {
      command: "mark_notifications_seen",
      args: { notificationIds: ["notification-1"] },
    },
    {
      command: "dismiss_notification",
      args: { notificationId: "notification-1" },
    },
    {
      command: "restore_notification",
      args: { notificationId: "notification-1" },
    },
    { command: "get_notifications_setting", args: undefined },
    { command: "set_notifications_enabled", args: { enabled: false } },
  ]);
});

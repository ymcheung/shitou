import { invoke } from "@tauri-apps/api/core";
import type {
  AuthSession,
  Folder,
  MailAccount,
  MessageDetail,
  MessageSummary,
  NotificationList,
  NotificationSetting,
  Provider,
  ThemeMode,
} from "../shared/mail.types";

type InvokeCommand = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

export type MailboxClient = {
  connectProvider(provider: Exclude<Provider, "icloud">): Promise<MailAccount>;
  connectIcloud(email: string, appPassword: string): Promise<MailAccount>;
  removeAccount(accountId: string): Promise<{ removed: boolean }>;
  syncAccount(accountId: string): Promise<MailAccount>;
  syncAll(): Promise<MailAccount[]>;
  listAccounts(): Promise<MailAccount[]>;
  listFolders(accountId: string): Promise<Folder[]>;
  listMessages(folderId: string, query?: string): Promise<MessageSummary[]>;
  getMessage(messageId: string): Promise<MessageDetail>;
  markMessagesRead(messageIds: string[]): Promise<{ count: number }>;
  markMessagesUnread(messageIds: string[]): Promise<{ count: number }>;
  deleteMessages(messageIds: string[]): Promise<{ count: number }>;
  markMessagesSpam(messageIds: string[]): Promise<{ count: number }>;
  processNotifications(summarizeSecurity: boolean): Promise<NotificationList>;
  listNotifications(accountId?: string): Promise<NotificationList>;
  markNotificationsSeen(notificationIds: string[]): Promise<{ count: number }>;
  dismissNotification(notificationId: string): Promise<{ count: number }>;
  restoreNotification(notificationId: string): Promise<{ count: number }>;
  getNotificationsSetting(): Promise<NotificationSetting>;
  setNotificationsEnabled(enabled: boolean): Promise<NotificationSetting>;
};

const canInvoke =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function call<T>(
  invokeCommand: InvokeCommand,
  command: string,
  args?: Record<string, unknown>,
): Promise<T> {
  try {
    return await invokeCommand<T>(command, args);
  } catch (error) {
    throw new Error(String(error));
  }
}

export function createDesktopMailboxClient(
  invokeCommand: InvokeCommand = invoke,
): MailboxClient {
  return {
    connectProvider: (provider) =>
      call<MailAccount>(invokeCommand, "account_connect_provider", {
        provider,
      }),
    connectIcloud: (email, appPassword) =>
      call<MailAccount>(invokeCommand, "account_connect_icloud", {
        email,
        appPassword,
      }),
    removeAccount: (accountId) =>
      call(invokeCommand, "account_remove", { accountId }),
    syncAccount: (accountId) =>
      call(invokeCommand, "sync_account", { accountId }),
    syncAll: () => call(invokeCommand, "sync_all"),
    listAccounts: () => call(invokeCommand, "list_accounts"),
    listFolders: (accountId) =>
      call(invokeCommand, "list_folders", { accountId }),
    listMessages: (folderId, query = "") =>
      call(invokeCommand, "list_messages", { folderId, query }),
    getMessage: (messageId) =>
      call(invokeCommand, "get_message", { messageId }),
    markMessagesRead: (messageIds) =>
      call(invokeCommand, "mark_messages_read", { messageIds }),
    markMessagesUnread: (messageIds) =>
      call(invokeCommand, "mark_messages_unread", { messageIds }),
    deleteMessages: (messageIds) =>
      call(invokeCommand, "delete_messages", { messageIds }),
    markMessagesSpam: (messageIds) =>
      call(invokeCommand, "mark_messages_spam", { messageIds }),
    processNotifications: (summarizeSecurity) =>
      call(invokeCommand, "process_notifications", { summarizeSecurity }),
    listNotifications: (accountId) =>
      call(invokeCommand, "list_notifications", { accountId }),
    markNotificationsSeen: (notificationIds) =>
      call(invokeCommand, "mark_notifications_seen", { notificationIds }),
    dismissNotification: (notificationId) =>
      call(invokeCommand, "dismiss_notification", { notificationId }),
    restoreNotification: (notificationId) =>
      call(invokeCommand, "restore_notification", { notificationId }),
    getNotificationsSetting: () =>
      call(invokeCommand, "get_notifications_setting"),
    setNotificationsEnabled: (enabled) =>
      call(invokeCommand, "set_notifications_enabled", { enabled }),
  };
}

export const desktopMailboxClient = createDesktopMailboxClient();

export const authClient = {
  sendEmailOtp: (email: string) =>
    call<{ sent: boolean; email: string }>(invoke, "auth_send_email_otp", {
      email,
    }),
  currentSession: () =>
    canInvoke
      ? call<AuthSession | null>(invoke, "auth_current_session")
      : Promise.resolve(null),
  verifyEmailOtp: (email: string, otp: string) =>
    call<AuthSession>(invoke, "auth_verify_email_otp", { email, otp }),
  logout: () => call<{ removed: boolean }>(invoke, "auth_logout"),
};

export const settingsClient = {
  setTheme: (mode: ThemeMode) =>
    canInvoke
      ? call<{ mode: ThemeMode }>(invoke, "set_theme", { mode })
      : Promise.resolve({ mode }),
};

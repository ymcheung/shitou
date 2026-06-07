import { invoke } from "@tauri-apps/api/core";
import type {
  AuthSession,
  Folder,
  MailAccount,
  MessageDetail,
  MessageSummary,
  Provider,
  ThemeMode,
} from "../shared/mail.types";
import { demoAccounts, demoMailbox } from "./demo-mailbox";

const canInvoke =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
const authSessionStorageKey = "shitou.authSession";

async function call<T>(
  command: string,
  args?: Record<string, unknown>,
  fallback?: T,
): Promise<T> {
  if (canInvoke) {
    try {
      return await invoke<T>(command, args);
    } catch (error) {
      throw new Error(String(error));
    }
  }

  if (fallback !== undefined) return fallback;
  throw new Error(
    `Tauri command ${command} is unavailable outside the desktop shell.`,
  );
}

export const api = {
  authSendEmailOtp: (email: string) =>
    call<{ sent: boolean; email: string }>("auth_send_email_otp", { email }),
  authCurrentSession: async () => {
    if (canInvoke) return call<AuthSession | null>("auth_current_session");

    const value = window.localStorage.getItem(authSessionStorageKey);
    if (!value) return null;

    try {
      return JSON.parse(value) as AuthSession;
    } catch {
      window.localStorage.removeItem(authSessionStorageKey);
      return null;
    }
  },
  authVerifyEmailOtp: async (email: string, otp: string) => {
    const session = await call<AuthSession>(
      "auth_verify_email_otp",
      { email, otp },
      { authenticated: true, email, userId: "demo-user" },
    );
    if (!canInvoke)
      window.localStorage.setItem(
        authSessionStorageKey,
        JSON.stringify(session),
      );
    return session;
  },
  authLogout: async () => {
    const result = await call<{ removed: boolean }>("auth_logout", undefined, {
      removed: true,
    });
    if (!canInvoke) window.localStorage.removeItem(authSessionStorageKey);
    return result;
  },
  connectProvider: (provider: Exclude<Provider, "icloud">) =>
    call<{ provider: Provider; authUrl: string }>(
      "account_connect_provider",
      { provider },
      demoMailbox.connectProviderFallback(provider),
    ),
  connectIcloud: (email: string, appPassword: string) =>
    call<MailAccount>(
      "account_connect_icloud",
      { email, appPassword },
      demoMailbox.connectIcloudFallback(email),
    ),
  removeAccount: (accountId: string) =>
    call<{ removed: boolean }>(
      "account_remove",
      { accountId },
      { removed: true },
    ),
  syncAccount: (accountId: string) =>
    call<MailAccount>(
      "sync_account",
      { accountId },
      demoAccounts.find((account) => account.id === accountId) ??
        demoAccounts[0],
    ),
  syncAll: () => call<MailAccount[]>("sync_all", undefined, demoAccounts),
  listAccounts: () =>
    call<MailAccount[]>("list_accounts", undefined, demoAccounts),
  listFolders: (accountId: string) =>
    call<Folder[]>(
      "list_folders",
      { accountId },
      demoMailbox.listFolders(accountId),
    ),
  listMessages: (folderId: string, query = "") =>
    call<MessageSummary[]>(
      "list_messages",
      { folderId, query },
      demoMailbox.listMessages(folderId, query),
    ),
  getMessage: (messageId: string) =>
    call<MessageDetail>(
      "get_message",
      { messageId },
      demoMailbox.getMessage(messageId),
    ),
  markMessagesRead: (messageIds: string[]) =>
    call<{ count: number }>(
      "mark_messages_read",
      { messageIds },
      demoMailbox.markMessagesRead(messageIds),
    ),
  markMessagesUnread: (messageIds: string[]) =>
    call<{ count: number }>(
      "mark_messages_unread",
      { messageIds },
      demoMailbox.markMessagesUnread(messageIds),
    ),
  deleteMessages: (messageIds: string[]) =>
    call<{ count: number }>(
      "delete_messages",
      { messageIds },
      demoMailbox.deleteMessages(messageIds),
    ),
  markMessagesSpam: (messageIds: string[]) =>
    call<{ count: number }>(
      "mark_messages_spam",
      { messageIds },
      demoMailbox.markMessagesSpam(messageIds),
    ),
  setTheme: (mode: ThemeMode) =>
    call<{ mode: ThemeMode }>("set_theme", { mode }, { mode }),
};

export const demoApi = {
  authCompleteDemo: async (): Promise<AuthSession> => ({
    authenticated: true,
    email: "demo.reader@shitou.local",
    userId: "demo-user",
  }),
  connectProvider: async (_provider: Exclude<Provider, "icloud">) => {
    throw new Error("Adding accounts is unavailable in demo mode.");
  },
  connectIcloud: async (_email: string, _appPassword: string) => {
    throw new Error("Adding accounts is unavailable in demo mode.");
  },
  removeAccount: async (_accountId: string) => ({ removed: true }),
  syncAccount: async (accountId: string) =>
    demoAccounts.find((account) => account.id === accountId) ?? demoAccounts[0],
  syncAll: async () => demoAccounts,
  listAccounts: async () => demoAccounts,
  listFolders: async (accountId: string) => demoMailbox.listFolders(accountId),
  listMessages: async (folderId: string, query = "") =>
    demoMailbox.listMessages(folderId, query),
  getMessage: async (messageId: string) => demoMailbox.getMessage(messageId),
  markMessagesRead: async (messageIds: string[]) =>
    demoMailbox.markMessagesRead(messageIds),
  markMessagesUnread: async (messageIds: string[]) =>
    demoMailbox.markMessagesUnread(messageIds),
  deleteMessages: async (messageIds: string[]) =>
    demoMailbox.deleteMessages(messageIds),
  markMessagesSpam: async (messageIds: string[]) =>
    demoMailbox.markMessagesSpam(messageIds),
  setTheme: (mode: ThemeMode) => api.setTheme(mode),
};

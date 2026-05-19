import { invoke } from '@tauri-apps/api/core';
import type { AuthSession, Folder, MailAccount, MessageDetail, MessageSummary, Provider, ThemeMode } from '../shared/mail.types';
import { demoAccounts, demoMailbox } from './demo-mailbox';

const canInvoke = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

async function call<T>(command: string, args?: Record<string, unknown>, fallback?: T): Promise<T> {
  if (canInvoke) {
    return invoke<T>(command, args);
  }

  if (fallback !== undefined) return fallback;
  throw new Error(`Tauri command ${command} is unavailable outside the desktop shell.`);
}

export const api = {
  authStartMagicLink: (email: string) => call<{ sent: boolean; email: string }>('auth_start_magic_link', { email }, { sent: true, email }),
  authCompleteCallback: (url: string) =>
    call<AuthSession>('auth_complete_callback', { url }, { authenticated: true, email: 'reader@example.com', userId: 'demo-user' }),
  connectProvider: (provider: Exclude<Provider, 'icloud'>) =>
    call<{ provider: Provider; authUrl: string }>('account_connect_provider', { provider }, demoMailbox.connectProviderFallback(provider)),
  connectIcloud: (email: string, appPassword: string) =>
    call<MailAccount>('account_connect_icloud', { email, app_password: appPassword }, demoMailbox.connectIcloudFallback(email)),
  removeAccount: (accountId: string) => call<{ removed: boolean }>('account_remove', { account_id: accountId }, { removed: true }),
  syncAccount: (accountId: string) =>
    call<MailAccount>('sync_account', { account_id: accountId }, demoAccounts.find((account) => account.id === accountId) ?? demoAccounts[0]),
  syncAll: () => call<MailAccount[]>('sync_all', undefined, demoAccounts),
  listAccounts: () => call<MailAccount[]>('list_accounts', undefined, demoAccounts),
  listFolders: (accountId: string) => call<Folder[]>('list_folders', { account_id: accountId }, demoMailbox.listFolders(accountId)),
  listMessages: (folderId: string, query = '') =>
    call<MessageSummary[]>('list_messages', { folder_id: folderId, query }, demoMailbox.listMessages(folderId, query)),
  getMessage: (messageId: string) => call<MessageDetail>('get_message', { message_id: messageId }, demoMailbox.getMessage(messageId)),
  markMessagesRead: (messageIds: string[]) =>
    call<{ count: number }>('mark_messages_read', { message_ids: messageIds }, demoMailbox.markMessagesRead(messageIds)),
  deleteMessages: (messageIds: string[]) =>
    call<{ count: number }>('delete_messages', { message_ids: messageIds }, demoMailbox.deleteMessages(messageIds)),
  setTheme: (mode: ThemeMode) => call<{ mode: ThemeMode }>('set_theme', { mode }, { mode })
};

export const demoApi = {
  authCompleteDemo: async (): Promise<AuthSession> => ({
    authenticated: true,
    email: 'demo.reader@shitou.local',
    userId: 'demo-user'
  }),
  connectProvider: async (_provider: Exclude<Provider, 'icloud'>) => {
    throw new Error('Adding accounts is unavailable in demo mode.');
  },
  connectIcloud: async (_email: string, _appPassword: string) => {
    throw new Error('Adding accounts is unavailable in demo mode.');
  },
  removeAccount: async (_accountId: string) => ({ removed: true }),
  syncAccount: async (accountId: string) => demoAccounts.find((account) => account.id === accountId) ?? demoAccounts[0],
  syncAll: async () => demoAccounts,
  listAccounts: async () => demoAccounts,
  listFolders: async (accountId: string) => demoMailbox.listFolders(accountId),
  listMessages: async (folderId: string, query = '') => demoMailbox.listMessages(folderId, query),
  getMessage: async (messageId: string) => demoMailbox.getMessage(messageId),
  markMessagesRead: async (messageIds: string[]) => demoMailbox.markMessagesRead(messageIds),
  deleteMessages: async (messageIds: string[]) => demoMailbox.deleteMessages(messageIds),
  setTheme: (mode: ThemeMode) => api.setTheme(mode)
};

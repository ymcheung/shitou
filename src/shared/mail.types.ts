export type ThemeMode = 'system' | 'light' | 'dark';
export type Provider = 'gmail' | 'outlook' | 'icloud';
export type SettingsTab = 'general' | 'accounts' | 'advanced';

export type AuthSession = {
  email: string;
  userId: string;
  authenticated: boolean;
};

export type MailAccount = {
  id: string;
  provider: Provider;
  email: string;
  displayName: string;
  syncStatus: 'idle' | 'syncing' | 'offline' | 'error';
  lastSyncedAt: string | null;
};

export type Folder = {
  id: string;
  accountId: string;
  name: string;
  unreadCount: number;
};

export type MessageSummary = {
  id: string;
  folderId: string;
  accountId: string;
  providerMessageId: string;
  sender: string;
  recipients: string[];
  subject: string;
  preview: string;
  receivedAt: string;
  hasAttachments: boolean;
  isUnread: boolean;
};

export type MessageDetail = MessageSummary & {
  bodyHtml: string;
  bodyText: string;
  attachments: Array<{
    id: string;
    fileName: string;
    mimeType: string;
    byteSize: number;
  }>;
};

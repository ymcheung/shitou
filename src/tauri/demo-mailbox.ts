import type { Folder, MailAccount, MessageDetail, MessageSummary } from '../shared/mail.types';

export const demoAccounts: MailAccount[] = [
  {
    id: 'acc-gmail',
    provider: 'gmail',
    email: 'reader@gmail.com',
    displayName: 'Gmail',
    syncStatus: 'idle',
    lastSyncedAt: new Date(Date.now() - 1000 * 60 * 12).toISOString()
  },
  {
    id: 'acc-icloud',
    provider: 'icloud',
    email: 'reader@icloud.com',
    displayName: 'iCloud Mail',
    syncStatus: 'offline',
    lastSyncedAt: new Date(Date.now() - 1000 * 60 * 55).toISOString()
  }
];

export const demoFolders: Folder[] = [
  { id: 'inbox', accountId: 'acc-gmail', name: 'Inbox', unreadCount: 1 },
  { id: 'important', accountId: 'acc-gmail', name: 'Important', unreadCount: 0 },
  { id: 'archive', accountId: 'acc-gmail', name: 'Archive', unreadCount: 0 },
  { id: 'trash', accountId: 'acc-gmail', name: 'Trash', unreadCount: 0 },
  { id: 'spam', accountId: 'acc-gmail', name: 'Spam', unreadCount: 1 },
  { id: 'icloud-inbox', accountId: 'acc-icloud', name: 'Inbox', unreadCount: 1 },
  { id: 'icloud-trash', accountId: 'acc-icloud', name: 'Trash', unreadCount: 0 },
  { id: 'icloud-spam', accountId: 'acc-icloud', name: 'Spam', unreadCount: 0 }
];

let demoMessages: MessageDetail[] = [
  {
    id: 'msg-1',
    folderId: 'inbox',
    accountId: 'acc-gmail',
    providerMessageId: 'gmail:msg-1',
    sender: 'Neon Auth',
    recipients: ['reader@gmail.com'],
    subject: 'Magic link sign-in configured',
    preview: 'Password login is disabled and email magic links are the only registration path.',
    receivedAt: new Date(Date.now() - 1000 * 60 * 20).toISOString(),
    hasAttachments: false,
    isUnread: true,
    bodyText:
      'Magic link sign-in is configured for the desktop client. Password, social, calendar, reminder, contact, and notification features stay out of scope.\n\nPriya Shah\nProduct Security\nNeon Auth',
    bodyHtml:
      '<p>Magic link sign-in is configured for the desktop client.</p><p>Password, social, calendar, reminder, contact, and notification features stay out of scope.</p><div class="signature"><p>Priya Shah<br>Product Security<br>Neon Auth</p></div>',
    attachments: []
  },
  {
    id: 'msg-2',
    folderId: 'inbox',
    accountId: 'acc-gmail',
    providerMessageId: 'gmail:msg-2',
    sender: 'Gmail API',
    recipients: ['reader@gmail.com'],
    subject: 'Read-only scope accepted',
    preview: 'This account uses gmail.readonly and stores offline bodies locally on this Mac.',
    receivedAt: new Date(Date.now() - 1000 * 60 * 65).toISOString(),
    hasAttachments: true,
    isUnread: false,
    bodyText:
      'The OAuth request includes https://www.googleapis.com/auth/gmail.readonly and offline access for sync refresh. It does not request modify, send, compose, calendar, contacts, or notification scopes.',
    bodyHtml:
      '<p>The OAuth request includes <code>https://www.googleapis.com/auth/gmail.readonly</code> and offline access for sync refresh.</p><p>It does not request modify, send, compose, calendar, contacts, or notification scopes.</p>',
    attachments: [{ id: 'att-1', fileName: 'scope-audit.txt', mimeType: 'text/plain', byteSize: 1842 }]
  },
  {
    id: 'msg-3',
    folderId: 'icloud-inbox',
    accountId: 'acc-icloud',
    providerMessageId: 'imap:uid-301',
    sender: 'iCloud Mail',
    recipients: ['reader@icloud.com'],
    subject: 'IMAP sync cached',
    preview: 'iCloud reads over IMAP with an app-specific password stored in Keychain.',
    receivedAt: new Date(Date.now() - 1000 * 60 * 130).toISOString(),
    hasAttachments: false,
    isUnread: true,
    bodyText:
      'iCloud Mail uses imap.mail.me.com on port 993 with TLS. SMTP is not configured in this read-only release.\n\nMina Park\nMailbox Operations\niCloud Mail',
    bodyHtml:
      '<p>iCloud Mail uses <code>imap.mail.me.com:993</code> with TLS.</p><p>SMTP is not configured in this read-only release.</p><div class="signature"><p>Mina Park<br>Mailbox Operations<br>iCloud Mail</p></div>',
    attachments: []
  },
  {
    id: 'msg-4',
    folderId: 'spam',
    accountId: 'acc-gmail',
    providerMessageId: 'gmail:msg-4',
    sender: 'Security Notice',
    recipients: ['reader@gmail.com'],
    subject: 'Untrusted sender quarantined',
    preview: 'This demo message appears in the aggregate spam folder.',
    receivedAt: new Date(Date.now() - 1000 * 60 * 180).toISOString(),
    hasAttachments: false,
    isUnread: true,
    bodyText: 'This demo message appears in the aggregate spam folder.',
    bodyHtml: '<p>This demo message appears in the aggregate spam folder.</p>',
    attachments: []
  },
  {
    id: 'msg-5',
    folderId: 'inbox',
    accountId: 'acc-gmail',
    providerMessageId: 'gmail:msg-5',
    sender: 'Design Review',
    recipients: ['reader@gmail.com'],
    subject: 'Signature samples attached',
    preview: 'Attached are the signature samples for testing cached attachment metadata.',
    receivedAt: new Date(Date.now() - 1000 * 60 * 240).toISOString(),
    hasAttachments: true,
    isUnread: false,
    bodyText:
      'Attached are two signature samples for the demo mailbox. The HTML version uses a normal text signature so we can compare it with image-heavy signatures separately.\n\nJordan Lee\nDesign Systems\nShitou Mail',
    bodyHtml:
      '<p>Attached are two signature samples for the demo mailbox.</p><p>The HTML version uses a normal text signature so we can compare it with image-heavy signatures separately.</p><div class="signature"><p>Jordan Lee<br>Design Systems<br>Shitou Mail</p></div>',
    attachments: [
      { id: 'att-2', fileName: 'signature-samples.pdf', mimeType: 'application/pdf', byteSize: 482176 },
      { id: 'att-3', fileName: 'brand-footer.png', mimeType: 'image/png', byteSize: 128904 }
    ]
  },
  {
    id: 'msg-6',
    folderId: 'icloud-inbox',
    accountId: 'acc-icloud',
    providerMessageId: 'imap:uid-302',
    sender: 'Northstar Labs',
    recipients: ['reader@icloud.com'],
    subject: 'Logo signature rendering check',
    preview: 'This message includes images inside the signature block.',
    receivedAt: new Date(Date.now() - 1000 * 60 * 310).toISOString(),
    hasAttachments: false,
    isUnread: false,
    bodyText:
      'Please confirm the reader keeps inline signature images visible in the offline body cache.\n\nAvery Chen\nNorthstar Labs',
    bodyHtml:
      '<p>Please confirm the reader keeps inline signature images visible in the offline body cache.</p><div class="signature"><p><img alt="Northstar Labs mark" width="36" height="36" src="data:image/svg+xml,%3Csvg%20xmlns=%22http://www.w3.org/2000/svg%22%20width=%2236%22%20height=%2236%22%20viewBox=%220%200%2036%2036%22%3E%3Crect%20width=%2236%22%20height=%2236%22%20rx=%228%22%20fill=%22%2318181b%22/%3E%3Cpath%20d=%22M18%206l3.2%208.8L30%2018l-8.8%203.2L18%2030l-3.2-8.8L6%2018l8.8-3.2L18%206z%22%20fill=%22%23facc15%22/%3E%3C/svg%3E"></p><p>Avery Chen<br>Northstar Labs</p><p><img alt="Certified offline badge" width="96" height="24" src="data:image/svg+xml,%3Csvg%20xmlns=%22http://www.w3.org/2000/svg%22%20width=%2296%22%20height=%2224%22%20viewBox=%220%200%2096%2024%22%3E%3Crect%20width=%2296%22%20height=%2224%22%20rx=%2212%22%20fill=%22%23ecfeff%22/%3E%3Ctext%20x=%2212%22%20y=%2216%22%20font-family=%22Arial%22%20font-size=%2210%22%20font-weight=%22700%22%20fill=%22%230e7490%22%3EOFFLINE%20READY%3C/text%3E%3C/svg%3E"></p></div>',
    attachments: []
  }
];

const aggregateFolderNames: Record<string, string[]> = {
  'root:inbox': ['inbox'],
  'root:trash': ['trash'],
  'root:spam': ['spam', 'junk']
};

function folderMatchesAggregate(folderId: string, aggregateId: string) {
  const folder = demoFolders.find((item) => item.id === folderId);
  if (!folder) return false;
  return aggregateFolderNames[aggregateId]?.includes(folder.name.toLowerCase()) ?? false;
}

function refreshDemoUnreadCounts() {
  for (const folder of demoFolders) {
    folder.unreadCount = demoMessages.filter((message) => message.folderId === folder.id && message.isUnread).length;
  }
}

export const demoMailbox = {
  connectProviderFallback(provider: 'gmail' | 'outlook') {
    return {
      provider,
      authUrl:
        provider === 'gmail'
          ? 'https://accounts.google.com/o/oauth2/v2/auth'
          : 'https://login.microsoftonline.com/common/oauth2/v2.0/authorize'
    };
  },
  connectIcloudFallback(email: string): MailAccount {
    return {
      id: `acc-${Date.now()}`,
      provider: 'icloud',
      email,
      displayName: 'iCloud Mail',
      syncStatus: 'idle',
      lastSyncedAt: null
    };
  },
  listFolders(accountId: string) {
    return demoFolders.filter((folder) => folder.accountId === accountId);
  },
  listMessages(folderId: string, query = ''): MessageSummary[] {
    return demoMessages
      .filter((message) => (folderId.startsWith('root:') ? folderMatchesAggregate(message.folderId, folderId) : message.folderId === folderId))
      .filter((message) => `${message.sender} ${message.subject} ${message.preview}`.toLowerCase().includes(query.toLowerCase()))
      .map(({ bodyHtml: _bodyHtml, bodyText: _bodyText, attachments: _attachments, ...summary }) => summary);
  },
  getMessage(messageId: string) {
    return demoMessages.find((message) => message.id === messageId) ?? demoMessages[0];
  },
  markMessagesRead(messageIds: string[]) {
    let count = 0;
    for (const message of demoMessages) {
      if (messageIds.includes(message.id) && message.isUnread) {
        message.isUnread = false;
        count += 1;
      }
    }
    refreshDemoUnreadCounts();
    return { count };
  },
  deleteMessages(messageIds: string[]) {
    let count = 0;
    for (const message of demoMessages) {
      if (!messageIds.includes(message.id)) continue;
      const currentFolder = demoFolders.find((folder) => folder.id === message.folderId);
      if (currentFolder && ['trash', 'spam', 'junk'].includes(currentFolder.name.toLowerCase())) {
        demoMessages = demoMessages.filter((item) => item.id !== message.id);
        count += 1;
        continue;
      }
      const trash = demoFolders.find((folder) => folder.accountId === message.accountId && folder.name.toLowerCase() === 'trash');
      if (trash && message.folderId !== trash.id) {
        message.folderId = trash.id;
        count += 1;
      }
    }
    refreshDemoUnreadCounts();
    return { count };
  }
};

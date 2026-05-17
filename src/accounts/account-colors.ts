import type { MailAccount } from '../shared/mail.types';

export const accountColors = ['#0d9488', '#2563eb', '#f97316', '#7c3aed', '#dc2626', '#0891b2'];

export function accountColor(accountId: string, accounts: MailAccount[], overrides: Record<string, string>) {
  if (overrides[accountId]) return overrides[accountId];
  const index = accounts.findIndex((account) => account.id === accountId);
  return accountColors[Math.max(0, index) % accountColors.length];
}

export function accountLabel(accountId: string, accounts: MailAccount[]) {
  return accounts.find((account) => account.id === accountId)?.displayName ?? 'Mailbox';
}

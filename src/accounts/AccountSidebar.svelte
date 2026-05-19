<script lang="ts">
  import { AlertCircle, Apple, CloudOff, Inbox, Mail, RefreshCw, Trash2 } from '@lucide/svelte';
  import type { Folder, MailAccount } from '../shared/mail.types';

  let {
    unreadTotal,
    offlineAccounts,
    appError,
    appBusy,
    rootFolders,
    folders,
    accounts,
    selectedFolderId,
    selectedAccountId,
    accountColor,
    onLoadRootFolder,
    onLoadFolders,
    onLoadMessages,
    onRemoveAccount,
    onSyncAll
  }: {
    unreadTotal: number;
    offlineAccounts: number;
    appError: string;
    appBusy: boolean;
    rootFolders: Folder[];
    folders: Folder[];
    accounts: MailAccount[];
    selectedFolderId: string;
    selectedAccountId: string;
    accountColor: (accountId: string) => string;
    onLoadRootFolder: (folderId: string) => void | Promise<void>;
    onLoadFolders: (accountId: string) => void | Promise<void>;
    onLoadMessages: (folderId: string) => void | Promise<void>;
    onRemoveAccount: (accountId: string) => void | Promise<void>;
    onSyncAll: () => void | Promise<void>;
  } = $props();

  function folderIcon(folderId: string) {
    if (folderId.includes('trash')) return Trash2;
    if (folderId.includes('spam')) return AlertCircle;
    return Inbox;
  }
</script>

<aside class="flex min-w-0 flex-col bg-transparent">
  <div class="flex h-16 items-center justify-between border-b border-zinc-200/80 px-4 dark:border-zinc-900">
    <div class="flex min-w-0 items-center gap-3">
      <div class="grid size-10 shrink-0 place-items-center rounded-xl bg-zinc-900 text-white shadow-sm ring-1 ring-zinc-200 dark:bg-zinc-100 dark:text-zinc-950 dark:ring-zinc-700">
        <Mail size={20} />
      </div>
      <div class="min-w-0">
        <div class="truncate text-sm font-semibold">Shitou Mail</div>
        <div class="truncate text-xs text-zinc-500 dark:text-zinc-400">{unreadTotal} unread</div>
      </div>
    </div>
  </div>

  <div class="mail-scrollbar flex-1 overflow-y-auto p-3">
    {#if offlineAccounts}
      <div class="mb-3 flex items-center gap-2 rounded-lg border border-zinc-200 bg-zinc-50 px-3 py-2 text-xs font-medium text-zinc-700 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-200">
        <CloudOff size={15} />
        {offlineAccounts} account cached for offline reading
      </div>
    {/if}

    {#if appError}
      <div class="mb-3 flex items-start gap-2 rounded-lg border border-red-200 bg-red-50 px-3 py-2 text-xs font-medium text-red-700 dark:border-red-900/50 dark:bg-red-950/40 dark:text-red-200">
        <AlertCircle class="mt-0.5 shrink-0" size={15} />
        {appError}
      </div>
    {/if}

    <div class="mb-4 space-y-1">
      {#each rootFolders as rootFolder (rootFolder.id)}
        {@const RootIcon = folderIcon(rootFolder.id)}
        <button
          class={[
            'flex h-9 w-full cursor-pointer items-center justify-between rounded-lg px-3 text-sm font-medium',
            selectedFolderId === rootFolder.id ? 'bg-zinc-900 text-white shadow-sm dark:bg-zinc-100 dark:text-zinc-950' : 'text-zinc-700 hover:bg-zinc-100 hover:text-zinc-950 dark:text-zinc-200 dark:hover:bg-zinc-700'
          ]}
          type="button"
          onclick={() => void onLoadRootFolder(rootFolder.id)}
        >
          <span class="flex items-center gap-2"><RootIcon size={15} /> {rootFolder.name}</span>
          {#if rootFolder.unreadCount}<span class="text-xs font-semibold">{rootFolder.unreadCount}</span>{/if}
        </button>
      {/each}
    </div>

    <div class="space-y-3">
      {#each accounts as account (account.id)}
        <section>
          <div class="group flex items-center justify-between gap-2 rounded-lg px-2 py-1.5 hover:bg-zinc-100/70 dark:hover:bg-zinc-700/70">
            <button class="flex min-w-0 flex-1 cursor-pointer items-center gap-2 text-left" type="button" onclick={() => void onLoadFolders(account.id)}>
              <span class="grid size-8 shrink-0 place-items-center rounded-lg bg-zinc-100 text-zinc-600 dark:bg-zinc-900 dark:text-zinc-300">
                {#if account.provider === 'icloud'}<Apple size={16} />{:else}<Mail size={16} />{/if}
              </span>
              <span class="min-w-0">
                <span class="flex min-w-0 items-center gap-2">
                  <span class="size-2 shrink-0 rounded-full" style:background-color={accountColor(account.id)}></span>
                  <span class="block truncate text-sm font-semibold">{account.displayName}</span>
                </span>
                <span class="block truncate text-xs text-zinc-500 dark:text-zinc-400">{account.email}</span>
              </span>
            </button>
            <button
              class="hidden size-7 cursor-pointer place-items-center rounded-lg text-zinc-400 hover:bg-red-50 hover:text-red-600 focus-visible:grid group-hover:grid dark:hover:bg-red-950/40"
              title="Remove account"
              type="button"
              onclick={() => void onRemoveAccount(account.id)}
            >
              <Trash2 size={15} />
            </button>
          </div>

          {#if selectedAccountId === account.id}
            <div class="mt-1 space-y-1">
              {#each folders as folder (folder.id)}
                {@const FolderIcon = folderIcon(folder.id)}
                <button
                  class={[
                    'flex h-9 w-full cursor-pointer items-center justify-between rounded-lg px-3 text-sm font-medium',
                    selectedFolderId === folder.id ? 'bg-zinc-900 text-white shadow-sm dark:bg-zinc-100 dark:text-zinc-950' : 'text-zinc-600 hover:bg-zinc-100 hover:text-zinc-950 dark:text-zinc-300 dark:hover:bg-zinc-700'
                  ]}
                  type="button"
                  onclick={() => void onLoadMessages(folder.id)}
                >
                  <span class="flex items-center gap-2"><FolderIcon size={15} /> {folder.name}</span>
                  {#if folder.unreadCount}<span class="text-xs font-semibold">{folder.unreadCount}</span>{/if}
                </button>
              {/each}
            </div>
          {/if}
        </section>
      {/each}
    </div>
  </div>

  <div class="border-t border-zinc-200/80 p-3 dark:border-zinc-900">
    <div class="flex justify-end">
      <button
        class="inline-flex h-10 shrink-0 cursor-pointer items-center gap-2 rounded-md border border-zinc-200 bg-zinc-50 px-3 text-sm font-semibold text-zinc-700 transition-colors duration-200 hover:bg-zinc-100 focus:outline-none focus:ring-2 focus:ring-zinc-500 disabled:cursor-not-allowed disabled:opacity-60 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-200 dark:hover:bg-zinc-700"
        type="button"
        onclick={() => void onSyncAll()}
        disabled={appBusy}
      >
        <RefreshCw size={16} class={appBusy ? 'animate-spin' : ''} />
        Sync
      </button>
    </div>
  </div>
</aside>

<script lang="ts">
  import { AlertCircle, Apple, CloudOff, Inbox, Mail, Plus, RefreshCw, Settings, Trash2 } from '@lucide/svelte';
  import type { Folder, MailAccount, Provider, SettingsTab } from '../shared/mail.types';

  let {
    accountPanelOpen = $bindable(false),
    icloudEmail = $bindable(''),
    icloudPassword = $bindable(''),
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
    onConnectProvider,
    onConnectIcloud,
    onLoadRootFolder,
    onLoadFolders,
    onLoadMessages,
    onRemoveAccount,
    onOpenSettings,
    onSyncAll
  }: {
    accountPanelOpen: boolean;
    icloudEmail: string;
    icloudPassword: string;
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
    onConnectProvider: (provider: Exclude<Provider, 'icloud'>) => void | Promise<void>;
    onConnectIcloud: () => void | Promise<void>;
    onLoadRootFolder: (folderId: string) => void | Promise<void>;
    onLoadFolders: (accountId: string) => void | Promise<void>;
    onLoadMessages: (folderId: string) => void | Promise<void>;
    onRemoveAccount: (accountId: string) => void | Promise<void>;
    onOpenSettings: (tab: SettingsTab) => void;
    onSyncAll: () => void | Promise<void>;
  } = $props();

  function folderIcon(folderId: string) {
    if (folderId.includes('trash')) return Trash2;
    if (folderId.includes('spam')) return AlertCircle;
    return Inbox;
  }
</script>

<aside class="flex min-w-0 flex-col border-r border-ink-200 bg-white/80 backdrop-blur dark:border-slate-800 dark:bg-slate-900/90">
  <div class="flex h-16 items-center justify-between border-b border-ink-200 px-4 dark:border-slate-800">
    <div class="flex min-w-0 items-center gap-3">
      <div class="grid size-9 shrink-0 place-items-center rounded-md bg-signal-600 text-white">
        <Mail size={20} />
      </div>
      <div class="min-w-0">
        <div class="truncate text-sm font-semibold">Shitou Mail</div>
        <div class="truncate text-xs text-ink-500 dark:text-slate-400">{unreadTotal} unread</div>
      </div>
    </div>
    <button
      class="grid size-8 place-items-center rounded-md text-ink-500 hover:bg-ink-100 hover:text-ink-900 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-white"
      type="button"
      title="Add account"
      onclick={() => (accountPanelOpen = !accountPanelOpen)}
    >
      <Plus size={18} />
    </button>
  </div>

  <div class="mail-scrollbar flex-1 overflow-y-auto p-3">
    {#if accountPanelOpen}
      <section class="mb-3 rounded-lg border border-ink-200 bg-ink-50 p-3 dark:border-slate-800 dark:bg-slate-950">
        <div class="text-sm font-semibold">Add read-only account</div>
        <div class="mt-3 grid grid-cols-2 gap-2">
          <button
            class="rounded-md border border-ink-200 bg-white px-3 py-2 text-sm font-medium hover:bg-ink-50 dark:border-slate-700 dark:bg-slate-900 dark:hover:bg-slate-800"
            type="button"
            onclick={() => void onConnectProvider('gmail')}>Gmail</button
          >
          <button
            class="rounded-md border border-ink-200 bg-white px-3 py-2 text-sm font-medium hover:bg-ink-50 dark:border-slate-700 dark:bg-slate-900 dark:hover:bg-slate-800"
            type="button"
            onclick={() => void onConnectProvider('outlook')}>Outlook</button
          >
        </div>
        <div class="mt-3 space-y-2">
          <div class="block text-xs font-medium text-ink-500 dark:text-slate-400">iCloud Mail</div>
          <label class="sr-only" for="icloud-email">iCloud email</label>
          <input
            id="icloud-email"
            class="h-9 w-full rounded-md border-ink-200 text-sm dark:border-slate-700 dark:bg-slate-900"
            type="email"
            placeholder="name@icloud.com"
            bind:value={icloudEmail}
          />
          <label class="sr-only" for="icloud-password">iCloud app-specific password</label>
          <input
            id="icloud-password"
            class="h-9 w-full rounded-md border-ink-200 text-sm dark:border-slate-700 dark:bg-slate-900"
            type="password"
            placeholder="App-specific password"
            bind:value={icloudPassword}
          />
          <button
            class="inline-flex h-9 w-full items-center justify-center gap-2 rounded-md bg-ink-900 px-3 text-sm font-semibold text-white hover:bg-ink-800 disabled:opacity-50 dark:bg-white dark:text-slate-950"
            type="button"
            disabled={!icloudEmail || !icloudPassword}
            onclick={() => void onConnectIcloud()}
          >
            <Apple size={16} /> Connect iCloud
          </button>
        </div>
      </section>
    {/if}

    {#if offlineAccounts}
      <div class="mb-3 flex items-center gap-2 rounded-md bg-amber-50 px-3 py-2 text-xs text-amber-900 dark:bg-amber-950/40 dark:text-amber-100">
        <CloudOff size={15} />
        {offlineAccounts} account cached for offline reading
      </div>
    {/if}

    {#if appError}
      <div class="mb-3 flex items-start gap-2 rounded-md bg-red-50 px-3 py-2 text-xs text-red-700 dark:bg-red-950/40 dark:text-red-200">
        <AlertCircle class="mt-0.5 shrink-0" size={15} />
        {appError}
      </div>
    {/if}

    <div class="mb-4 space-y-1">
      {#each rootFolders as rootFolder (rootFolder.id)}
        {@const RootIcon = folderIcon(rootFolder.id)}
        <button
          class={[
            'flex h-9 w-full items-center justify-between rounded-md px-3 text-sm',
            selectedFolderId === rootFolder.id ? 'bg-signal-600 text-white' : 'text-ink-700 hover:bg-ink-100 dark:text-slate-200 dark:hover:bg-slate-800'
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
          <div class="group flex items-center justify-between gap-2 rounded-md px-2 py-1.5">
            <button class="flex min-w-0 flex-1 items-center gap-2 text-left" type="button" onclick={() => void onLoadFolders(account.id)}>
              <span class="grid size-7 shrink-0 place-items-center rounded-md bg-ink-100 text-ink-600 dark:bg-slate-800 dark:text-slate-300">
                {#if account.provider === 'icloud'}<Apple size={16} />{:else}<Mail size={16} />{/if}
              </span>
              <span class="min-w-0">
                <span class="flex min-w-0 items-center gap-2">
                  <span class="size-2 shrink-0 rounded-full" style:background-color={accountColor(account.id)}></span>
                  <span class="block truncate text-sm font-semibold">{account.displayName}</span>
                </span>
                <span class="block truncate text-xs text-ink-500 dark:text-slate-400">{account.email}</span>
              </span>
            </button>
            <button
              class="hidden size-7 place-items-center rounded-md text-ink-400 hover:bg-red-50 hover:text-red-600 group-hover:grid dark:hover:bg-red-950/40"
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
                    'flex h-9 w-full items-center justify-between rounded-md px-3 text-sm',
                    selectedFolderId === folder.id ? 'bg-signal-600 text-white' : 'text-ink-600 hover:bg-ink-100 dark:text-slate-300 dark:hover:bg-slate-800'
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

  <div class="border-t border-ink-200 p-3 dark:border-slate-800">
    <div class="flex items-center gap-2">
      <button
        class="flex h-10 min-w-0 flex-1 cursor-pointer items-center gap-2 rounded-md px-3 text-sm font-semibold text-ink-700 transition-colors duration-200 hover:bg-ink-100 focus:outline-none focus:ring-2 focus:ring-signal-500 dark:text-slate-200 dark:hover:bg-slate-800"
        type="button"
        onclick={() => onOpenSettings('general')}
      >
        <Settings size={17} /> Settings
      </button>
      <button
        class="inline-flex h-10 shrink-0 cursor-pointer items-center gap-2 rounded-md border border-ink-200 bg-white px-3 text-sm font-semibold text-ink-700 transition-colors duration-200 hover:bg-ink-50 focus:outline-none focus:ring-2 focus:ring-signal-500 disabled:cursor-not-allowed disabled:opacity-60 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
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

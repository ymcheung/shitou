<script lang="ts">
  import { AlertCircle, Apple, CloudOff, Inbox, Mail, Trash2 } from '@lucide/svelte';
  import type { Folder, MailAccount } from '../shared/mail.types';

  let {
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
    onRemoveAccount
  }: {
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
  } = $props();

  const sidebarStyles = {
    folderButton: 'flex h-9 w-full cursor-pointer items-center justify-between rounded-lg px-3 text-sm font-medium',
    iconChip: 'grid size-6 shrink-0 place-items-center rounded-md ring-1',
    selectedIconChip: 'bg-white/18 text-current ring-white/25 dark:bg-black/10 dark:ring-black/10',
    unreadBadge: 'rounded-full px-1.5 py-0.5 text-xs font-semibold',
    selectedUnreadBadge: 'bg-white/20 text-current dark:bg-black/10'
  } as const;

  const folderThemes = {
    inbox: {
      chip: 'bg-indigo-50 text-indigo-700 ring-indigo-200 dark:bg-indigo-950/45 dark:text-indigo-200 dark:ring-indigo-800/70',
      idle:
        'text-zinc-700 hover:bg-indigo-50 hover:text-indigo-700 dark:text-zinc-200 dark:hover:bg-indigo-950/40 dark:hover:text-indigo-100',
      selected: 'bg-indigo-600 text-white shadow-sm shadow-indigo-900/10',
      unread: 'bg-indigo-100 text-indigo-800 dark:bg-indigo-950 dark:text-indigo-200'
    },
    trash: {
      chip: 'bg-zinc-100 text-zinc-700 ring-zinc-200 dark:bg-zinc-900 dark:text-zinc-200 dark:ring-zinc-700',
      idle:
        'text-zinc-700 hover:bg-zinc-100 hover:text-zinc-950 dark:text-zinc-200 dark:hover:bg-zinc-800 dark:hover:text-zinc-50',
      selected: 'bg-zinc-800 text-white shadow-sm shadow-zinc-900/10 dark:bg-zinc-200 dark:text-zinc-950',
      unread: 'bg-zinc-200 text-zinc-800 dark:bg-zinc-800 dark:text-zinc-100'
    },
    spam: {
      chip: 'bg-red-50 text-red-700 ring-red-200 dark:bg-red-950/45 dark:text-red-200 dark:ring-red-800/70',
      idle:
        'text-zinc-700 hover:bg-red-50 hover:text-red-900 dark:text-zinc-200 dark:hover:bg-red-950/40 dark:hover:text-red-100',
      selected: 'bg-red-600 text-white shadow-sm shadow-red-900/10 dark:bg-red-400 dark:text-red-950',
      unread: 'bg-red-100 text-red-800 dark:bg-red-950 dark:text-red-200'
    },
    default: {
      chip: 'bg-zinc-100 text-zinc-600 ring-zinc-200 dark:bg-zinc-900 dark:text-zinc-300 dark:ring-zinc-800',
      idle:
        'text-zinc-600 hover:bg-zinc-100 hover:text-zinc-950 dark:text-zinc-300 dark:hover:bg-zinc-700',
      selected: 'bg-zinc-900 text-white shadow-sm dark:bg-zinc-100 dark:text-zinc-950',
      unread: 'text-current'
    }
  } as const;

  function folderKey(folder: Pick<Folder, 'id' | 'name'>) {
    return `${folder.id}:${folder.name}`.toLowerCase();
  }

  function folderTheme(folder: Pick<Folder, 'id' | 'name'>) {
    const key = folderKey(folder);
    if (key.includes('trash')) return folderThemes.trash;
    if (key.includes('spam') || key.includes('junk')) return folderThemes.spam;
    if (key.includes('inbox')) return folderThemes.inbox;
    return folderThemes.default;
  }

  function folderIcon(folder: Pick<Folder, 'id' | 'name'>) {
    const key = folderKey(folder);
    if (key.includes('trash')) return Trash2;
    if (key.includes('spam') || key.includes('junk')) return AlertCircle;
    return Inbox;
  }
</script>

<aside class="flex min-w-0 flex-col bg-transparent">
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
        {@const RootIcon = folderIcon(rootFolder)}
        {@const rootTheme = folderTheme(rootFolder)}
        {@const rootSelected = selectedFolderId === rootFolder.id}
        <button
          class={[
            sidebarStyles.folderButton,
            rootSelected ? rootTheme.selected : rootTheme.idle
          ]}
          type="button"
          onclick={() => void onLoadRootFolder(rootFolder.id)}
        >
          <span class="flex items-center gap-2">
            <span
              class={[
                sidebarStyles.iconChip,
                rootSelected ? sidebarStyles.selectedIconChip : rootTheme.chip
              ]}
            >
              <RootIcon size={15} />
            </span>
            {rootFolder.name}
          </span>
          {#if rootFolder.unreadCount}
            <span
              class={[
                sidebarStyles.unreadBadge,
                rootSelected ? sidebarStyles.selectedUnreadBadge : rootTheme.unread
              ]}
            >
              {rootFolder.unreadCount}
            </span>
          {/if}
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
                {@const FolderIcon = folderIcon(folder)}
                {@const folderThemeStyles = folderTheme(folder)}
                {@const folderSelected = selectedFolderId === folder.id}
                <button
                  class={[
                    sidebarStyles.folderButton,
                    folderSelected ? folderThemeStyles.selected : folderThemeStyles.idle
                  ]}
                  type="button"
                  onclick={() => void onLoadMessages(folder.id)}
                >
                  <span class="flex items-center gap-2">
                    <span
                      class={[
                        sidebarStyles.iconChip,
                        folderSelected ? sidebarStyles.selectedIconChip : folderThemeStyles.chip
                      ]}
                    >
                      <FolderIcon size={15} />
                    </span>
                    {folder.name}
                  </span>
                  {#if folder.unreadCount}
                    <span
                      class={[
                        sidebarStyles.unreadBadge,
                        folderSelected ? sidebarStyles.selectedUnreadBadge : folderThemeStyles.unread
                      ]}
                    >
                      {folder.unreadCount}
                    </span>
                  {/if}
                </button>
              {/each}
            </div>
          {/if}
        </section>
      {/each}
    </div>
  </div>
</aside>

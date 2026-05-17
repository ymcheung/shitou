<script lang="ts">
  import { CheckCircle2, Paperclip, Search, Trash2 } from '@lucide/svelte';
  import { formatRelative } from '../app/formatting';
  import type { Folder, MailAccount, MessageDetail, MessageSummary } from '../shared/mail.types';

  let {
    query = $bindable(''),
    selectedFolder,
    selectedAccount,
    accountsCount,
    messages,
    selectedMessage,
    selectedMessageIds,
    selectionMode,
    allVisibleSelected,
    isPermanentDeleteFolder,
    accountColor,
    accountLabel,
    onSearch,
    onStartSelection,
    onSelectAllVisible,
    onMarkSelectedRead,
    onDeleteSelected,
    onToggleMessageSelection,
    onOpenMessage
  }: {
    query: string;
    selectedFolder: Folder | null;
    selectedAccount: MailAccount | null;
    accountsCount: number;
    messages: MessageSummary[];
    selectedMessage: MessageDetail | null;
    selectedMessageIds: string[];
    selectionMode: boolean;
    allVisibleSelected: boolean;
    isPermanentDeleteFolder: boolean;
    accountColor: (accountId: string) => string;
    accountLabel: (accountId: string) => string;
    onSearch: () => void | Promise<void>;
    onStartSelection: () => void;
    onSelectAllVisible: () => void;
    onMarkSelectedRead: () => void | Promise<void>;
    onDeleteSelected: () => void | Promise<void>;
    onToggleMessageSelection: (messageId: string) => void;
    onOpenMessage: (messageId: string) => void | Promise<void>;
  } = $props();

  let selectedMessageIdSet = $derived(new Set(selectedMessageIds));
</script>

<section class="flex min-w-0 flex-col border-r border-ink-200 bg-ink-50 dark:border-slate-800 dark:bg-slate-950">
  <div class="border-b border-ink-200 p-4 dark:border-slate-800">
    <div class="mb-3 flex items-center justify-between gap-3">
      <div class="min-w-0">
        <h2 class="truncate text-lg font-semibold">{selectedFolder?.name ?? 'Mailbox'}</h2>
        <p class="truncate text-xs text-ink-500 dark:text-slate-400">
          {selectedAccount?.email ?? 'All mailboxes'} · {selectedAccount ? formatRelative(selectedAccount.lastSyncedAt) : `${accountsCount} accounts`}
        </p>
      </div>
    </div>
    <form
      class="relative"
      onsubmit={(event) => {
        event.preventDefault();
        void onSearch();
      }}
    >
      <Search class="absolute left-3 top-2.5 text-ink-400" size={16} />
      <input
        class="h-10 w-full rounded-md border-ink-200 bg-white pl-9 text-sm dark:border-slate-800 dark:bg-slate-900"
        placeholder="Search offline mail"
        bind:value={query}
      />
    </form>
    <div class="mt-3 flex items-center justify-between gap-2">
      <div class="flex min-w-0 items-center gap-2">
        <button
          class="inline-flex h-8 items-center rounded-md border border-ink-200 bg-white px-2.5 text-xs font-semibold text-ink-700 hover:bg-ink-50 disabled:opacity-50 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
          type="button"
          disabled={messages.length === 0}
          onclick={onStartSelection}
        >
          Select
        </button>
        {#if selectionMode}
          <button
            class="inline-flex h-8 items-center rounded-md border border-ink-200 bg-white px-2.5 text-xs font-semibold text-ink-700 hover:bg-ink-50 disabled:opacity-50 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
            type="button"
            disabled={messages.length === 0 || allVisibleSelected}
            onclick={onSelectAllVisible}
          >
            All
          </button>
          <span class="truncate text-xs font-medium text-ink-500 dark:text-slate-400">{selectedMessageIds.length} selected</span>
        {/if}
      </div>
      <div class="flex shrink-0 items-center gap-2">
        {#if selectionMode}
          <button
            class="inline-flex h-8 items-center gap-1.5 rounded-md border border-ink-200 bg-white px-2.5 text-xs font-semibold text-ink-700 hover:bg-ink-50 disabled:opacity-50 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
            type="button"
            disabled={selectedMessageIds.length === 0}
            onclick={() => void onMarkSelectedRead()}
          >
            <CheckCircle2 size={14} /> Read
          </button>
          <button
            class="inline-flex h-8 items-center gap-1.5 rounded-md border border-red-200 bg-white px-2.5 text-xs font-semibold text-red-700 hover:bg-red-50 disabled:opacity-50 dark:border-red-950 dark:bg-slate-900 dark:text-red-300 dark:hover:bg-red-950/40"
            type="button"
            disabled={selectedMessageIds.length === 0}
            onclick={() => void onDeleteSelected()}
          >
            <Trash2 size={14} />
            {isPermanentDeleteFolder ? 'Delete forever' : 'Delete'}
          </button>
        {/if}
      </div>
    </div>
  </div>

  <div class="mail-scrollbar flex-1 overflow-y-auto">
    {#if messages.length === 0}
      <div class="grid h-full place-items-center p-8 text-center text-sm text-ink-500 dark:text-slate-400">No cached messages in this folder.</div>
    {:else}
      {#each messages as message (message.id)}
        <div
          class={[
            'flex w-full gap-3 border-b border-ink-200 p-4 hover:bg-white dark:border-slate-800 dark:hover:bg-slate-900',
            selectedMessage?.id === message.id ? 'bg-white dark:bg-slate-900' : ''
          ]}
        >
          <input
            class={[
              'mt-1 size-4 shrink-0 rounded border-ink-300 text-signal-600 focus:ring-signal-500 dark:border-slate-700 dark:bg-slate-900',
              selectionMode ? '' : 'pointer-events-none opacity-0'
            ]}
            type="checkbox"
            aria-label={`Select ${message.subject}`}
            aria-hidden={!selectionMode}
            checked={selectedMessageIdSet.has(message.id)}
            disabled={!selectionMode}
            tabindex={selectionMode ? 0 : -1}
            onchange={() => onToggleMessageSelection(message.id)}
          />
          <button class="min-w-0 flex-1 text-left" type="button" onclick={() => void onOpenMessage(message.id)}>
            <div class="flex items-center justify-between gap-3">
              <span class="flex min-w-0 items-center gap-2">
                <span class="size-2 shrink-0 rounded-full" style:background-color={accountColor(message.accountId)} title={accountLabel(message.accountId)}></span>
                <span
                  class={[
                    'truncate text-base',
                    message.isUnread ? 'font-semibold text-ink-950 dark:text-white' : 'font-medium text-ink-700 dark:text-slate-300'
                  ]}>{message.subject}</span
                >
              </span>
              <span class="shrink-0 text-xs text-ink-400">{formatRelative(message.receivedAt)}</span>
            </div>
            <div class="mt-1 flex items-center gap-2">
              <span class={['truncate text-xs text-ink-500 dark:text-slate-400', message.isUnread ? 'font-semibold' : 'font-medium']}>
                {message.sender}
              </span>
              {#if message.hasAttachments}<Paperclip class="shrink-0 text-ink-400" size={14} />{/if}
            </div>
            <p class="mt-1 line-clamp-2 text-sm leading-5 text-ink-500 dark:text-slate-400">{message.preview}</p>
          </button>
        </div>
      {/each}
    {/if}
  </div>
</section>

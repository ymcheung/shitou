<script lang="ts">
  import { CheckCircle2, Paperclip, Trash2 } from "@lucide/svelte";
  import { formatRelative } from "../app/formatting";
  import type {
    Folder,
    MailAccount,
    MessageDetail,
    MessageSummary,
  } from "../shared/mail.types";

  let {
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
    onStartSelection,
    onSelectAllVisible,
    onMarkSelectedRead,
    onDeleteSelected,
    onToggleMessageSelection,
    onOpenMessage,
  }: {
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
    onStartSelection: () => void;
    onSelectAllVisible: () => void;
    onMarkSelectedRead: () => void | Promise<void>;
    onDeleteSelected: () => void | Promise<void>;
    onToggleMessageSelection: (messageId: string) => void;
    onOpenMessage: (messageId: string) => void | Promise<void>;
  } = $props();

  let selectedMessageIdSet = $derived(new Set(selectedMessageIds));
</script>

<section
  class="flex min-w-0 flex-col overflow-hidden rounded-tl-[28px] bg-zinc-50/80 shadow-panel backdrop-blur-[12px] dark:bg-zinc-900/80"
>
  <div
    class="border-b border-zinc-200 bg-zinc-50/80 p-4 backdrop-blur-[12px] dark:border-zinc-800 dark:bg-zinc-900/80"
  >
    <div class="mb-3 flex items-center justify-between gap-3">
      <div class="min-w-0">
        <h2 class="truncate text-lg font-semibold tracking-normal">
          {selectedFolder?.name ?? "Mailbox"}
        </h2>
        <p
          class="truncate text-xs font-medium text-zinc-500 dark:text-zinc-400"
        >
          {selectedAccount?.email ?? "All mailboxes"} · {selectedAccount
            ? formatRelative(selectedAccount.lastSyncedAt)
            : `${accountsCount} accounts`}
        </p>
      </div>
    </div>
    <div class="mt-3 flex items-center justify-between gap-2">
      <div class="flex min-w-0 items-center gap-2">
        <button
          class="inline-flex h-8 cursor-pointer items-center rounded-lg border border-zinc-200 bg-zinc-50 px-2.5 text-xs font-semibold text-zinc-700 hover:bg-zinc-100 disabled:cursor-not-allowed disabled:opacity-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-200 dark:hover:bg-zinc-700"
          type="button"
          disabled={messages.length === 0}
          onclick={onStartSelection}
        >
          Select
        </button>
        {#if selectionMode}
          <button
            class="inline-flex h-8 cursor-pointer items-center rounded-lg border border-zinc-200 bg-zinc-50 px-2.5 text-xs font-semibold text-zinc-700 hover:bg-zinc-100 disabled:cursor-not-allowed disabled:opacity-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-200 dark:hover:bg-zinc-700"
            type="button"
            disabled={messages.length === 0 || allVisibleSelected}
            onclick={onSelectAllVisible}
          >
            All
          </button>
          <span
            class="truncate text-xs font-medium text-zinc-500 dark:text-zinc-400"
            >{selectedMessageIds.length} selected</span
          >
        {/if}
      </div>
      <div class="flex shrink-0 items-center gap-2">
        {#if selectionMode}
          <button
            class="inline-flex h-8 cursor-pointer items-center gap-1.5 rounded-lg border border-zinc-200 bg-zinc-50 px-2.5 text-xs font-semibold text-zinc-700 hover:bg-zinc-100 disabled:cursor-not-allowed disabled:opacity-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-200 dark:hover:bg-zinc-700"
            type="button"
            disabled={selectedMessageIds.length === 0}
            onclick={() => void onMarkSelectedRead()}
          >
            <CheckCircle2 size={14} /> Read
          </button>
          <button
            class="inline-flex h-8 cursor-pointer items-center gap-1.5 rounded-lg border border-red-200 bg-zinc-50 px-2.5 text-xs font-semibold text-red-700 hover:bg-red-50 disabled:cursor-not-allowed disabled:opacity-50 dark:border-red-950 dark:bg-zinc-900 dark:text-red-300 dark:hover:bg-red-950/40"
            type="button"
            disabled={selectedMessageIds.length === 0}
            onclick={() => void onDeleteSelected()}
          >
            <Trash2 size={14} />
            {isPermanentDeleteFolder ? "Delete forever" : "Move to Trash"}
          </button>
        {/if}
      </div>
    </div>
  </div>

  <div class="mail-scrollbar flex-1 overflow-y-auto">
    {#if messages.length === 0}
      <div
        class="grid h-full place-items-center p-8 text-center text-sm font-medium text-zinc-500 dark:text-zinc-400"
      >
        No cached messages in this folder.
      </div>
    {:else}
      {#each messages as message (message.id)}
        <div
          class={[
            "flex w-full gap-3 border-b border-zinc-200 p-4 hover:bg-zinc-100 dark:border-zinc-900 dark:hover:bg-zinc-700",
            selectedMessage?.id === message.id
              ? "bg-zinc-50 shadow-sm ring-1 ring-inset ring-zinc-200 dark:bg-zinc-900 dark:ring-zinc-700"
              : "",
          ]}
        >
          <input
            class={[
              "mt-1 size-4 shrink-0 rounded border-zinc-300 text-zinc-900 focus:ring-zinc-500 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100",
              selectionMode ? "" : "pointer-events-none opacity-0",
            ]}
            type="checkbox"
            aria-label={`Select ${message.subject}`}
            aria-hidden={!selectionMode}
            checked={selectedMessageIdSet.has(message.id)}
            disabled={!selectionMode}
            tabindex={selectionMode ? 0 : -1}
            onchange={() => onToggleMessageSelection(message.id)}
          />
          <button
            class="min-w-0 flex-1 cursor-pointer text-left focus-visible:rounded-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-zinc-500"
            type="button"
            onclick={() => void onOpenMessage(message.id)}
          >
            <div class="flex items-center justify-between gap-3">
              <span class="flex min-w-0 items-center gap-2">
                <span
                  class="size-2 shrink-0 rounded-full"
                  style:background-color={accountColor(message.accountId)}
                  title={accountLabel(message.accountId)}
                ></span>
                <span
                  class={[
                    "truncate text-base",
                    message.isUnread
                      ? "font-semibold text-zinc-950 dark:text-white"
                      : "font-medium text-zinc-700 dark:text-zinc-300",
                  ]}>{message.subject}</span
                >
              </span>
              <span class="shrink-0 text-xs text-zinc-400"
                >{formatRelative(message.receivedAt)}</span
              >
            </div>
            <div class="mt-1 flex items-center gap-2">
              <span
                class={[
                  "truncate text-xs text-zinc-500 dark:text-zinc-400",
                  message.isUnread ? "font-semibold" : "font-medium",
                ]}
              >
                {message.sender}
              </span>
              {#if message.hasAttachments}<Paperclip
                  class="shrink-0 text-zinc-400"
                  size={14}
                />{/if}
            </div>
            <p
              class="mt-1 line-clamp-2 text-sm leading-5 text-zinc-500 dark:text-zinc-400"
            >
              {message.preview}
            </p>
          </button>
        </div>
      {/each}
    {/if}
  </div>
</section>

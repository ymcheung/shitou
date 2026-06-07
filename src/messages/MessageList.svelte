<script lang="ts">
  import {
    CheckCircle2,
    Mail,
    MailOpen,
    OctagonAlert,
    Paperclip,
    Trash2,
  } from "@lucide/svelte";
  import { onMount, tick } from "svelte";
  import { formatRelative } from "../app/formatting";
  import SenderAvatar from "./SenderAvatar.svelte";
  import * as ContextMenu from "../shared/ui/context-menu/index.js";
  import type {
    Folder,
    MailAccount,
    MessageDetail,
    MessageSummary,
  } from "../shared/mail.types";
  import type { AutoLayout } from "animejs/layout";

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
    onMarkMessageRead,
    onMarkMessageUnread,
    onMoveMessageToTrash,
    onMarkMessageSpam,
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
    onMarkMessageRead: (messageIds: string[]) => void | Promise<void>;
    onMarkMessageUnread: (messageIds: string[]) => void | Promise<void>;
    onMoveMessageToTrash: (messageIds: string[]) => void | Promise<void>;
    onMarkMessageSpam: (messageIds: string[]) => void | Promise<void>;
    onToggleMessageSelection: (messageId: string) => void;
    onOpenMessage: (messageId: string) => void | Promise<void>;
  } = $props();

  let selectedMessageIdSet = $derived(new Set(selectedMessageIds));
  let messageListElement: HTMLDivElement | undefined;
  let messageLayout: AutoLayout | undefined;
  let prefersReducedMotion = false;
  let hasMountedLayout = false;

  function handleMessageClick(event: MouseEvent, messageId: string) {
    if (event.metaKey || event.ctrlKey) {
      event.preventDefault();
      onStartSelection();
      onToggleMessageSelection(messageId);
      return;
    }

    void onOpenMessage(messageId);
  }

  function isFolderNamed(name: string) {
    return selectedFolder?.name.toLowerCase() === name;
  }

  function getContextTargetIds(message: MessageSummary) {
    return selectionMode && selectedMessageIds.length > 0
      ? selectedMessageIds
      : [message.id];
  }

  function getContextTargetMessages(message: MessageSummary) {
    const targetIds = new Set(getContextTargetIds(message));
    return messages.filter((item) => targetIds.has(item.id));
  }

  function hasUnreadContextTarget(message: MessageSummary) {
    return getContextTargetMessages(message).some((item) => item.isUnread);
  }

  function hasReadContextTarget(message: MessageSummary) {
    return getContextTargetMessages(message).some((item) => !item.isUnread);
  }

  onMount(() => {
    const motionQuery = window.matchMedia("(prefers-reduced-motion: reduce)");
    prefersReducedMotion = motionQuery.matches;

    const handleMotionChange = (event: MediaQueryListEvent) => {
      prefersReducedMotion = event.matches;
    };

    motionQuery.addEventListener("change", handleMotionChange);

    const setupLayout = async () => {
      if (!messageListElement || prefersReducedMotion) return;
      const { createLayout } = await import("animejs/layout");
      messageLayout = createLayout(messageListElement, {
        children: ".message-list-item",
        duration: 240,
        ease: "out(3)",
        swapAt: { opacity: 1 },
        enterFrom: {
          opacity: 0,
          transform: "translateY(-10px)",
          duration: 180,
          ease: "out(2)",
        },
        leaveTo: {
          opacity: 0,
          transform: "translateY(10px)",
          duration: 160,
          ease: "in(2)",
        },
      });
      hasMountedLayout = true;
    };

    void setupLayout();

    return () => {
      motionQuery.removeEventListener("change", handleMotionChange);
      messageLayout?.revert();
    };
  });

  $effect.pre(() => {
    messages;
    if (!messageLayout || prefersReducedMotion || !hasMountedLayout) return;

    messageLayout.record();
    void tick().then(() => {
      if (!messageLayout || prefersReducedMotion) return;
      messageLayout.animate();
    });
  });
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

  <div
    class="mail-scrollbar flex-1 overflow-y-auto"
    bind:this={messageListElement}
  >
    {#if messages.length === 0}
      <div
        class="grid h-full place-items-center p-8 text-center text-sm font-medium text-zinc-500 dark:text-zinc-400"
      >
        No cached messages in this folder.
      </div>
    {:else}
      {#each messages as message (message.id)}
        <ContextMenu.Root>
          <ContextMenu.Trigger>
            {#snippet child({ props })}
              <div
                {...props}
                data-layout-id={message.id}
                class={[
                  "message-list-item",
                  "flex w-full gap-3 border-b border-zinc-200 p-4 transition-colors duration-200 dark:border-zinc-900",
                  selectedMessage?.id === message.id
                    ? "bg-zinc-200 shadow-sm hover:bg-zinc-200 dark:bg-zinc-600 dark:hover:bg-zinc-600"
                    : "hover:bg-zinc-100 dark:hover:bg-zinc-700",
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
                <SenderAvatar
                  sender={message.sender}
                  avatarUrl={message.senderAvatarUrl}
                  size="sm"
                />
                <button
                  class="min-w-0 flex-1 cursor-pointer text-left focus-visible:rounded-lg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-zinc-500"
                  type="button"
                  onclick={(event) => handleMessageClick(event, message.id)}
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
            {/snippet}
          </ContextMenu.Trigger>
          <ContextMenu.Content class="w-48">
            {#if hasUnreadContextTarget(message)}
              <ContextMenu.Item
                onSelect={() =>
                  void onMarkMessageRead(getContextTargetIds(message))}
              >
                <MailOpen size={15} />
                Mark as read
              </ContextMenu.Item>
            {/if}
            {#if hasReadContextTarget(message)}
              <ContextMenu.Item
                onSelect={() =>
                  void onMarkMessageUnread(getContextTargetIds(message))}
              >
                <Mail size={15} />
                Mark as unread
              </ContextMenu.Item>
            {/if}
            <ContextMenu.Separator />
            <ContextMenu.Item
              disabled={isPermanentDeleteFolder}
              onSelect={() =>
                void onMoveMessageToTrash(getContextTargetIds(message))}
            >
              <Trash2 size={15} />
              Move to Trash
            </ContextMenu.Item>
            <ContextMenu.Item
              disabled={isFolderNamed("spam")}
              onSelect={() =>
                void onMarkMessageSpam(getContextTargetIds(message))}
            >
              <OctagonAlert size={15} />
              Mark as spam
            </ContextMenu.Item>
          </ContextMenu.Content>
        </ContextMenu.Root>
      {/each}
    {/if}
  </div>
</section>

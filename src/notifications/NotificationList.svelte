<script lang="ts">
  import { Bell, Check, Clock3, Copy, RotateCcw, X } from "@lucide/svelte";
  import { Button } from "$shared/ui/button/index.js";
  import { formatRelative } from "../app/formatting";
  import type { MailAccount, NotificationSummary } from "../shared/mail.types";

  let {
    notifications,
    accounts,
    selectedNotificationId,
    accountFilter,
    accountColor,
    onAccountFilter,
    onOpen,
    onDismiss,
    onRestore,
  }: {
    notifications: NotificationSummary[];
    accounts: MailAccount[];
    selectedNotificationId: string;
    accountFilter: string;
    accountColor: (accountId: string) => string;
    onAccountFilter: (accountId: string) => void | Promise<void>;
    onOpen: (notification: NotificationSummary) => void | Promise<void>;
    onDismiss: (notificationId: string) => void | Promise<void>;
    onRestore: (notificationId: string) => void | Promise<void>;
  } = $props();

  let copiedId = $state("");

  function expiryLabel(notification: NotificationSummary) {
    if (!notification.expiresAt) return "Security summary";
    if (notification.status === "expired") {
      return `Expired ${formatRelative(notification.expiresAt)}`;
    }
    const minutes = Math.max(
      1,
      Math.ceil((Date.parse(notification.expiresAt) - Date.now()) / 60_000),
    );
    return `Expires in ${minutes}m`;
  }

  async function copyCode(notification: NotificationSummary) {
    if (!notification.code) return;
    await navigator.clipboard.writeText(notification.code);
    copiedId = notification.id;
    window.setTimeout(() => {
      if (copiedId === notification.id) copiedId = "";
    }, 1_500);
  }
</script>

<section
  class="mail-scrollbar min-h-0 min-w-0 overflow-y-auto bg-white/70 dark:bg-zinc-950/40"
  aria-label="Notifications"
>
  <header
    class="sticky top-0 z-10 border-b border-zinc-200/80 bg-white/90 p-4 backdrop-blur-xl dark:border-zinc-800 dark:bg-zinc-950/90"
  >
    <div class="flex items-start justify-between gap-3">
      <div>
        <h1
          class="text-balance text-lg font-semibold text-zinc-950 dark:text-white"
        >
          Notifications
        </h1>
        <p class="mt-0.5 text-pretty text-xs text-zinc-500 dark:text-zinc-400">
          Temporary access mail and recent login activity.
        </p>
      </div>
      <span
        class="grid size-10 shrink-0 place-items-center rounded-xl bg-violet-100 text-violet-700 dark:bg-violet-950/60 dark:text-violet-200"
      >
        <Bell size={18} />
      </span>
    </div>

    <label class="mt-4 block">
      <span class="sr-only">Filter notifications by account</span>
      <select
        class="h-10 w-full rounded-lg border border-zinc-300 bg-white px-3 text-sm font-medium text-zinc-800 outline-none focus:border-violet-600 focus:ring-2 focus:ring-violet-600/20 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100 dark:focus:border-violet-400 dark:focus:ring-violet-400/20"
        value={accountFilter}
        onchange={(event) => void onAccountFilter(event.currentTarget.value)}
      >
        <option value="">All iCloud accounts</option>
        {#each accounts.filter((account) => account.provider === "icloud") as account (account.id)}
          <option value={account.id}>{account.email}</option>
        {/each}
      </select>
    </label>
  </header>

  {#if notifications.length}
    <div class="grid gap-3 p-3">
      {#each notifications as notification (notification.id)}
        <article
          class={[
            "rounded-2xl p-2 shadow-[0_0_0_1px_rgba(0,0,0,0.06),0_1px_2px_-1px_rgba(0,0,0,0.06),0_2px_4px_rgba(0,0,0,0.04)] dark:shadow-[0_0_0_1px_rgba(255,255,255,0.08)]",
            selectedNotificationId === notification.id
              ? "bg-violet-50 dark:bg-violet-950/30"
              : "bg-white dark:bg-zinc-900",
          ]}
        >
          <button
            class="w-full cursor-pointer rounded-xl p-2 text-left outline-none transition-[background-color,scale] duration-150 hover:bg-zinc-50 active:scale-[0.96] focus-visible:ring-2 focus-visible:ring-violet-500/40 dark:hover:bg-zinc-800/70"
            type="button"
            onclick={() => void onOpen(notification)}
          >
            <div class="flex items-start justify-between gap-3">
              <div class="min-w-0">
                <div
                  class="flex items-center gap-2 text-xs font-medium text-zinc-500 dark:text-zinc-400"
                >
                  <span
                    class="size-2 shrink-0 rounded-full"
                    style:background-color={accountColor(
                      notification.accountId,
                    )}
                  ></span>
                  <span class="truncate">{notification.accountEmail}</span>
                </div>
                <p class="mt-1 truncate text-[11px] text-zinc-500 dark:text-zinc-400">
                  {notification.sender} · Received {formatRelative(
                    notification.receivedAt,
                  )}
                </p>
                <h2
                  class="mt-2 text-pretty text-sm font-semibold text-zinc-950 dark:text-white"
                >
                  {notification.subject}
                </h2>
              </div>
              <span
                class={[
                  "shrink-0 rounded-full px-2 py-1 text-[11px] font-semibold",
                  notification.status === "valid"
                    ? "bg-emerald-100 text-emerald-800 dark:bg-emerald-950 dark:text-emerald-200"
                    : notification.status === "expired"
                      ? "bg-zinc-100 text-zinc-600 dark:bg-zinc-800 dark:text-zinc-300"
                      : "bg-amber-100 text-amber-800 dark:bg-amber-950 dark:text-amber-200",
                ]}
              >
                {notification.status === "valid"
                  ? "Active"
                  : notification.status === "expired"
                    ? "Expired"
                    : "Login"}
              </span>
            </div>
            <p
              class="mt-2 line-clamp-2 text-pretty text-xs leading-5 text-zinc-500 dark:text-zinc-400"
            >
              {notification.preview}
            </p>
            <div
              class="mt-3 flex items-center justify-between gap-2 text-[11px] font-medium text-zinc-500 dark:text-zinc-400"
            >
              <span>{notification.reason}</span>
              <span class="flex shrink-0 items-center gap-1 tabular-nums">
                <Clock3 size={12} />
                {expiryLabel(notification)}
              </span>
            </div>
          </button>

          <div class="mt-1 flex flex-wrap items-center gap-2 px-2 pb-2">
            {#if notification.status === "valid" && notification.code}
              <Button
                variant="secondary"
                class="h-10 min-w-0 flex-1 cursor-pointer transition-transform duration-150 ease-out active:scale-[0.96]"
                onclick={() => void copyCode(notification)}
              >
                {#if copiedId === notification.id}<Check size={15} /> Copied{:else}<Copy
                    size={15}
                  /> Copy {notification.code}{/if}
              </Button>
            {/if}
            <Button
              variant="outline"
              class="h-10 cursor-pointer transition-transform duration-150 ease-out active:scale-[0.96]"
              onclick={() => void onRestore(notification.id)}
            >
              <RotateCcw size={15} />
              {notification.status === "valid" ? "Keep in Inbox" : "Restore"}
            </Button>
            <Button
              variant="ghost"
              class="h-10 cursor-pointer text-zinc-500 transition-transform duration-150 ease-out active:scale-[0.96]"
              aria-label={`Dismiss ${notification.subject}`}
              onclick={() => void onDismiss(notification.id)}
            >
              <X size={15} /> Dismiss
            </Button>
          </div>
        </article>
      {/each}
    </div>
  {:else}
    <div class="grid min-h-80 place-items-center p-8 text-center">
      <div>
        <span
          class="mx-auto grid size-14 place-items-center rounded-2xl bg-zinc-100 text-zinc-400 dark:bg-zinc-900 dark:text-zinc-500"
        >
          <Bell size={25} />
        </span>
        <h2 class="mt-4 text-balance text-base font-semibold">
          Nothing temporary here
        </h2>
        <p
          class="mt-1 max-w-64 text-pretty text-sm text-zinc-500 dark:text-zinc-400"
        >
          Access mail and login summaries will appear after iCloud sync.
        </p>
      </div>
    </div>
  {/if}
</section>

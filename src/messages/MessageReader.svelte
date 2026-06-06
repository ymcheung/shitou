<script lang="ts">
  import {
    CloudOff,
    Mail,
    Paperclip,
    ShieldCheck,
    Trash2,
  } from "@lucide/svelte";
  import { formatBytes } from "../app/formatting";
  import type { MessageDetail } from "../shared/mail.types";

  let {
    message,
    isPermanentDeleteFolder,
    onDeleteMessage,
  }: {
    message: MessageDetail | null;
    isPermanentDeleteFolder: boolean;
    onDeleteMessage: () => void | Promise<void>;
  } = $props();
</script>

<article
  class="mail-scrollbar min-w-0 overflow-y-auto bg-zinc-50/80 backdrop-blur-[12px] dark:bg-zinc-900/80"
>
  {#if message}
    <header class="border-b border-zinc-200 dark:border-zinc-800">
      <div class="mx-auto w-full max-w-[800px] p-6">
        <div class="mb-4 flex flex-wrap items-center justify-between gap-2">
          <div class="flex flex-wrap gap-2">
            <span
              class="inline-flex items-center gap-1 rounded-lg border border-indigo-200 bg-indigo-50 px-2 py-1 text-xs font-semibold text-indigo-700 dark:border-indigo-900/60 dark:bg-indigo-950/40 dark:text-indigo-200"
            >
              <ShieldCheck size={14} /> Read-only
            </span>
            <span
              class="inline-flex items-center gap-1 rounded-lg border border-zinc-200 bg-zinc-50 px-2 py-1 text-xs font-semibold text-zinc-700 dark:border-zinc-900/60 dark:bg-zinc-950/40 dark:text-zinc-200"
            >
              <CloudOff size={14} /> Offline cache
            </span>
          </div>
          <button
            class="inline-flex h-8 cursor-pointer items-center gap-1.5 rounded-lg border border-red-200 bg-zinc-50 px-2.5 text-xs font-semibold text-red-700 transition-colors duration-200 hover:bg-red-50 focus:outline-none focus:ring-2 focus:ring-red-500/30 dark:border-red-950 dark:bg-zinc-900 dark:text-red-300 dark:hover:bg-red-950/40 dark:focus:ring-red-400/30"
            type="button"
            aria-label={isPermanentDeleteFolder
              ? "Delete this mail forever"
              : "Move this mail to Trash"}
            onclick={() => void onDeleteMessage()}
          >
            <Trash2 size={14} />
            {isPermanentDeleteFolder ? "Delete forever" : "Move to Trash"}
          </button>
        </div>
        <h2
          class="text-2xl font-semibold leading-tight tracking-normal text-zinc-950 dark:text-white"
        >
          {message.subject}
        </h2>
        <div class="mt-4 grid gap-1.5 text-sm text-zinc-500 dark:text-zinc-400">
          <div>
            <span class="font-medium text-zinc-700 dark:text-zinc-200"
              >From:</span
            >
            {message.sender}
          </div>
          <div>
            <span class="font-medium text-zinc-700 dark:text-zinc-200">To:</span
            >
            {message.recipients.join(", ")}
          </div>
          <div>
            <span class="font-medium text-zinc-700 dark:text-zinc-200"
              >Received:</span
            >
            {new Date(message.receivedAt).toLocaleString()}
          </div>
        </div>
      </div>
    </header>

    <div class="mx-auto w-full max-w-[800px]">
      <div
        class="prose prose-zinc max-w-none p-6 prose-a:text-zinc-950 dark:prose-invert dark:prose-a:text-zinc-100"
      >
        {@html message.bodyHtml}
      </div>

      {#if message.attachments.length}
        <section class="px-6 pb-6">
          <h3 class="mb-3 text-sm font-semibold">Cached attachments</h3>
          <div class="grid gap-2">
            {#each message.attachments as attachment (attachment.id)}
              <div
                class="flex items-center justify-between rounded-xl border border-zinc-200 bg-zinc-50/70 px-3 py-2 shadow-sm dark:border-zinc-800 dark:bg-zinc-950/35"
              >
                <div class="flex min-w-0 items-center gap-3">
                  <Paperclip class="shrink-0 text-zinc-400" size={17} />
                  <div class="min-w-0">
                    <div class="truncate text-sm font-medium">
                      {attachment.fileName}
                    </div>
                    <div class="text-xs text-zinc-500 dark:text-zinc-400">
                      {attachment.mimeType} · {formatBytes(attachment.byteSize)}
                    </div>
                  </div>
                </div>
              </div>
            {/each}
          </div>
        </section>
      {/if}
    </div>
  {:else}
    <div class="mx-auto flex min-h-full w-full max-w-[800px] flex-col">
      <div class="grid h-full flex-1 place-items-center p-8 text-center">
        <div>
          <div
            class="mx-auto grid size-16 place-items-center rounded-2xl border border-zinc-200 bg-zinc-50/80 shadow-sm dark:border-zinc-800 dark:bg-zinc-950/40"
          >
            <Mail class="text-zinc-400 dark:text-zinc-600" size={32} />
          </div>
          <h2 class="mt-4 text-lg font-semibold">Select a message</h2>
          <p class="mt-1 text-sm text-zinc-500 dark:text-zinc-400">
            Synced messages are stored locally for offline reading.
          </p>
        </div>
      </div>
    </div>
  {/if}
</article>

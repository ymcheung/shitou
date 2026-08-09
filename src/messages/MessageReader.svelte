<script lang="ts">
  import { Mail, Paperclip } from "@lucide/svelte";
  import TrashIcon from "phosphor-svelte/lib/TrashIcon";
  import { formatBytes } from "../app/formatting";
  import SenderAvatar from "./SenderAvatar.svelte";
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
        <h2
          class="text-2xl font-semibold leading-tight tracking-normal text-zinc-950 dark:text-white"
        >
          {message.subject}
        </h2>
        <div class="mt-3 flex justify-end">
          <button
            class="grid size-10 cursor-pointer place-items-center rounded-lg text-zinc-500 transition-[color,background-color,scale] duration-200 hover:bg-red-50 hover:text-red-700 active:scale-[0.96] focus:outline-none focus:ring-2 focus:ring-red-500/30 dark:text-zinc-400 dark:hover:bg-red-950/40 dark:hover:text-red-300 dark:focus:ring-red-400/30"
            type="button"
            aria-label={isPermanentDeleteFolder
              ? "Delete this mail forever"
              : "Move this mail to Trash"}
            title={isPermanentDeleteFolder
              ? "Delete this mail forever"
              : "Move this mail to Trash"}
            onclick={() => void onDeleteMessage()}
          >
            <TrashIcon size={20} />
          </button>
        </div>
        <div class="mt-5 flex min-w-0 items-start gap-3">
          <SenderAvatar
            sender={message.sender}
            avatarUrl={message.senderAvatarUrl}
            size="lg"
          />
          <div
            class="grid min-w-0 gap-1.5 text-sm text-zinc-500 dark:text-zinc-400"
          >
            <div class="min-w-0">
              <span class="font-medium text-zinc-700 dark:text-zinc-200"
                >From:</span
              >
              <span class="break-words">{message.sender}</span>
            </div>
            <div class="min-w-0">
              <span class="font-medium text-zinc-700 dark:text-zinc-200"
                >To:</span
              >
              <span class="break-words">{message.recipients.join(", ")}</span>
            </div>
            <div>
              <span class="font-medium text-zinc-700 dark:text-zinc-200"
                >Received:</span
              >
              {new Date(message.receivedAt).toLocaleString()}
            </div>
          </div>
        </div>
      </div>
    </header>

    <div class="mx-auto w-full max-w-[800px]">
      <div
        class="mail-content prose prose-zinc max-w-none p-6 prose-a:text-zinc-950 dark:prose-invert dark:prose-a:text-zinc-100"
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

<style>
  :global(.dark .mail-content [style*="color: black" i]),
  :global(.dark .mail-content [style*="color:#000" i]),
  :global(.dark .mail-content [style*="color: #000" i]),
  :global(.dark .mail-content [style*="color:rgb(0" i]),
  :global(.dark .mail-content [style*="color: rgb(0" i]),
  :global(.dark .mail-content font[color="black" i]),
  :global(.dark .mail-content font[color="#000" i]),
  :global(.dark .mail-content font[color="#000000" i]) {
    color: #f4f4f5 !important;
  }
</style>

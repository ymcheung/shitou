<script lang="ts">
  import { CloudOff, Mail, Paperclip, ShieldCheck } from '@lucide/svelte';
  import { formatBytes } from '../app/formatting';
  import type { MessageDetail } from '../shared/mail.types';

  let { message }: { message: MessageDetail | null } = $props();
</script>

<article class="mail-scrollbar min-w-0 overflow-y-auto bg-white dark:bg-slate-900">
  {#if message}
    <header class="border-b border-ink-200 p-6 dark:border-slate-800">
      <div class="mb-4 flex flex-wrap gap-2">
        <span class="inline-flex items-center gap-1 rounded-md bg-emerald-50 px-2 py-1 text-xs font-medium text-emerald-700 dark:bg-emerald-950/40 dark:text-emerald-200">
          <ShieldCheck size={14} /> Read-only
        </span>
        <span class="inline-flex items-center gap-1 rounded-md bg-blue-50 px-2 py-1 text-xs font-medium text-blue-700 dark:bg-blue-950/40 dark:text-blue-200">
          <CloudOff size={14} /> Offline cache
        </span>
      </div>
      <h2 class="text-2xl font-semibold leading-tight tracking-normal">{message.subject}</h2>
      <div class="mt-4 grid gap-1 text-sm text-ink-500 dark:text-slate-400">
        <div>
          <span class="font-medium text-ink-700 dark:text-slate-200">From:</span>
          {message.sender}
        </div>
        <div>
          <span class="font-medium text-ink-700 dark:text-slate-200">To:</span>
          {message.recipients.join(', ')}
        </div>
        <div>
          <span class="font-medium text-ink-700 dark:text-slate-200">Received:</span>
          {new Date(message.receivedAt).toLocaleString()}
        </div>
      </div>
    </header>

    <div class="prose prose-ink max-w-none p-6 dark:prose-invert">
      {@html message.bodyHtml}
    </div>

    {#if message.attachments.length}
      <section class="px-6 pb-6">
        <h3 class="mb-3 text-sm font-semibold">Cached attachments</h3>
        <div class="grid gap-2">
          {#each message.attachments as attachment (attachment.id)}
            <div class="flex items-center justify-between rounded-lg border border-ink-200 px-3 py-2 dark:border-slate-800">
              <div class="flex min-w-0 items-center gap-3">
                <Paperclip class="shrink-0 text-ink-400" size={17} />
                <div class="min-w-0">
                  <div class="truncate text-sm font-medium">{attachment.fileName}</div>
                  <div class="text-xs text-ink-500 dark:text-slate-400">
                    {attachment.mimeType} · {formatBytes(attachment.byteSize)}
                  </div>
                </div>
              </div>
            </div>
          {/each}
        </div>
      </section>
    {/if}
  {:else}
    <div class="grid h-full place-items-center p-8 text-center">
      <div>
        <Mail class="mx-auto text-ink-300 dark:text-slate-700" size={48} />
        <h2 class="mt-4 text-lg font-semibold">Select a message</h2>
        <p class="mt-1 text-sm text-ink-500 dark:text-slate-400">Synced messages are stored locally for offline reading.</p>
      </div>
    </div>
  {/if}
</article>

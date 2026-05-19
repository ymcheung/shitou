<script lang="ts">
  import { AlertCircle, CheckCircle2, KeyRound, Loader2, Mail, ShieldCheck, UserRoundCheck } from '@lucide/svelte';

  let {
    email = $bindable(''),
    magicLinkSent,
    busy,
    error,
    onSendMagicLink,
    onCompleteMagicLink,
    onStartDemo
  }: {
    email: string;
    magicLinkSent: boolean;
    busy: boolean;
    error: string;
    onSendMagicLink: () => void | Promise<void>;
    onCompleteMagicLink: () => void | Promise<void>;
    onStartDemo: () => void | Promise<void>;
  } = $props();
</script>

<main class="grid h-screen place-items-center overflow-hidden bg-[linear-gradient(135deg,#fafafa_0%,#f4f4f5_48%,#eef2ff_100%)] px-6 text-zinc-950 dark:bg-[linear-gradient(135deg,#09090b_0%,#18181b_52%,#1e1b4b_100%)] dark:text-zinc-50">
  <section class="w-full max-w-md">
    <div class="mb-8 flex items-center gap-3">
      <div class="grid size-12 place-items-center rounded-xl bg-indigo-600 text-white shadow-soft ring-1 ring-indigo-500/20 dark:bg-indigo-500 dark:ring-indigo-300/20">
        <Mail size={24} />
      </div>
      <div>
        <h1 class="text-2xl font-semibold tracking-normal text-zinc-950 dark:text-white">Shitou Mail</h1>
        <p class="mt-1 text-sm font-medium text-zinc-600 dark:text-zinc-300">Read-only desktop mail for macOS</p>
      </div>
    </div>

    <div class="rounded-xl border border-zinc-200/90 bg-white/95 p-5 shadow-panel backdrop-blur dark:border-zinc-700/80 dark:bg-zinc-950/80">
      <div class="flex items-start gap-3 rounded-lg border border-indigo-200 bg-indigo-50/90 p-3 text-sm leading-6 text-zinc-800 dark:border-indigo-400/25 dark:bg-indigo-400/10 dark:text-zinc-100">
        <ShieldCheck class="mt-0.5 shrink-0 text-indigo-700 dark:text-indigo-300" size={18} />
        <p>
          Registration and sign-in use email magic links only. Password, social, calendar, reminder, contact, and notification permissions
          are not part of this app.
        </p>
      </div>

      <form
        class="mt-5 space-y-4"
        onsubmit={(event) => {
          event.preventDefault();
          void onSendMagicLink();
        }}
      >
        <label class="block">
          <span class="text-sm font-medium text-zinc-800 dark:text-zinc-100">Email address</span>
          <input
            class="mt-2 h-11 w-full rounded-lg border-zinc-300 bg-white text-zinc-950 shadow-sm placeholder:text-zinc-400 focus:border-indigo-500 focus:ring-indigo-500 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-50 dark:placeholder:text-zinc-500 dark:focus:border-indigo-400 dark:focus:ring-indigo-400"
            type="email"
            autocomplete="email"
            placeholder="you@example.com"
            bind:value={email}
            required
          />
        </label>

        {#if error}
          <p class="flex items-center gap-2 text-sm text-red-600 dark:text-red-400">
            <AlertCircle size={16} />
            {error}
          </p>
        {/if}

        <button
          class="inline-flex h-11 w-full cursor-pointer items-center justify-center gap-2 rounded-lg bg-indigo-600 px-4 text-sm font-semibold text-white shadow-sm shadow-indigo-600/20 hover:bg-indigo-500 focus-visible:outline-indigo-700 disabled:cursor-not-allowed disabled:opacity-60 dark:bg-indigo-500 dark:hover:bg-indigo-400 dark:focus-visible:outline-indigo-300"
          disabled={busy || !email}
          type="submit"
        >
          {#if busy}<Loader2 class="animate-spin" size={16} />{:else}<KeyRound size={16} />{/if}
          Send magic link
        </button>
      </form>

      {#if magicLinkSent}
        <div class="mt-5 rounded-lg border border-indigo-200 bg-indigo-50 p-3 text-sm text-indigo-950 dark:border-indigo-400/25 dark:bg-indigo-400/10 dark:text-indigo-100">
          <div class="flex items-center gap-2 font-medium">
            <CheckCircle2 size={16} /> Magic link sent
          </div>
          <button
            class="mt-3 cursor-pointer text-sm font-semibold text-indigo-700 hover:text-indigo-600 dark:text-indigo-200 dark:hover:text-indigo-100"
            type="button"
            onclick={() => void onCompleteMagicLink()}
          >
            Complete sign-in from callback
          </button>
        </div>
      {/if}

      <div class="mt-5 border-t border-zinc-200 pt-5 dark:border-zinc-800">
        <button
          class="inline-flex h-11 w-full cursor-pointer items-center justify-center gap-2 rounded-lg border border-zinc-300 bg-white px-4 text-sm font-semibold text-zinc-800 shadow-sm hover:border-zinc-400 hover:bg-zinc-100 disabled:cursor-not-allowed disabled:opacity-60 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100 dark:hover:border-zinc-600 dark:hover:bg-zinc-800"
          disabled={busy}
          type="button"
          onclick={() => void onStartDemo()}
        >
          {#if busy}<Loader2 class="animate-spin" size={16} />{:else}<UserRoundCheck size={16} />{/if}
          Continue in demo mode
        </button>
        <p class="mt-2 text-xs leading-5 text-zinc-600 dark:text-zinc-400">
          Uses a fake user and mailbox data. Adding mail accounts is disabled in demo mode.
        </p>
      </div>
    </div>
  </section>
</main>

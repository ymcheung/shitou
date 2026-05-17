<script lang="ts">
  import { AlertCircle, CheckCircle2, KeyRound, Loader2, Mail, ShieldCheck } from '@lucide/svelte';

  let {
    email = $bindable(''),
    magicLinkSent,
    busy,
    error,
    onSendMagicLink,
    onCompleteDemoSignIn
  }: {
    email: string;
    magicLinkSent: boolean;
    busy: boolean;
    error: string;
    onSendMagicLink: () => void | Promise<void>;
    onCompleteDemoSignIn: () => void | Promise<void>;
  } = $props();
</script>

<main class="grid h-screen place-items-center bg-ink-50 px-6 text-ink-900 dark:bg-slate-950 dark:text-slate-100">
  <section class="w-full max-w-md">
    <div class="mb-8 flex items-center gap-3">
      <div class="grid size-11 place-items-center rounded-lg bg-signal-600 text-white shadow-soft">
        <Mail size={24} />
      </div>
      <div>
        <h1 class="text-2xl font-semibold tracking-normal">Shitou Mail</h1>
        <p class="mt-1 text-sm text-ink-500 dark:text-slate-400">Read-only desktop mail for macOS</p>
      </div>
    </div>

    <div class="rounded-lg border border-ink-200 bg-white p-5 shadow-soft dark:border-slate-800 dark:bg-slate-900">
      <div class="flex items-start gap-3 rounded-md bg-blue-50 p-3 text-sm text-blue-900 dark:bg-blue-950/50 dark:text-blue-100">
        <ShieldCheck class="mt-0.5 shrink-0" size={18} />
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
          <span class="text-sm font-medium">Email address</span>
          <input
            class="mt-2 w-full rounded-md border-ink-200 bg-white text-ink-900 placeholder:text-ink-400 focus:border-signal-500 focus:ring-signal-500 dark:border-slate-700 dark:bg-slate-950 dark:text-slate-100"
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
          class="inline-flex h-10 w-full items-center justify-center gap-2 rounded-md bg-signal-600 px-4 text-sm font-semibold text-white hover:bg-signal-500 disabled:cursor-not-allowed disabled:opacity-60"
          disabled={busy || !email}
          type="submit"
        >
          {#if busy}<Loader2 class="animate-spin" size={16} />{:else}<KeyRound size={16} />{/if}
          Send magic link
        </button>
      </form>

      {#if magicLinkSent}
        <div class="mt-5 rounded-md border border-emerald-200 bg-emerald-50 p-3 text-sm text-emerald-900 dark:border-emerald-900/70 dark:bg-emerald-950/40 dark:text-emerald-100">
          <div class="flex items-center gap-2 font-medium">
            <CheckCircle2 size={16} /> Magic link sent
          </div>
          <button
            class="mt-3 text-sm font-semibold text-emerald-700 hover:text-emerald-600 dark:text-emerald-300"
            type="button"
            onclick={() => void onCompleteDemoSignIn()}
          >
            Open demo callback
          </button>
        </div>
      {/if}
    </div>
  </section>
</main>

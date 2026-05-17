<script lang="ts">
  import { Apple, LogOut, Monitor, Moon, Palette, ShieldAlert, Sun, Trash2, UserPlus, X } from '@lucide/svelte';
  import { accountColors } from '../accounts/account-colors';
  import { themeOptions } from '../app/theme';
  import type { MailAccount, Provider, SettingsTab, ThemeMode } from '../shared/mail.types';

  const tabs: Array<{ id: SettingsTab; label: string; icon: typeof Monitor }> = [
    { id: 'general', label: 'General', icon: Monitor },
    { id: 'accounts', label: 'Accounts', icon: UserPlus },
    { id: 'advanced', label: 'Advanced', icon: ShieldAlert }
  ];

  let {
    open = $bindable(false),
    tab = $bindable<SettingsTab>('general'),
    icloudEmail = $bindable(''),
    icloudPassword = $bindable(''),
    theme,
    accounts,
    accountColor,
    onChangeTheme,
    onLogout,
    onConnectProvider,
    onConnectIcloud,
    onRemoveAccount,
    onUpdateAccountColor,
    onDeleteUserAccount
  }: {
    open: boolean;
    tab: SettingsTab;
    icloudEmail: string;
    icloudPassword: string;
    theme: ThemeMode;
    accounts: MailAccount[];
    accountColor: (accountId: string) => string;
    onChangeTheme: (theme: ThemeMode) => void | Promise<void>;
    onLogout: () => void;
    onConnectProvider: (provider: Exclude<Provider, 'icloud'>) => void | Promise<void>;
    onConnectIcloud: () => void | Promise<void>;
    onRemoveAccount: (accountId: string) => void | Promise<void>;
    onUpdateAccountColor: (accountId: string, color: string) => void;
    onDeleteUserAccount: () => void;
  } = $props();
</script>

{#if open}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/35 p-4 backdrop-blur-sm"
    role="presentation"
    onclick={(event) => {
      if (event.target === event.currentTarget) open = false;
    }}
  >
    <div
      class="flex max-h-[88vh] w-full max-w-4xl overflow-hidden rounded-lg border border-ink-200 bg-white shadow-soft dark:border-slate-800 dark:bg-slate-900"
      role="dialog"
      aria-modal="true"
      aria-labelledby="settings-title"
    >
      <div class="w-56 shrink-0 border-r border-ink-200 bg-ink-50 p-3 dark:border-slate-800 dark:bg-slate-950">
        <div class="mb-3 flex items-center justify-between px-2">
          <div>
            <h2 id="settings-title" class="text-sm font-semibold">Settings</h2>
            <p class="mt-0.5 text-xs text-ink-500 dark:text-slate-400">Mail preferences</p>
          </div>
          <button
            class="grid size-8 cursor-pointer place-items-center rounded-md text-ink-500 transition-colors duration-200 hover:bg-ink-100 hover:text-ink-950 focus:outline-none focus:ring-2 focus:ring-signal-500 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-white"
            type="button"
            aria-label="Close settings"
            onclick={() => (open = false)}
          >
            <X size={17} />
          </button>
        </div>

        <div class="space-y-1" role="tablist" aria-label="Settings tabs">
          {#each tabs as settingsTab (settingsTab.id)}
            {@const TabIcon = settingsTab.icon}
            <button
              class={[
                'flex h-10 w-full cursor-pointer items-center gap-2 rounded-md px-3 text-sm font-medium transition-colors duration-200 focus:outline-none focus:ring-2 focus:ring-signal-500',
                tab === settingsTab.id
                  ? 'bg-signal-600 text-white'
                  : 'text-ink-600 hover:bg-white hover:text-ink-950 dark:text-slate-300 dark:hover:bg-slate-900 dark:hover:text-white'
              ]}
              type="button"
              role="tab"
              aria-selected={tab === settingsTab.id}
              onclick={() => (tab = settingsTab.id)}
            >
              <TabIcon size={16} />
              {settingsTab.label}
            </button>
          {/each}
        </div>
      </div>

      <div class="mail-scrollbar min-w-0 flex-1 overflow-y-auto p-6">
        {#if tab === 'general'}
          <section class="space-y-6">
            <div>
              <h3 class="text-lg font-semibold">General</h3>
              <p class="mt-1 text-sm text-ink-500 dark:text-slate-400">Set the app appearance and session state.</p>
            </div>

            <div>
              <div class="mb-2 text-sm font-semibold">Appearance</div>
              <div class="grid grid-cols-3 gap-2 rounded-lg border border-ink-200 bg-ink-50 p-1 dark:border-slate-800 dark:bg-slate-950">
                {#each themeOptions as option (option.value)}
                  <button
                    class={[
                      'inline-flex h-10 cursor-pointer items-center justify-center gap-2 rounded-md text-sm font-semibold transition-colors duration-200 focus:outline-none focus:ring-2 focus:ring-signal-500',
                      theme === option.value
                        ? 'bg-white text-ink-950 shadow-sm dark:bg-slate-800 dark:text-white'
                        : 'text-ink-600 hover:bg-white/70 dark:text-slate-300 dark:hover:bg-slate-900'
                    ]}
                    type="button"
                    onclick={() => void onChangeTheme(option.value)}
                  >
                    {#if option.value === 'dark'}<Moon size={16} />{:else if option.value === 'light'}<Sun size={16} />{:else}<Monitor size={16} />{/if}
                    {option.label}
                  </button>
                {/each}
              </div>
            </div>

            <div class="rounded-lg border border-ink-200 p-4 dark:border-slate-800">
              <div class="flex items-center justify-between gap-4">
                <div>
                  <div class="text-sm font-semibold">Session</div>
                  <p class="mt-1 text-sm text-ink-500 dark:text-slate-400">Log out of the current local mail session.</p>
                </div>
                <button
                  class="inline-flex h-10 cursor-pointer items-center gap-2 rounded-md border border-ink-200 bg-white px-3 text-sm font-semibold text-ink-700 transition-colors duration-200 hover:bg-ink-50 focus:outline-none focus:ring-2 focus:ring-signal-500 dark:border-slate-700 dark:bg-slate-950 dark:text-slate-200 dark:hover:bg-slate-800"
                  type="button"
                  onclick={onLogout}
                >
                  <LogOut size={16} /> Log out
                </button>
              </div>
            </div>
          </section>
        {:else if tab === 'accounts'}
          <section class="space-y-6">
            <div>
              <h3 class="text-lg font-semibold">Accounts</h3>
              <p class="mt-1 text-sm text-ink-500 dark:text-slate-400">Add mailboxes, remove mailboxes, and tune account colors.</p>
            </div>

            <div class="rounded-lg border border-ink-200 bg-ink-50 p-4 dark:border-slate-800 dark:bg-slate-950">
              <div class="text-sm font-semibold">Add account</div>
              <div class="mt-3 grid gap-2 sm:grid-cols-2">
                <button
                  class="h-10 cursor-pointer rounded-md border border-ink-200 bg-white px-3 text-sm font-semibold text-ink-700 transition-colors duration-200 hover:bg-ink-50 focus:outline-none focus:ring-2 focus:ring-signal-500 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
                  type="button"
                  onclick={() => void onConnectProvider('gmail')}>Gmail</button
                >
                <button
                  class="h-10 cursor-pointer rounded-md border border-ink-200 bg-white px-3 text-sm font-semibold text-ink-700 transition-colors duration-200 hover:bg-ink-50 focus:outline-none focus:ring-2 focus:ring-signal-500 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
                  type="button"
                  onclick={() => void onConnectProvider('outlook')}>Outlook</button
                >
              </div>
              <div class="mt-3 grid gap-2 md:grid-cols-[1fr_1fr_auto]">
                <label class="sr-only" for="settings-icloud-email">iCloud email</label>
                <input
                  id="settings-icloud-email"
                  class="h-10 rounded-md border-ink-200 text-sm focus:border-signal-500 focus:ring-signal-500 dark:border-slate-700 dark:bg-slate-900"
                  type="email"
                  placeholder="name@icloud.com"
                  bind:value={icloudEmail}
                />
                <label class="sr-only" for="settings-icloud-password">iCloud app-specific password</label>
                <input
                  id="settings-icloud-password"
                  class="h-10 rounded-md border-ink-200 text-sm focus:border-signal-500 focus:ring-signal-500 dark:border-slate-700 dark:bg-slate-900"
                  type="password"
                  placeholder="App-specific password"
                  bind:value={icloudPassword}
                />
                <button
                  class="inline-flex h-10 cursor-pointer items-center justify-center gap-2 rounded-md bg-ink-900 px-3 text-sm font-semibold text-white transition-colors duration-200 hover:bg-ink-800 disabled:cursor-not-allowed disabled:opacity-50 dark:bg-white dark:text-slate-950"
                  type="button"
                  disabled={!icloudEmail || !icloudPassword}
                  onclick={() => void onConnectIcloud()}
                >
                  <Apple size={16} /> Connect
                </button>
              </div>
            </div>

            <div class="space-y-3">
              {#each accounts as account (account.id)}
                <div class="rounded-lg border border-ink-200 p-4 dark:border-slate-800">
                  <div class="flex items-start justify-between gap-4">
                    <div class="min-w-0">
                      <div class="flex min-w-0 items-center gap-2">
                        <span class="size-3 shrink-0 rounded-full" style:background-color={accountColor(account.id)}></span>
                        <div class="truncate text-sm font-semibold">{account.displayName}</div>
                      </div>
                      <div class="mt-1 truncate text-sm text-ink-500 dark:text-slate-400">{account.email}</div>
                    </div>
                    <button
                      class="inline-flex h-9 cursor-pointer items-center gap-2 rounded-md border border-red-200 bg-white px-3 text-sm font-semibold text-red-700 transition-colors duration-200 hover:bg-red-50 focus:outline-none focus:ring-2 focus:ring-red-500 dark:border-red-950 dark:bg-slate-950 dark:text-red-300 dark:hover:bg-red-950/40"
                      type="button"
                      onclick={() => void onRemoveAccount(account.id)}
                    >
                      <Trash2 size={15} /> Remove
                    </button>
                  </div>
                  <div class="mt-4 flex flex-wrap items-center gap-2">
                    <span class="inline-flex items-center gap-1 text-xs font-semibold uppercase tracking-wide text-ink-500 dark:text-slate-400">
                      <Palette size={14} /> Color
                    </span>
                    {#each accountColors as color (color)}
                      <button
                        class={[
                          'size-7 cursor-pointer rounded-full border-2 transition-colors duration-200 focus:outline-none focus:ring-2 focus:ring-signal-500',
                          accountColor(account.id) === color ? 'border-ink-900 dark:border-white' : 'border-transparent hover:border-ink-300 dark:hover:border-slate-600'
                        ]}
                        style:background-color={color}
                        type="button"
                        aria-label={`Set ${account.displayName} color to ${color}`}
                        onclick={() => onUpdateAccountColor(account.id, color)}
                      ></button>
                    {/each}
                  </div>
                </div>
              {/each}
            </div>
          </section>
        {:else}
          <section class="space-y-6">
            <div>
              <h3 class="text-lg font-semibold">Advanced</h3>
              <p class="mt-1 text-sm text-ink-500 dark:text-slate-400">Destructive account controls for this device.</p>
            </div>
            <div class="rounded-lg border border-red-200 bg-red-50 p-4 dark:border-red-950 dark:bg-red-950/30">
              <div class="flex items-start justify-between gap-4">
                <div>
                  <div class="flex items-center gap-2 text-sm font-semibold text-red-900 dark:text-red-100">
                    <ShieldAlert size={17} /> Delete account
                  </div>
                  <p class="mt-1 text-sm text-red-700 dark:text-red-200">
                    Clears the local Shitou Mail session and removes the connected mailboxes from this device.
                  </p>
                </div>
                <button
                  class="inline-flex h-10 shrink-0 cursor-pointer items-center gap-2 rounded-md bg-red-600 px-3 text-sm font-semibold text-white transition-colors duration-200 hover:bg-red-500 focus:outline-none focus:ring-2 focus:ring-red-500"
                  type="button"
                  onclick={onDeleteUserAccount}
                >
                  <Trash2 size={16} /> Delete Account
                </button>
              </div>
            </div>
          </section>
        {/if}
      </div>
    </div>
  </div>
{/if}

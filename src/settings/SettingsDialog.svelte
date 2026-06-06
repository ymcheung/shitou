<script lang="ts">
  import { Apple, LogOut, Monitor, Moon, Palette, ShieldAlert, Sun, Trash2, UserPlus } from '@lucide/svelte';
  import * as Sheet from '$shared/ui/sheet/index.js';
  import { accountColors } from '../accounts/account-colors';
  import { themeOptions } from '../app/theme';
  import type { MailAccount, Provider, SettingsTab, ThemeMode } from '../shared/mail.types';

  const tabs: Array<{ id: SettingsTab; label: string; icon: typeof Monitor }> = [
    { id: 'general', label: 'General', icon: Monitor },
    { id: 'accounts', label: 'Mail Accounts', icon: UserPlus },
    { id: 'advanced', label: 'Advanced', icon: ShieldAlert }
  ];

  let {
    open = $bindable(false),
    tab = $bindable<SettingsTab>('general'),
    icloudEmail = $bindable(''),
    icloudPassword = $bindable(''),
    theme,
    accounts,
    canAddAccounts = true,
    isDemoMode = false,
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
    canAddAccounts: boolean;
    isDemoMode: boolean;
    accountColor: (accountId: string) => string;
    onChangeTheme: (theme: ThemeMode) => void | Promise<void>;
    onLogout: () => void | Promise<void>;
    onConnectProvider: (provider: Exclude<Provider, 'icloud'>) => void | Promise<void>;
    onConnectIcloud: () => void | Promise<void>;
    onRemoveAccount: (accountId: string) => void | Promise<void>;
    onUpdateAccountColor: (accountId: string, color: string) => void;
    onDeleteUserAccount: () => void | Promise<void>;
  } = $props();
</script>

<Sheet.Root bind:open>
  <Sheet.Content
    side="right"
    class="border-zinc-200 bg-zinc-50 p-0 shadow-panel data-[side=right]:w-full data-[side=right]:sm:max-w-[860px] dark:border-zinc-800 dark:bg-zinc-900"
  >
    <div class="flex h-full min-h-0 flex-col overflow-hidden sm:flex-row">
      <div class="shrink-0 border-b border-zinc-200 bg-zinc-50 p-3 pr-14 sm:w-56 sm:border-b-0 sm:border-r sm:pr-3 dark:border-zinc-800 dark:bg-zinc-950">
        <Sheet.Header class="mb-3 px-1 py-0">
          <Sheet.Title class="text-sm font-semibold text-zinc-950 dark:text-white">Settings</Sheet.Title>
          <Sheet.Description class="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">Mail preferences</Sheet.Description>
        </Sheet.Header>

        <div class="flex gap-1 overflow-x-auto sm:block sm:space-y-1 sm:overflow-visible" role="tablist" aria-label="Settings tabs">
          {#each tabs as settingsTab (settingsTab.id)}
            {@const TabIcon = settingsTab.icon}
            <button
              class={[
                'flex h-9 shrink-0 cursor-pointer items-center gap-2 rounded-md px-3 text-sm font-medium transition-colors duration-200 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2 focus:ring-offset-zinc-50 sm:w-full dark:focus:ring-offset-zinc-950',
                tab === settingsTab.id
                  ? 'bg-indigo-600 text-white shadow-sm'
                  : 'text-zinc-600 hover:bg-zinc-100 hover:text-zinc-950 dark:text-zinc-300 dark:hover:bg-zinc-900 dark:hover:text-white'
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

      <div class="mail-scrollbar min-w-0 flex-1 overflow-y-auto bg-zinc-50 p-4 sm:p-6 dark:bg-zinc-900">
        {#if tab === 'general'}
          <section class="space-y-5">
            <div class="border-b border-zinc-200 pb-4 dark:border-zinc-800">
              <h3 class="text-lg font-semibold text-zinc-950 dark:text-white">General</h3>
              <p class="mt-1 text-sm text-zinc-500 dark:text-zinc-400">Set the app appearance and session state.</p>
            </div>

            <div class="grid gap-3 sm:grid-cols-[11rem_1fr] sm:items-start">
              <div>
                <div class="text-sm font-semibold text-zinc-950 dark:text-white">Appearance</div>
                <p class="mt-1 text-xs leading-5 text-zinc-500 dark:text-zinc-400">Choose the window color mode.</p>
              </div>
              <div class="grid grid-cols-3 gap-1 rounded-lg border border-zinc-200 bg-zinc-50 p-1 dark:border-zinc-800 dark:bg-zinc-950">
                {#each themeOptions as option (option.value)}
                  <button
                    class={[
                      'inline-flex h-9 cursor-pointer items-center justify-center gap-2 rounded-md text-sm font-semibold transition-colors duration-200 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2 focus:ring-offset-zinc-50 dark:focus:ring-offset-zinc-950',
                      theme === option.value
                        ? 'bg-zinc-50 text-zinc-950 shadow-sm dark:bg-zinc-800 dark:text-white'
                        : 'text-zinc-600 hover:bg-zinc-100/70 dark:text-zinc-300 dark:hover:bg-zinc-900'
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

            <div class="grid gap-3 border-t border-zinc-200 pt-5 sm:grid-cols-[11rem_1fr] sm:items-center dark:border-zinc-800">
              <div>
                <div class="text-sm font-semibold text-zinc-950 dark:text-white">Session</div>
                <p class="mt-1 text-xs leading-5 text-zinc-500 dark:text-zinc-400">Current local mail session.</p>
              </div>
              <div class="flex items-center justify-between gap-4 rounded-lg border border-zinc-200 bg-zinc-50/60 px-3 py-3 dark:border-zinc-800 dark:bg-zinc-950/60">
                <div>
                  <div class="text-sm font-medium text-zinc-800 dark:text-zinc-200">Signed in</div>
                  <p class="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">Log out from this device.</p>
                </div>
                <button
                  class="inline-flex h-9 shrink-0 cursor-pointer items-center gap-2 rounded-md border border-zinc-200 bg-zinc-50 px-3 text-sm font-semibold text-zinc-700 transition-colors duration-200 hover:border-zinc-300 hover:bg-zinc-100 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2 focus:ring-offset-zinc-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-200 dark:hover:bg-zinc-800 dark:focus:ring-offset-zinc-950"
                  type="button"
                  onclick={onLogout}
                >
                  <LogOut size={16} /> Log out
                </button>
              </div>
            </div>
          </section>
        {:else if tab === 'accounts'}
          <section class="space-y-5">
            <div class="border-b border-zinc-200 pb-4 dark:border-zinc-800">
              <h3 class="text-lg font-semibold text-zinc-950 dark:text-white">Mail Accounts</h3>
              <p class="mt-1 text-sm text-zinc-500 dark:text-zinc-400">Add mailboxes, remove mailboxes, and tune account colors.</p>
            </div>

            <div class="grid gap-3 sm:grid-cols-[11rem_1fr] sm:items-start">
              <div>
                <div class="text-sm font-semibold text-zinc-950 dark:text-white">Add mail account</div>
                <p class="mt-1 text-xs leading-5 text-zinc-500 dark:text-zinc-400">Connect a mailbox provider.</p>
              </div>
              {#if isDemoMode || !canAddAccounts}
                <p class="rounded-lg border border-zinc-200 bg-zinc-50 px-3 py-2 text-sm text-zinc-500 dark:border-zinc-800 dark:bg-zinc-950 dark:text-zinc-400">
                  Adding accounts is unavailable while using fake demo data.
                </p>
              {:else}
                <div class="space-y-2">
                  <div class="grid gap-2 sm:grid-cols-2">
                    <button
                      class="h-9 cursor-pointer rounded-md border border-zinc-200 bg-zinc-50 px-3 text-sm font-semibold text-zinc-700 transition-colors duration-200 hover:border-indigo-200 hover:bg-indigo-50 hover:text-indigo-700 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2 focus:ring-offset-zinc-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-200 dark:hover:bg-zinc-800 dark:hover:text-white dark:focus:ring-offset-zinc-900"
                      type="button"
                      onclick={() => void onConnectProvider('gmail')}>Gmail</button
                    >
                    <button
                      class="h-9 cursor-pointer rounded-md border border-zinc-200 bg-zinc-50 px-3 text-sm font-semibold text-zinc-700 transition-colors duration-200 hover:border-indigo-200 hover:bg-indigo-50 hover:text-indigo-700 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2 focus:ring-offset-zinc-50 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-200 dark:hover:bg-zinc-800 dark:hover:text-white dark:focus:ring-offset-zinc-900"
                      type="button"
                      onclick={() => void onConnectProvider('outlook')}>Outlook</button
                    >
                  </div>
                  <div class="grid gap-2 md:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto]">
                    <label class="sr-only" for="settings-icloud-email">iCloud email</label>
                    <input
                      id="settings-icloud-email"
                      class="h-9 rounded-md border-zinc-200 bg-zinc-50 text-sm text-zinc-950 placeholder:text-zinc-400 focus:border-indigo-500 focus:ring-indigo-500 dark:border-zinc-700 dark:bg-zinc-950 dark:text-white dark:placeholder:text-zinc-500"
                      type="email"
                      placeholder="name@icloud.com"
                      bind:value={icloudEmail}
                    />
                    <label class="sr-only" for="settings-icloud-password">iCloud app-specific password</label>
                    <input
                      id="settings-icloud-password"
                      class="h-9 rounded-md border-zinc-200 bg-zinc-50 text-sm text-zinc-950 placeholder:text-zinc-400 focus:border-indigo-500 focus:ring-indigo-500 dark:border-zinc-700 dark:bg-zinc-950 dark:text-white dark:placeholder:text-zinc-500"
                      type="password"
                      placeholder="App-specific password"
                      bind:value={icloudPassword}
                    />
                    <button
                      class="inline-flex h-9 cursor-pointer items-center justify-center gap-2 rounded-md bg-indigo-600 px-3 text-sm font-semibold text-white transition-colors duration-200 hover:bg-indigo-500 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2 focus:ring-offset-zinc-50 disabled:cursor-not-allowed disabled:opacity-50 dark:focus:ring-offset-zinc-900"
                      type="button"
                      disabled={!icloudEmail || !icloudPassword}
                      onclick={() => void onConnectIcloud()}
                    >
                      <Apple size={16} /> Connect
                    </button>
                  </div>
                </div>
              {/if}
            </div>

            <div class="space-y-3 border-t border-zinc-200 pt-5 dark:border-zinc-800">
              <div>
                <div class="text-sm font-semibold text-zinc-950 dark:text-white">Connected mail accounts</div>
                <p class="mt-1 text-xs leading-5 text-zinc-500 dark:text-zinc-400">Mailboxes available in Shitou Mail.</p>
              </div>

              {#if accounts.length === 0}
                <div class="rounded-lg border border-dashed border-zinc-300 bg-zinc-50 px-4 py-6 text-center dark:border-zinc-700 dark:bg-zinc-950/60">
                  <div class="mx-auto grid size-10 place-items-center rounded-lg border border-zinc-200 bg-zinc-100 text-zinc-500 dark:border-zinc-800 dark:bg-zinc-900 dark:text-zinc-400">
                    <UserPlus size={18} />
                  </div>
                  <div class="mt-3 text-sm font-semibold text-zinc-950 dark:text-white">No mail accounts connected</div>
                  <p class="mx-auto mt-1 max-w-sm text-sm leading-6 text-zinc-500 dark:text-zinc-400">
                    {#if isDemoMode || !canAddAccounts}
                      Add a real mail session to connect Gmail, Outlook, or iCloud accounts.
                    {:else}
                      Use the provider controls above to connect your first mailbox.
                    {/if}
                  </p>
                </div>
              {:else}
                <div class="divide-y divide-zinc-200 rounded-lg border border-zinc-200 dark:divide-zinc-800 dark:border-zinc-800">
                  {#each accounts as account (account.id)}
                    <div class="bg-zinc-50 p-3 first:rounded-t-lg last:rounded-b-lg dark:bg-zinc-900">
                      <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                        <div class="min-w-0">
                          <div class="flex min-w-0 items-center gap-2">
                            <span class="size-2.5 shrink-0 rounded-full ring-2 ring-zinc-50 dark:ring-zinc-900" style:background-color={accountColor(account.id)}></span>
                            <div class="truncate text-sm font-semibold text-zinc-950 dark:text-white">
                              {account.displayName}
                            </div>
                          </div>
                          <div class="mt-1 truncate text-sm text-zinc-500 dark:text-zinc-400">
                            {account.email}
                          </div>
                        </div>
                        <button
                          class="inline-flex h-9 shrink-0 cursor-pointer items-center justify-center gap-2 rounded-md border border-red-200 bg-zinc-50 px-3 text-sm font-semibold text-red-700 transition-colors duration-200 hover:bg-red-50 focus:outline-none focus:ring-2 focus:ring-red-500 focus:ring-offset-2 focus:ring-offset-zinc-50 dark:border-red-950 dark:bg-zinc-950 dark:text-red-300 dark:hover:bg-red-950/40 dark:focus:ring-offset-zinc-900"
                          type="button"
                          onclick={() => void onRemoveAccount(account.id)}
                        >
                          <Trash2 size={15} /> Remove
                        </button>
                      </div>
                      <div class="mt-3 flex flex-wrap items-center gap-2">
                        <span class="inline-flex items-center gap-1 text-xs font-semibold text-zinc-500 dark:text-zinc-400">
                          <Palette size={14} /> Color
                        </span>
                        {#each accountColors as color (color)}
                          <button
                            class={[
                              'size-6 cursor-pointer rounded-full border-2 transition-colors duration-200 focus:outline-none focus:ring-2 focus:ring-indigo-500 focus:ring-offset-2 focus:ring-offset-zinc-50 dark:focus:ring-offset-zinc-900',
                              accountColor(account.id) === color ? 'border-zinc-900 dark:border-zinc-200' : 'border-zinc-200 hover:border-zinc-300 dark:border-zinc-900 dark:hover:border-zinc-600'
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
              {/if}
            </div>
          </section>
        {:else}
          <section class="space-y-5">
            <div class="border-b border-zinc-200 pb-4 dark:border-zinc-800">
              <h3 class="text-lg font-semibold text-zinc-950 dark:text-white">Advanced</h3>
              <p class="mt-1 text-sm text-zinc-500 dark:text-zinc-400">Destructive account controls for this device.</p>
            </div>
            <div class="rounded-lg border border-red-200 bg-red-50 p-4 dark:border-red-950 dark:bg-red-950/30">
              <div class="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
                <div>
                  <div class="flex items-center gap-2 text-sm font-semibold text-red-900 dark:text-red-100">
                    <ShieldAlert size={17} /> Delete account
                  </div>
                  <p class="mt-1 text-sm text-red-700 dark:text-red-200">
                    Clears the local Shitou Mail session and removes the connected mailboxes from this device.
                  </p>
                </div>
                <button
                  class="inline-flex h-9 shrink-0 cursor-pointer items-center justify-center gap-2 rounded-md bg-red-600 px-3 text-sm font-semibold text-white transition-colors duration-200 hover:bg-red-500 focus:outline-none focus:ring-2 focus:ring-red-500 focus:ring-offset-2 focus:ring-offset-red-50 dark:focus:ring-offset-red-950"
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
  </Sheet.Content>
</Sheet.Root>

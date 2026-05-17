<script lang="ts">
  import {
    AlertCircle,
    Apple,
    CheckCircle2,
    CloudOff,
    Inbox,
    KeyRound,
    Loader2,
    LogOut,
    Mail,
    Monitor,
    Moon,
    Paperclip,
    Palette,
    Plus,
    RefreshCw,
    Search,
    Settings,
    ShieldAlert,
    ShieldCheck,
    Sun,
    Trash2,
    UserPlus,
    X,
  } from "@lucide/svelte";
  import { api } from "$lib/tauri";
  import type {
    Folder,
    MailAccount,
    MessageDetail,
    MessageSummary,
    Provider,
    ThemeMode,
  } from "$lib/types";

  const providerLabels: Record<Provider, string> = {
    gmail: "Gmail",
    outlook: "Outlook",
    icloud: "iCloud Mail",
  };

  const themeOptions: Array<{ value: ThemeMode; label: string }> = [
    { value: "system", label: "System" },
    { value: "light", label: "Light" },
    { value: "dark", label: "Dark" },
  ];

  const accountColors = [
    "#0d9488",
    "#2563eb",
    "#f97316",
    "#7c3aed",
    "#dc2626",
    "#0891b2",
  ];
  const rootFolderDefinitions = [
    { id: "root:inbox", name: "Inbox", names: ["inbox"] },
    { id: "root:trash", name: "Trash", names: ["trash"] },
    { id: "root:spam", name: "Spam", names: ["spam", "junk"] },
  ];
  const panelHandleWidth = 6;
  const minAccountPanelWidth = 220;
  const maxAccountPanelWidth = 420;
  const minMessageListWidth = 300;
  const maxMessageListWidth = 640;
  const minMessagePanelWidth = 360;

  type ResizeTarget = "accounts" | "message";
  type ResizeState = {
    target: ResizeTarget;
    startX: number;
    startAccountWidth: number;
    startMessageListWidth: number;
  };

  let email = $state("");
  let magicLinkSent = $state(false);
  let authBusy = $state(false);
  let authError = $state("");
  let isSignedIn = $state(false);

  let accounts = $state.raw<MailAccount[]>([]);
  let folders = $state.raw<Folder[]>([]);
  let foldersByAccount = $state.raw<Record<string, Folder[]>>({});
  let messages = $state.raw<MessageSummary[]>([]);
  let selectedMessageIds = $state.raw<string[]>([]);
  let selectedAccountId = $state("");
  let selectedFolderId = $state("");
  let selectedMessage = $state.raw<MessageDetail | null>(null);
  let query = $state("");
  let appBusy = $state(false);
  let appError = $state("");
  let accountPanelOpen = $state(false);
  let settingsOpen = $state(false);
  let settingsTab = $state<"general" | "accounts" | "advanced">("general");
  let icloudEmail = $state("");
  let icloudPassword = $state("");
  let theme = $state<ThemeMode>("system");
  let selectionMode = $state(false);
  let accountColorOverrides = $state<Record<string, string>>({});
  let mailShell = $state<HTMLElement | null>(null);
  let accountPanelWidth = $state(272);
  let messageListWidth = $state(430);
  let activeResize = $state.raw<ResizeState | null>(null);

  let selectedAccount = $derived(
    accounts.find((account) => account.id === selectedAccountId) ?? null,
  );
  let allFolders = $derived(Object.values(foldersByAccount).flat());
  let rootFolders = $derived(
    rootFolderDefinitions.map((definition) => ({
      id: definition.id,
      accountId: "root",
      name: definition.name,
      unreadCount: allFolders
        .filter((folder) =>
          definition.names.includes(folder.name.toLowerCase()),
        )
        .reduce((total, folder) => total + folder.unreadCount, 0),
    })),
  );
  let selectedFolder = $derived(
    [...rootFolders, ...folders].find(
      (folder) => folder.id === selectedFolderId,
    ) ?? null,
  );
  let isPermanentDeleteFolder = $derived(
    selectedFolder
      ? ["trash", "spam", "junk"].includes(selectedFolder.name.toLowerCase())
      : false,
  );
  let unreadTotal = $derived(
    rootFolders.find((folder) => folder.id === "root:inbox")?.unreadCount ?? 0,
  );
  let offlineAccounts = $derived(
    accounts.filter((account) => account.syncStatus === "offline").length,
  );
  let selectedMessageIdSet = $derived(new Set(selectedMessageIds));
  let selectedMessages = $derived(
    messages.filter((message) => selectedMessageIdSet.has(message.id)),
  );
  let allVisibleSelected = $derived(
    messages.length > 0 && selectedMessageIds.length === messages.length,
  );
  let mailGridColumns = $derived(
    `${accountPanelWidth}px ${panelHandleWidth}px ${messageListWidth}px ${panelHandleWidth}px minmax(${minMessagePanelWidth}px, 1fr)`,
  );

  $effect(() => {
    applyTheme(theme);
  });

  $effect(() => {
    if (!("__TAURI_INTERNALS__" in window)) return;

    let unlisten: (() => void) | undefined;
    void import("@tauri-apps/api/event").then(({ listen }) => {
      void listen<"general" | "accounts" | "advanced">(
        "open-settings",
        (event) => openSettings(event.payload ?? "general"),
      ).then((nextUnlisten) => {
        unlisten = nextUnlisten;
      });
    });

    return () => {
      unlisten?.();
    };
  });

  function applyTheme(mode: ThemeMode) {
    const prefersDark = window.matchMedia(
      "(prefers-color-scheme: dark)",
    ).matches;
    document.documentElement.classList.toggle(
      "dark",
      mode === "dark" || (mode === "system" && prefersDark),
    );
  }

  async function sendMagicLink() {
    authBusy = true;
    authError = "";

    try {
      await api.authStartMagicLink(email);
      magicLinkSent = true;
    } catch (error) {
      authError =
        error instanceof Error
          ? error.message
          : "Unable to start magic-link sign-in.";
    } finally {
      authBusy = false;
    }
  }

  async function completeDemoSignIn() {
    authBusy = true;
    authError = "";

    try {
      await api.authCompleteCallback("shitou://auth/callback?token=demo");
      isSignedIn = true;
      await loadMailbox();
    } catch (error) {
      authError =
        error instanceof Error ? error.message : "Unable to complete sign-in.";
    } finally {
      authBusy = false;
    }
  }

  async function loadMailbox() {
    appBusy = true;
    appError = "";

    try {
      accounts = await api.listAccounts();
      foldersByAccount = await loadFoldersByAccount(accounts);
      folders = selectedAccountId
        ? (foldersByAccount[selectedAccountId] ?? [])
        : [];
      if (!selectedFolderId) {
        await loadRootFolder("root:inbox");
      } else if (selectedFolderId.startsWith("root:")) {
        await loadMessages(selectedFolderId);
      } else if (selectedAccountId) {
        await loadFolders(selectedAccountId);
      }
    } catch (error) {
      appError =
        error instanceof Error
          ? error.message
          : "Unable to load local mailbox.";
    } finally {
      appBusy = false;
    }
  }

  async function loadFoldersByAccount(nextAccounts: MailAccount[]) {
    const entries = await Promise.all(
      nextAccounts.map(
        async (account) =>
          [account.id, await api.listFolders(account.id)] as const,
      ),
    );
    return Object.fromEntries(entries);
  }

  async function refreshFolders() {
    foldersByAccount = await loadFoldersByAccount(accounts);
    if (selectedAccountId) {
      folders = foldersByAccount[selectedAccountId] ?? [];
    }
  }

  async function loadRootFolder(folderId: string) {
    selectedAccountId = "";
    folders = [];
    await loadMessages(folderId);
  }

  async function loadFolders(accountId: string) {
    selectedAccountId = accountId;
    folders = foldersByAccount[accountId] ?? (await api.listFolders(accountId));
    foldersByAccount = { ...foldersByAccount, [accountId]: folders };
    selectedFolderId = folders[0]?.id || "";
    selectedMessage = null;
    if (selectedFolderId) await loadMessages(selectedFolderId);
  }

  async function loadMessages(folderId: string) {
    selectedFolderId = folderId;
    messages = await api.listMessages(folderId, query);
    selectedMessageIds = [];
    selectionMode = false;
    selectedMessage = messages[0] ? await api.getMessage(messages[0].id) : null;
  }

  async function searchMessages() {
    if (!selectedFolderId) return;
    messages = await api.listMessages(selectedFolderId, query);
    selectedMessageIds = [];
    selectionMode = false;
    selectedMessage = messages[0] ? await api.getMessage(messages[0].id) : null;
  }

  async function openMessage(messageId: string) {
    const message = messages.find((item) => item.id === messageId);
    if (message?.isUnread) {
      await api.markMessagesRead([messageId]);
      messages = messages.map((item) =>
        item.id === messageId ? { ...item, isUnread: false } : item,
      );
      await refreshFolders();
    }
    const detail = await api.getMessage(messageId);
    selectedMessage = { ...detail, isUnread: false };
  }

  function toggleMessageSelection(messageId: string) {
    selectedMessageIds = selectedMessageIdSet.has(messageId)
      ? selectedMessageIds.filter((id) => id !== messageId)
      : [...selectedMessageIds, messageId];
  }

  function startSelection() {
    selectionMode = true;
  }

  function selectAllVisibleMessages() {
    selectedMessageIds = messages.map((message) => message.id);
  }

  async function markSelectedRead() {
    if (selectedMessageIds.length === 0) return;
    await api.markMessagesRead(selectedMessageIds);
    messages = messages.map((message) =>
      selectedMessageIdSet.has(message.id)
        ? { ...message, isUnread: false }
        : message,
    );
    if (selectedMessage && selectedMessageIdSet.has(selectedMessage.id)) {
      selectedMessage = { ...selectedMessage, isUnread: false };
    }
    selectedMessageIds = [];
    await refreshFolders();
  }

  async function deleteSelectedMessages() {
    if (selectedMessageIds.length === 0) return;
    if (
      isPermanentDeleteFolder &&
      !window.confirm("Permanently delete the selected mail?")
    ) {
      return;
    }
    await api.deleteMessages(selectedMessageIds);
    selectedMessageIds = [];
    await refreshFolders();
    if (selectedFolderId) await loadMessages(selectedFolderId);
  }

  async function syncAll() {
    appBusy = true;
    appError = "";

    try {
      accounts = await api.syncAll();
      await refreshFolders();
      if (selectedFolderId) await loadMessages(selectedFolderId);
    } catch (error) {
      appError = error instanceof Error ? error.message : "Sync failed.";
    } finally {
      appBusy = false;
    }
  }

  async function connectProvider(provider: Exclude<Provider, "icloud">) {
    appBusy = true;
    appError = "";

    try {
      await api.connectProvider(provider);
      accountPanelOpen = false;
    } catch (error) {
      appError =
        error instanceof Error
          ? error.message
          : `Unable to connect ${providerLabels[provider]}.`;
    } finally {
      appBusy = false;
    }
  }

  async function connectIcloud() {
    appBusy = true;
    appError = "";

    try {
      const account = await api.connectIcloud(icloudEmail, icloudPassword);
      accounts = [...accounts, account];
      foldersByAccount = { ...foldersByAccount, [account.id]: [] };
      icloudEmail = "";
      icloudPassword = "";
      accountPanelOpen = false;
    } catch (error) {
      appError =
        error instanceof Error
          ? error.message
          : "Unable to connect iCloud Mail.";
    } finally {
      appBusy = false;
    }
  }

  async function removeAccount(accountId: string) {
    if (!window.confirm("Remove this mail account from Shitou Mail?")) {
      return;
    }
    await api.removeAccount(accountId);
    accounts = accounts.filter((account) => account.id !== accountId);
    const { [accountId]: _removed, ...remainingColors } =
      accountColorOverrides;
    accountColorOverrides = remainingColors;
    if (selectedAccountId === accountId) {
      selectedAccountId = accounts[0]?.id || "";
      folders = [];
      messages = [];
      selectedMessage = null;
      if (selectedAccountId) await loadFolders(selectedAccountId);
      else await loadRootFolder("root:inbox");
    }
  }

  async function changeTheme(nextTheme: ThemeMode) {
    theme = nextTheme;
    await api.setTheme(nextTheme);
  }

  function openSettings(tab: "general" | "accounts" | "advanced" = "general") {
    settingsTab = tab;
    accountPanelOpen = false;
    settingsOpen = true;
  }

  function logout() {
    if (!window.confirm("Log out of Shitou Mail?")) return;
    isSignedIn = false;
    selectedMessage = null;
    selectedMessageIds = [];
    selectionMode = false;
    settingsOpen = false;
  }

  function updateAccountColor(accountId: string, color: string) {
    accountColorOverrides = { ...accountColorOverrides, [accountId]: color };
  }

  function deleteUserAccount() {
    if (
      !window.confirm(
        "Delete this Shitou Mail account from this device? Local demo session data will be cleared.",
      )
    ) {
      return;
    }
    accounts = [];
    folders = [];
    foldersByAccount = {};
    messages = [];
    selectedAccountId = "";
    selectedFolderId = "";
    selectedMessage = null;
    selectedMessageIds = [];
    accountColorOverrides = {};
    settingsOpen = false;
    isSignedIn = false;
  }

  function formatRelative(value: string | null) {
    if (!value) return "Never";
    const minutes = Math.max(
      1,
      Math.round((Date.now() - new Date(value).getTime()) / 60000),
    );
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.round(minutes / 60);
    if (hours < 24) return `${hours}h ago`;
    return `${Math.round(hours / 24)}d ago`;
  }

  function formatBytes(bytes: number) {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${Math.round(bytes / 102.4) / 10} KB`;
    return `${Math.round(bytes / 1024 / 102.4) / 10} MB`;
  }

  function accountColor(accountId: string) {
    if (accountColorOverrides[accountId]) return accountColorOverrides[accountId];
    const index = accounts.findIndex((account) => account.id === accountId);
    return accountColors[Math.max(0, index) % accountColors.length];
  }

  function accountLabel(accountId: string) {
    return (
      accounts.find((account) => account.id === accountId)?.displayName ??
      "Mailbox"
    );
  }

  function folderIcon(folderId: string) {
    if (folderId.includes("trash")) return Trash2;
    if (folderId.includes("spam")) return AlertCircle;
    return Inbox;
  }

  function clamp(value: number, min: number, max: number) {
    return Math.min(Math.max(value, min), max);
  }

  function availablePanelWidth() {
    return (
      mailShell?.getBoundingClientRect().width ??
      document.documentElement.clientWidth
    ) - panelHandleWidth * 2;
  }

  function resizeAccountDivider(nextAccountWidth: number) {
    const totalLeftWidth = accountPanelWidth + messageListWidth;
    accountPanelWidth = clamp(
      nextAccountWidth,
      minAccountPanelWidth,
      Math.min(maxAccountPanelWidth, totalLeftWidth - minMessageListWidth),
    );
    messageListWidth = totalLeftWidth - accountPanelWidth;
  }

  function resizeMessageDivider(nextMessageListWidth: number) {
    const maxWidth = Math.min(
      maxMessageListWidth,
      availablePanelWidth() - accountPanelWidth - minMessagePanelWidth,
    );
    messageListWidth = clamp(
      nextMessageListWidth,
      minMessageListWidth,
      Math.max(minMessageListWidth, maxWidth),
    );
  }

  function startPanelResize(target: ResizeTarget, event: PointerEvent) {
    event.preventDefault();
    activeResize = {
      target,
      startX: event.clientX,
      startAccountWidth: accountPanelWidth,
      startMessageListWidth: messageListWidth,
    };
  }

  function updatePanelResize(event: PointerEvent) {
    if (!activeResize) return;

    const delta = event.clientX - activeResize.startX;
    if (activeResize.target === "accounts") {
      const totalLeftWidth =
        activeResize.startAccountWidth + activeResize.startMessageListWidth;
      accountPanelWidth = clamp(
        activeResize.startAccountWidth + delta,
        minAccountPanelWidth,
        Math.min(maxAccountPanelWidth, totalLeftWidth - minMessageListWidth),
      );
      messageListWidth = totalLeftWidth - accountPanelWidth;
    } else {
      const maxWidth = Math.min(
        maxMessageListWidth,
        availablePanelWidth() -
          activeResize.startAccountWidth -
          minMessagePanelWidth,
      );
      messageListWidth = clamp(
        activeResize.startMessageListWidth + delta,
        minMessageListWidth,
        Math.max(minMessageListWidth, maxWidth),
      );
    }
  }

  function stopPanelResize() {
    activeResize = null;
  }

  function handlePanelResizeKey(target: ResizeTarget, event: KeyboardEvent) {
    if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;

    event.preventDefault();
    const delta = event.key === "ArrowRight" ? 16 : -16;
    if (target === "accounts") {
      resizeAccountDivider(accountPanelWidth + delta);
    } else {
      resizeMessageDivider(messageListWidth + delta);
    }
  }
</script>

<svelte:window
  onpointermove={updatePanelResize}
  onpointerup={stopPanelResize}
  onpointercancel={stopPanelResize}
/>

{#if !isSignedIn}
  <main
    class="grid h-screen place-items-center bg-ink-50 px-6 text-ink-900 dark:bg-slate-950 dark:text-slate-100"
  >
    <section class="w-full max-w-md">
      <div class="mb-8 flex items-center gap-3">
        <div
          class="grid size-11 place-items-center rounded-lg bg-signal-600 text-white shadow-soft"
        >
          <Mail size={24} />
        </div>
        <div>
          <h1 class="text-2xl font-semibold tracking-normal">Shitou Mail</h1>
          <p class="mt-1 text-sm text-ink-500 dark:text-slate-400">
            Read-only desktop mail for macOS
          </p>
        </div>
      </div>

      <div
        class="rounded-lg border border-ink-200 bg-white p-5 shadow-soft dark:border-slate-800 dark:bg-slate-900"
      >
        <div
          class="flex items-start gap-3 rounded-md bg-blue-50 p-3 text-sm text-blue-900 dark:bg-blue-950/50 dark:text-blue-100"
        >
          <ShieldCheck class="mt-0.5 shrink-0" size={18} />
          <p>
            Registration and sign-in use email magic links only. Password,
            social, calendar, reminder, contact, and notification permissions
            are not part of this app.
          </p>
        </div>

        <form
          class="mt-5 space-y-4"
          onsubmit={(event) => {
            event.preventDefault();
            void sendMagicLink();
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

          {#if authError}
            <p
              class="flex items-center gap-2 text-sm text-red-600 dark:text-red-400"
            >
              <AlertCircle size={16} />
              {authError}
            </p>
          {/if}

          <button
            class="inline-flex h-10 w-full items-center justify-center gap-2 rounded-md bg-signal-600 px-4 text-sm font-semibold text-white hover:bg-signal-500 disabled:cursor-not-allowed disabled:opacity-60"
            disabled={authBusy || !email}
            type="submit"
          >
            {#if authBusy}<Loader2
                class="animate-spin"
                size={16}
              />{:else}<KeyRound size={16} />{/if}
            Send magic link
          </button>
        </form>

        {#if magicLinkSent}
          <div
            class="mt-5 rounded-md border border-emerald-200 bg-emerald-50 p-3 text-sm text-emerald-900 dark:border-emerald-900/70 dark:bg-emerald-950/40 dark:text-emerald-100"
          >
            <div class="flex items-center gap-2 font-medium">
              <CheckCircle2 size={16} /> Magic link sent
            </div>
            <button
              class="mt-3 text-sm font-semibold text-emerald-700 hover:text-emerald-600 dark:text-emerald-300"
              type="button"
              onclick={() => void completeDemoSignIn()}
            >
              Open demo callback
            </button>
          </div>
        {/if}
      </div>
    </section>
  </main>
{:else}
  <main
    bind:this={mailShell}
    class={[
      "grid h-screen bg-ink-50 text-ink-900 dark:bg-slate-950 dark:text-slate-100",
      activeResize ? "cursor-col-resize select-none" : "",
    ]}
    style:grid-template-columns={mailGridColumns}
  >
    <aside
      class="flex min-w-0 flex-col border-r border-ink-200 bg-white/80 backdrop-blur dark:border-slate-800 dark:bg-slate-900/90"
    >
      <div
        class="flex h-16 items-center justify-between border-b border-ink-200 px-4 dark:border-slate-800"
      >
        <div class="flex min-w-0 items-center gap-3">
          <div
            class="grid size-9 shrink-0 place-items-center rounded-md bg-signal-600 text-white"
          >
            <Mail size={20} />
          </div>
          <div class="min-w-0">
            <div class="truncate text-sm font-semibold">Shitou Mail</div>
            <div class="truncate text-xs text-ink-500 dark:text-slate-400">
              {unreadTotal} unread
            </div>
          </div>
        </div>
        <button
          class="grid size-8 place-items-center rounded-md text-ink-500 hover:bg-ink-100 hover:text-ink-900 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-white"
          type="button"
          title="Add account"
          onclick={() => (accountPanelOpen = !accountPanelOpen)}
        >
          <Plus size={18} />
        </button>
      </div>

      <div class="mail-scrollbar flex-1 overflow-y-auto p-3">
        {#if accountPanelOpen}
          <section
            class="mb-3 rounded-lg border border-ink-200 bg-ink-50 p-3 dark:border-slate-800 dark:bg-slate-950"
          >
            <div class="text-sm font-semibold">Add read-only account</div>
            <div class="mt-3 grid grid-cols-2 gap-2">
              <button
                class="rounded-md border border-ink-200 bg-white px-3 py-2 text-sm font-medium hover:bg-ink-50 dark:border-slate-700 dark:bg-slate-900 dark:hover:bg-slate-800"
                type="button"
                onclick={() => void connectProvider("gmail")}>Gmail</button
              >
              <button
                class="rounded-md border border-ink-200 bg-white px-3 py-2 text-sm font-medium hover:bg-ink-50 dark:border-slate-700 dark:bg-slate-900 dark:hover:bg-slate-800"
                type="button"
                onclick={() => void connectProvider("outlook")}>Outlook</button
              >
            </div>
            <div class="mt-3 space-y-2">
              <div
                class="block text-xs font-medium text-ink-500 dark:text-slate-400"
              >
                iCloud Mail
              </div>
              <label class="sr-only" for="icloud-email">iCloud email</label>
              <input
                id="icloud-email"
                class="h-9 w-full rounded-md border-ink-200 text-sm dark:border-slate-700 dark:bg-slate-900"
                type="email"
                placeholder="name@icloud.com"
                bind:value={icloudEmail}
              />
              <label class="sr-only" for="icloud-password"
                >iCloud app-specific password</label
              >
              <input
                id="icloud-password"
                class="h-9 w-full rounded-md border-ink-200 text-sm dark:border-slate-700 dark:bg-slate-900"
                type="password"
                placeholder="App-specific password"
                bind:value={icloudPassword}
              />
              <button
                class="inline-flex h-9 w-full items-center justify-center gap-2 rounded-md bg-ink-900 px-3 text-sm font-semibold text-white hover:bg-ink-800 disabled:opacity-50 dark:bg-white dark:text-slate-950"
                type="button"
                disabled={!icloudEmail || !icloudPassword}
                onclick={() => void connectIcloud()}
              >
                <Apple size={16} /> Connect iCloud
              </button>
            </div>
          </section>
        {/if}

        {#if offlineAccounts}
          <div
            class="mb-3 flex items-center gap-2 rounded-md bg-amber-50 px-3 py-2 text-xs text-amber-900 dark:bg-amber-950/40 dark:text-amber-100"
          >
            <CloudOff size={15} />
            {offlineAccounts} account cached for offline reading
          </div>
        {/if}

        {#if appError}
          <div
            class="mb-3 flex items-start gap-2 rounded-md bg-red-50 px-3 py-2 text-xs text-red-700 dark:bg-red-950/40 dark:text-red-200"
          >
            <AlertCircle class="mt-0.5 shrink-0" size={15} />
            {appError}
          </div>
        {/if}

        <div class="mb-4 space-y-1">
          {#each rootFolders as rootFolder (rootFolder.id)}
            {@const RootIcon = folderIcon(rootFolder.id)}
            <button
              class={[
                "flex h-9 w-full items-center justify-between rounded-md px-3 text-sm",
                selectedFolderId === rootFolder.id
                  ? "bg-signal-600 text-white"
                  : "text-ink-700 hover:bg-ink-100 dark:text-slate-200 dark:hover:bg-slate-800",
              ]}
              type="button"
              onclick={() => void loadRootFolder(rootFolder.id)}
            >
              <span class="flex items-center gap-2"
                ><RootIcon size={15} /> {rootFolder.name}</span
              >
              {#if rootFolder.unreadCount}<span class="text-xs font-semibold"
                  >{rootFolder.unreadCount}</span
                >{/if}
            </button>
          {/each}
        </div>

        <div class="space-y-3">
          {#each accounts as account (account.id)}
            <section>
              <div
                class="group flex items-center justify-between gap-2 rounded-md px-2 py-1.5"
              >
                <button
                  class="flex min-w-0 flex-1 items-center gap-2 text-left"
                  type="button"
                  onclick={() => void loadFolders(account.id)}
                >
                  <span
                    class="grid size-7 shrink-0 place-items-center rounded-md bg-ink-100 text-ink-600 dark:bg-slate-800 dark:text-slate-300"
                  >
                    {#if account.provider === "icloud"}<Apple
                        size={16}
                      />{:else}<Mail size={16} />{/if}
                  </span>
                  <span class="min-w-0">
                    <span class="flex min-w-0 items-center gap-2">
                      <span
                        class="size-2 shrink-0 rounded-full"
                        style:background-color={accountColor(account.id)}
                      ></span>
                      <span class="block truncate text-sm font-semibold"
                        >{account.displayName}</span
                      >
                    </span>
                    <span
                      class="block truncate text-xs text-ink-500 dark:text-slate-400"
                      >{account.email}</span
                    >
                  </span>
                </button>
                <button
                  class="hidden size-7 place-items-center rounded-md text-ink-400 hover:bg-red-50 hover:text-red-600 group-hover:grid dark:hover:bg-red-950/40"
                  title="Remove account"
                  type="button"
                  onclick={() => void removeAccount(account.id)}
                >
                  <Trash2 size={15} />
                </button>
              </div>

              {#if selectedAccountId === account.id}
                <div class="mt-1 space-y-1">
                  {#each folders as folder (folder.id)}
                    {@const FolderIcon = folderIcon(folder.id)}
                    <button
                      class={[
                        "flex h-9 w-full items-center justify-between rounded-md px-3 text-sm",
                        selectedFolderId === folder.id
                          ? "bg-signal-600 text-white"
                          : "text-ink-600 hover:bg-ink-100 dark:text-slate-300 dark:hover:bg-slate-800",
                      ]}
                      type="button"
                      onclick={() => void loadMessages(folder.id)}
                    >
                      <span class="flex items-center gap-2"
                        ><FolderIcon size={15} /> {folder.name}</span
                      >
                      {#if folder.unreadCount}<span
                          class="text-xs font-semibold"
                          >{folder.unreadCount}</span
                        >{/if}
                    </button>
                  {/each}
                </div>
              {/if}
            </section>
          {/each}
        </div>
      </div>

      <div class="border-t border-ink-200 p-3 dark:border-slate-800">
        <div class="flex items-center gap-2">
          <button
            class="flex h-10 min-w-0 flex-1 cursor-pointer items-center gap-2 rounded-md px-3 text-sm font-semibold text-ink-700 transition-colors duration-200 hover:bg-ink-100 focus:outline-none focus:ring-2 focus:ring-signal-500 dark:text-slate-200 dark:hover:bg-slate-800"
            type="button"
            onclick={() => openSettings("general")}
          >
            <Settings size={17} /> Settings
          </button>
          <button
            class="inline-flex h-10 shrink-0 cursor-pointer items-center gap-2 rounded-md border border-ink-200 bg-white px-3 text-sm font-semibold text-ink-700 transition-colors duration-200 hover:bg-ink-50 focus:outline-none focus:ring-2 focus:ring-signal-500 disabled:cursor-not-allowed disabled:opacity-60 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
            type="button"
            onclick={() => void syncAll()}
            disabled={appBusy}
          >
            <RefreshCw size={16} class={appBusy ? "animate-spin" : ""} />
            Sync
          </button>
        </div>
      </div>
    </aside>

    <button
      class="group relative cursor-col-resize border-r border-ink-200 bg-ink-100/70 focus:outline-none focus:ring-2 focus:ring-inset focus:ring-signal-500 dark:border-slate-800 dark:bg-slate-900"
      type="button"
      aria-label="Resize accounts panel. Drag or use left and right arrow keys."
      onpointerdown={(event) => startPanelResize("accounts", event)}
      onkeydown={(event) => handlePanelResizeKey("accounts", event)}
    >
      <span
        class="absolute inset-y-0 left-1/2 w-1 -translate-x-1/2 rounded-full bg-transparent transition-colors group-hover:bg-signal-500/70 group-focus:bg-signal-500/70"
      ></span>
    </button>

    <section
      class="flex min-w-0 flex-col border-r border-ink-200 bg-ink-50 dark:border-slate-800 dark:bg-slate-950"
    >
      <div class="border-b border-ink-200 p-4 dark:border-slate-800">
        <div class="mb-3 flex items-center justify-between gap-3">
          <div class="min-w-0">
            <h2 class="truncate text-lg font-semibold">
              {selectedFolder?.name ?? "Mailbox"}
            </h2>
            <p class="truncate text-xs text-ink-500 dark:text-slate-400">
              {selectedAccount?.email ?? "All mailboxes"} · {selectedAccount
                ? formatRelative(selectedAccount.lastSyncedAt)
                : `${accounts.length} accounts`}
            </p>
          </div>
          {#if theme === "dark"}<Moon
              class="text-slate-400"
              size={18}
            />{:else}<Sun class="text-amber-500" size={18} />{/if}
        </div>
        <form
          class="relative"
          onsubmit={(event) => {
            event.preventDefault();
            void searchMessages();
          }}
        >
          <Search class="absolute left-3 top-2.5 text-ink-400" size={16} />
          <input
            class="h-10 w-full rounded-md border-ink-200 bg-white pl-9 text-sm dark:border-slate-800 dark:bg-slate-900"
            placeholder="Search offline mail"
            bind:value={query}
          />
        </form>
        <div class="mt-3 flex items-center justify-between gap-2">
          <div class="flex min-w-0 items-center gap-2">
            <button
              class="inline-flex h-8 items-center rounded-md border border-ink-200 bg-white px-2.5 text-xs font-semibold text-ink-700 hover:bg-ink-50 disabled:opacity-50 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
              type="button"
              disabled={messages.length === 0}
              onclick={startSelection}
            >
              Select
            </button>
            {#if selectionMode}
              <button
                class="inline-flex h-8 items-center rounded-md border border-ink-200 bg-white px-2.5 text-xs font-semibold text-ink-700 hover:bg-ink-50 disabled:opacity-50 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
                type="button"
                disabled={messages.length === 0 || allVisibleSelected}
                onclick={selectAllVisibleMessages}
              >
                All
              </button>
              <span
                class="truncate text-xs font-medium text-ink-500 dark:text-slate-400"
                >{selectedMessageIds.length} selected</span
              >
            {/if}
          </div>
          <div class="flex shrink-0 items-center gap-2">
            {#if selectionMode}
              <button
                class="inline-flex h-8 items-center gap-1.5 rounded-md border border-ink-200 bg-white px-2.5 text-xs font-semibold text-ink-700 hover:bg-ink-50 disabled:opacity-50 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
                type="button"
                disabled={selectedMessageIds.length === 0}
                onclick={() => void markSelectedRead()}
              >
                <CheckCircle2 size={14} /> Read
              </button>
              <button
                class="inline-flex h-8 items-center gap-1.5 rounded-md border border-red-200 bg-white px-2.5 text-xs font-semibold text-red-700 hover:bg-red-50 disabled:opacity-50 dark:border-red-950 dark:bg-slate-900 dark:text-red-300 dark:hover:bg-red-950/40"
                type="button"
                disabled={selectedMessageIds.length === 0}
                onclick={() => void deleteSelectedMessages()}
              >
                <Trash2 size={14} /> {isPermanentDeleteFolder
                  ? "Delete forever"
                  : "Delete"}
              </button>
            {/if}
          </div>
        </div>
      </div>

      <div class="mail-scrollbar flex-1 overflow-y-auto">
        {#if messages.length === 0}
          <div
            class="grid h-full place-items-center p-8 text-center text-sm text-ink-500 dark:text-slate-400"
          >
            No cached messages in this folder.
          </div>
        {:else}
          {#each messages as message (message.id)}
            <div
              class={[
                "flex w-full gap-3 border-b border-ink-200 p-4 hover:bg-white dark:border-slate-800 dark:hover:bg-slate-900",
                selectedMessage?.id === message.id
                  ? "bg-white dark:bg-slate-900"
                  : "",
              ]}
            >
              <input
                class={[
                  "mt-1 size-4 shrink-0 rounded border-ink-300 text-signal-600 focus:ring-signal-500 dark:border-slate-700 dark:bg-slate-900",
                  selectionMode ? "" : "pointer-events-none opacity-0",
                ]}
                type="checkbox"
                aria-label={`Select ${message.subject}`}
                aria-hidden={!selectionMode}
                checked={selectedMessageIdSet.has(message.id)}
                disabled={!selectionMode}
                tabindex={selectionMode ? 0 : -1}
                onchange={() => toggleMessageSelection(message.id)}
              />
              <button
                class="min-w-0 flex-1 text-left"
                type="button"
                onclick={() => void openMessage(message.id)}
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
                          ? "font-semibold text-ink-950 dark:text-white"
                          : "font-medium text-ink-700 dark:text-slate-300",
                      ]}>{message.subject}</span
                    >
                  </span>
                  <span class="shrink-0 text-xs text-ink-400"
                    >{formatRelative(message.receivedAt)}</span
                  >
                </div>
                <div class="mt-1 flex items-center gap-2">
                  <span
                    class={[
                      "truncate text-xs text-ink-500 dark:text-slate-400",
                      message.isUnread ? "font-semibold" : "font-medium",
                    ]}>{message.sender}</span
                  >
                  {#if message.hasAttachments}<Paperclip
                      class="shrink-0 text-ink-400"
                      size={14}
                    />{/if}
                </div>
                <p
                  class="mt-1 line-clamp-2 text-sm leading-5 text-ink-500 dark:text-slate-400"
                >
                  {message.preview}
                </p>
              </button>
            </div>
          {/each}
        {/if}
      </div>
    </section>

    <button
      class="group relative cursor-col-resize border-r border-ink-200 bg-ink-100/70 focus:outline-none focus:ring-2 focus:ring-inset focus:ring-signal-500 dark:border-slate-800 dark:bg-slate-900"
      type="button"
      aria-label="Resize message list panel. Drag or use left and right arrow keys."
      onpointerdown={(event) => startPanelResize("message", event)}
      onkeydown={(event) => handlePanelResizeKey("message", event)}
    >
      <span
        class="absolute inset-y-0 left-1/2 w-1 -translate-x-1/2 rounded-full bg-transparent transition-colors group-hover:bg-signal-500/70 group-focus:bg-signal-500/70"
      ></span>
    </button>

    <article
      class="mail-scrollbar min-w-0 overflow-y-auto bg-white dark:bg-slate-900"
    >
      {#if selectedMessage}
        <header class="border-b border-ink-200 p-6 dark:border-slate-800">
          <div class="mb-4 flex flex-wrap gap-2">
            <span
              class="inline-flex items-center gap-1 rounded-md bg-emerald-50 px-2 py-1 text-xs font-medium text-emerald-700 dark:bg-emerald-950/40 dark:text-emerald-200"
              ><ShieldCheck size={14} /> Read-only</span
            >
            <span
              class="inline-flex items-center gap-1 rounded-md bg-blue-50 px-2 py-1 text-xs font-medium text-blue-700 dark:bg-blue-950/40 dark:text-blue-200"
              ><CloudOff size={14} /> Offline cache</span
            >
          </div>
          <h2 class="text-2xl font-semibold leading-tight tracking-normal">
            {selectedMessage.subject}
          </h2>
          <div class="mt-4 grid gap-1 text-sm text-ink-500 dark:text-slate-400">
            <div>
              <span class="font-medium text-ink-700 dark:text-slate-200"
                >From:</span
              >
              {selectedMessage.sender}
            </div>
            <div>
              <span class="font-medium text-ink-700 dark:text-slate-200"
                >To:</span
              >
              {selectedMessage.recipients.join(", ")}
            </div>
            <div>
              <span class="font-medium text-ink-700 dark:text-slate-200"
                >Received:</span
              >
              {new Date(selectedMessage.receivedAt).toLocaleString()}
            </div>
          </div>
        </header>

        <div class="prose prose-ink max-w-none p-6 dark:prose-invert">
          {@html selectedMessage.bodyHtml}
        </div>

        {#if selectedMessage.attachments.length}
          <section class="px-6 pb-6">
            <h3 class="mb-3 text-sm font-semibold">Cached attachments</h3>
            <div class="grid gap-2">
              {#each selectedMessage.attachments as attachment (attachment.id)}
                <div
                  class="flex items-center justify-between rounded-lg border border-ink-200 px-3 py-2 dark:border-slate-800"
                >
                  <div class="flex min-w-0 items-center gap-3">
                    <Paperclip class="shrink-0 text-ink-400" size={17} />
                    <div class="min-w-0">
                      <div class="truncate text-sm font-medium">
                        {attachment.fileName}
                      </div>
                      <div class="text-xs text-ink-500 dark:text-slate-400">
                        {attachment.mimeType} · {formatBytes(
                          attachment.byteSize,
                        )}
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
            <p class="mt-1 text-sm text-ink-500 dark:text-slate-400">
              Synced messages are stored locally for offline reading.
            </p>
          </div>
        </div>
      {/if}
    </article>

    {#if settingsOpen}
      <div
        class="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/35 p-4 backdrop-blur-sm"
        role="presentation"
        onclick={(event) => {
          if (event.target === event.currentTarget) settingsOpen = false;
        }}
      >
        <div
          class="flex max-h-[88vh] w-full max-w-4xl overflow-hidden rounded-lg border border-ink-200 bg-white shadow-soft dark:border-slate-800 dark:bg-slate-900"
          role="dialog"
          aria-modal="true"
          aria-labelledby="settings-title"
        >
          <div
            class="w-56 shrink-0 border-r border-ink-200 bg-ink-50 p-3 dark:border-slate-800 dark:bg-slate-950"
          >
            <div class="mb-3 flex items-center justify-between px-2">
              <div>
                <h2 id="settings-title" class="text-sm font-semibold">
                  Settings
                </h2>
                <p class="mt-0.5 text-xs text-ink-500 dark:text-slate-400">
                  Mail preferences
                </p>
              </div>
              <button
                class="grid size-8 cursor-pointer place-items-center rounded-md text-ink-500 transition-colors duration-200 hover:bg-ink-100 hover:text-ink-950 focus:outline-none focus:ring-2 focus:ring-signal-500 dark:text-slate-400 dark:hover:bg-slate-800 dark:hover:text-white"
                type="button"
                aria-label="Close settings"
                onclick={() => (settingsOpen = false)}
              >
                <X size={17} />
              </button>
            </div>

            <div class="space-y-1" role="tablist" aria-label="Settings tabs">
              {#each [
                { id: "general", label: "General", icon: Monitor },
                { id: "accounts", label: "Accounts", icon: UserPlus },
                { id: "advanced", label: "Advanced", icon: ShieldAlert },
              ] as tab (tab.id)}
                {@const TabIcon = tab.icon}
                <button
                  class={[
                    "flex h-10 w-full cursor-pointer items-center gap-2 rounded-md px-3 text-sm font-medium transition-colors duration-200 focus:outline-none focus:ring-2 focus:ring-signal-500",
                    settingsTab === tab.id
                      ? "bg-signal-600 text-white"
                      : "text-ink-600 hover:bg-white hover:text-ink-950 dark:text-slate-300 dark:hover:bg-slate-900 dark:hover:text-white",
                  ]}
                  type="button"
                  role="tab"
                  aria-selected={settingsTab === tab.id}
                  onclick={() =>
                    (settingsTab = tab.id as
                      | "general"
                      | "accounts"
                      | "advanced")}
                >
                  <TabIcon size={16} /> {tab.label}
                </button>
              {/each}
            </div>
          </div>

          <div class="mail-scrollbar min-w-0 flex-1 overflow-y-auto p-6">
            {#if settingsTab === "general"}
              <section class="space-y-6">
                <div>
                  <h3 class="text-lg font-semibold">General</h3>
                  <p class="mt-1 text-sm text-ink-500 dark:text-slate-400">
                    Set the app appearance and session state.
                  </p>
                </div>

                <div>
                  <div class="mb-2 text-sm font-semibold">Appearance</div>
                  <div
                    class="grid grid-cols-3 gap-2 rounded-lg border border-ink-200 bg-ink-50 p-1 dark:border-slate-800 dark:bg-slate-950"
                  >
                    {#each themeOptions as option (option.value)}
                      <button
                        class={[
                          "inline-flex h-10 cursor-pointer items-center justify-center gap-2 rounded-md text-sm font-semibold transition-colors duration-200 focus:outline-none focus:ring-2 focus:ring-signal-500",
                          theme === option.value
                            ? "bg-white text-ink-950 shadow-sm dark:bg-slate-800 dark:text-white"
                            : "text-ink-600 hover:bg-white/70 dark:text-slate-300 dark:hover:bg-slate-900",
                        ]}
                        type="button"
                        onclick={() => void changeTheme(option.value)}
                      >
                        {#if option.value === "dark"}<Moon size={16} />{:else if option.value === "light"}<Sun size={16} />{:else}<Monitor size={16} />{/if}
                        {option.label}
                      </button>
                    {/each}
                  </div>
                </div>

                <div
                  class="rounded-lg border border-ink-200 p-4 dark:border-slate-800"
                >
                  <div class="flex items-center justify-between gap-4">
                    <div>
                      <div class="text-sm font-semibold">Session</div>
                      <p class="mt-1 text-sm text-ink-500 dark:text-slate-400">
                        Log out of the current local mail session.
                      </p>
                    </div>
                    <button
                      class="inline-flex h-10 cursor-pointer items-center gap-2 rounded-md border border-ink-200 bg-white px-3 text-sm font-semibold text-ink-700 transition-colors duration-200 hover:bg-ink-50 focus:outline-none focus:ring-2 focus:ring-signal-500 dark:border-slate-700 dark:bg-slate-950 dark:text-slate-200 dark:hover:bg-slate-800"
                      type="button"
                      onclick={logout}
                    >
                      <LogOut size={16} /> Log out
                    </button>
                  </div>
                </div>
              </section>
            {:else if settingsTab === "accounts"}
              <section class="space-y-6">
                <div>
                  <h3 class="text-lg font-semibold">Accounts</h3>
                  <p class="mt-1 text-sm text-ink-500 dark:text-slate-400">
                    Add mailboxes, remove mailboxes, and tune account colors.
                  </p>
                </div>

                <div
                  class="rounded-lg border border-ink-200 bg-ink-50 p-4 dark:border-slate-800 dark:bg-slate-950"
                >
                  <div class="text-sm font-semibold">Add account</div>
                  <div class="mt-3 grid gap-2 sm:grid-cols-2">
                    <button
                      class="h-10 cursor-pointer rounded-md border border-ink-200 bg-white px-3 text-sm font-semibold text-ink-700 transition-colors duration-200 hover:bg-ink-50 focus:outline-none focus:ring-2 focus:ring-signal-500 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
                      type="button"
                      onclick={() => void connectProvider("gmail")}>Gmail</button
                    >
                    <button
                      class="h-10 cursor-pointer rounded-md border border-ink-200 bg-white px-3 text-sm font-semibold text-ink-700 transition-colors duration-200 hover:bg-ink-50 focus:outline-none focus:ring-2 focus:ring-signal-500 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
                      type="button"
                      onclick={() => void connectProvider("outlook")}>Outlook</button
                    >
                  </div>
                  <div class="mt-3 grid gap-2 md:grid-cols-[1fr_1fr_auto]">
                    <label class="sr-only" for="settings-icloud-email"
                      >iCloud email</label
                    >
                    <input
                      id="settings-icloud-email"
                      class="h-10 rounded-md border-ink-200 text-sm focus:border-signal-500 focus:ring-signal-500 dark:border-slate-700 dark:bg-slate-900"
                      type="email"
                      placeholder="name@icloud.com"
                      bind:value={icloudEmail}
                    />
                    <label class="sr-only" for="settings-icloud-password"
                      >iCloud app-specific password</label
                    >
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
                      onclick={() => void connectIcloud()}
                    >
                      <Apple size={16} /> Connect
                    </button>
                  </div>
                </div>

                <div class="space-y-3">
                  {#each accounts as account (account.id)}
                    <div
                      class="rounded-lg border border-ink-200 p-4 dark:border-slate-800"
                    >
                      <div class="flex items-start justify-between gap-4">
                        <div class="min-w-0">
                          <div class="flex min-w-0 items-center gap-2">
                            <span
                              class="size-3 shrink-0 rounded-full"
                              style:background-color={accountColor(account.id)}
                            ></span>
                            <div class="truncate text-sm font-semibold">
                              {account.displayName}
                            </div>
                          </div>
                          <div
                            class="mt-1 truncate text-sm text-ink-500 dark:text-slate-400"
                          >
                            {account.email}
                          </div>
                        </div>
                        <button
                          class="inline-flex h-9 cursor-pointer items-center gap-2 rounded-md border border-red-200 bg-white px-3 text-sm font-semibold text-red-700 transition-colors duration-200 hover:bg-red-50 focus:outline-none focus:ring-2 focus:ring-red-500 dark:border-red-950 dark:bg-slate-950 dark:text-red-300 dark:hover:bg-red-950/40"
                          type="button"
                          onclick={() => void removeAccount(account.id)}
                        >
                          <Trash2 size={15} /> Remove
                        </button>
                      </div>
                      <div class="mt-4 flex flex-wrap items-center gap-2">
                        <span
                          class="inline-flex items-center gap-1 text-xs font-semibold uppercase tracking-wide text-ink-500 dark:text-slate-400"
                          ><Palette size={14} /> Color</span
                        >
                        {#each accountColors as color (color)}
                          <button
                            class={[
                              "size-7 cursor-pointer rounded-full border-2 transition-colors duration-200 focus:outline-none focus:ring-2 focus:ring-signal-500",
                              accountColor(account.id) === color
                                ? "border-ink-900 dark:border-white"
                                : "border-transparent hover:border-ink-300 dark:hover:border-slate-600",
                            ]}
                            style:background-color={color}
                            type="button"
                            aria-label={`Set ${account.displayName} color to ${color}`}
                            onclick={() => updateAccountColor(account.id, color)}
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
                  <p class="mt-1 text-sm text-ink-500 dark:text-slate-400">
                    Destructive account controls for this device.
                  </p>
                </div>
                <div
                  class="rounded-lg border border-red-200 bg-red-50 p-4 dark:border-red-950 dark:bg-red-950/30"
                >
                  <div class="flex items-start justify-between gap-4">
                    <div>
                      <div
                        class="flex items-center gap-2 text-sm font-semibold text-red-900 dark:text-red-100"
                      >
                        <ShieldAlert size={17} /> Delete account
                      </div>
                      <p class="mt-1 text-sm text-red-700 dark:text-red-200">
                        Clears the local Shitou Mail session and removes the
                        connected mailboxes from this device.
                      </p>
                    </div>
                    <button
                      class="inline-flex h-10 shrink-0 cursor-pointer items-center gap-2 rounded-md bg-red-600 px-3 text-sm font-semibold text-white transition-colors duration-200 hover:bg-red-500 focus:outline-none focus:ring-2 focus:ring-red-500"
                      type="button"
                      onclick={deleteUserAccount}
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
  </main>
{/if}

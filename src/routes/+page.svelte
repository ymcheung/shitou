<script lang="ts">
  import AccountSidebar from "../accounts/AccountSidebar.svelte";
  import AuthScreen from "../auth/AuthScreen.svelte";
  import MessageList from "../messages/MessageList.svelte";
  import MessageReader from "../messages/MessageReader.svelte";
  import SettingsDialog from "../settings/SettingsDialog.svelte";
  import { Settings } from "@lucide/svelte";
  import {
    accountColor as resolveAccountColor,
    accountLabel as resolveAccountLabel,
  } from "../accounts/account-colors";
  import { providerLabels } from "../accounts/provider";
  import { applyTheme } from "../app/theme";
  import { api, demoApi } from "$lib/tauri";
  import {
    buildRootFolders,
    isPermanentDeleteFolder as folderAllowsPermanentDelete,
  } from "../mailbox/folder-model";
  import {
    minMessagePanelWidth,
    panelHandleWidth,
    resizeAccountDivider as calculateAccountDividerResize,
    resizeMessageDivider as calculateMessageDividerResize,
    type ResizeState,
    type ResizeTarget,
  } from "../layout/resizable-panels";
  import type {
    Folder,
    MailAccount,
    MessageDetail,
    MessageSummary,
    Provider,
    ThemeMode,
    AuthSession,
  } from "$lib/types";

  let email = $state("");
  let otp = $state("");
  let otpSent = $state(false);
  let authBusy = $state(false);
  let authAction = $state<"idle" | "sendingOtp" | "verifyingOtp" | "demo">(
    "idle",
  );
  let authError = $state("");
  let authReady = $state(false);
  let isSignedIn = $state(false);
  let isDemoMode = $state(false);
  let session = $state.raw<AuthSession | null>(null);

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
  let rootFolders = $derived(buildRootFolders(allFolders));
  let selectedFolder = $derived(
    [...rootFolders, ...folders].find(
      (folder) => folder.id === selectedFolderId,
    ) ?? null,
  );
  let isPermanentDeleteFolder = $derived(
    folderAllowsPermanentDelete(selectedFolder),
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
  let mailApi = $derived(isDemoMode ? demoApi : api);

  $effect(() => {
    applyTheme(theme);
  });

  $effect(() => {
    void restoreSession();
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

  async function restoreSession() {
    try {
      const restoredSession = await api.authCurrentSession();
      if (restoredSession?.authenticated) {
        session = restoredSession;
        isDemoMode = false;
        isSignedIn = true;
        await loadMailbox();
      }
    } catch (error) {
      authError =
        error instanceof Error
          ? error.message
          : "Unable to restore the local session.";
    } finally {
      authReady = true;
    }
  }

  async function sendOtp() {
    authBusy = true;
    authAction = "sendingOtp";
    authError = "";

    try {
      await api.authSendEmailOtp(email);
      otpSent = true;
      otp = "";
    } catch (error) {
      authError =
        error instanceof Error
          ? error.message
          : "Unable to send one-time code.";
    } finally {
      authBusy = false;
      authAction = "idle";
    }
  }

  async function verifyOtpSignIn() {
    authBusy = true;
    authAction = "verifyingOtp";
    authError = "";

    try {
      session = await api.authVerifyEmailOtp(email, otp);
      isDemoMode = false;
      isSignedIn = true;
      await loadMailbox();
    } catch (error) {
      authError =
        error instanceof Error ? error.message : "Unable to complete sign-in.";
    } finally {
      authBusy = false;
      authAction = "idle";
    }
  }

  async function startDemoMode() {
    authBusy = true;
    authAction = "demo";
    authError = "";

    try {
      session = await demoApi.authCompleteDemo();
      isDemoMode = true;
      isSignedIn = true;
      accountPanelOpen = false;
      await loadMailbox();
    } catch (error) {
      authError =
        error instanceof Error ? error.message : "Unable to start demo mode.";
    } finally {
      authBusy = false;
      authAction = "idle";
    }
  }

  async function loadMailbox() {
    appBusy = true;
    appError = "";

    try {
      accounts = await mailApi.listAccounts();
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
          [account.id, await mailApi.listFolders(account.id)] as const,
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
    folders =
      foldersByAccount[accountId] ?? (await mailApi.listFolders(accountId));
    foldersByAccount = { ...foldersByAccount, [accountId]: folders };
    selectedFolderId = folders[0]?.id || "";
    selectedMessage = null;
    if (selectedFolderId) await loadMessages(selectedFolderId);
  }

  async function loadMessages(folderId: string) {
    selectedFolderId = folderId;
    messages = await mailApi.listMessages(folderId, query);
    selectedMessageIds = [];
    selectionMode = false;
    selectedMessage = messages[0]
      ? await mailApi.getMessage(messages[0].id)
      : null;
  }

  async function searchMessages() {
    if (!selectedFolderId) return;
    messages = await mailApi.listMessages(selectedFolderId, query);
    selectedMessageIds = [];
    selectionMode = false;
    selectedMessage = messages[0]
      ? await mailApi.getMessage(messages[0].id)
      : null;
  }

  async function openMessage(messageId: string) {
    const message = messages.find((item) => item.id === messageId);
    if (message?.isUnread) {
      await mailApi.markMessagesRead([messageId]);
      messages = messages.map((item) =>
        item.id === messageId ? { ...item, isUnread: false } : item,
      );
      await refreshFolders();
    }
    const detail = await mailApi.getMessage(messageId);
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
    await mailApi.markMessagesRead(selectedMessageIds);
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
    await mailApi.deleteMessages(selectedMessageIds);
    selectedMessageIds = [];
    await refreshFolders();
    if (selectedFolderId) await loadMessages(selectedFolderId);
  }

  async function syncAll() {
    appBusy = true;
    appError = "";

    try {
      accounts = await mailApi.syncAll();
      await refreshFolders();
      if (selectedFolderId) await loadMessages(selectedFolderId);
    } catch (error) {
      appError = error instanceof Error ? error.message : "Sync failed.";
    } finally {
      appBusy = false;
    }
  }

  async function connectProvider(provider: Exclude<Provider, "icloud">) {
    if (isDemoMode) {
      appError = "Adding accounts is unavailable in demo mode.";
      return;
    }

    appBusy = true;
    appError = "";

    try {
      await mailApi.connectProvider(provider);
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
    if (isDemoMode) {
      appError = "Adding accounts is unavailable in demo mode.";
      return;
    }

    appBusy = true;
    appError = "";

    try {
      const account = await mailApi.connectIcloud(icloudEmail, icloudPassword);
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
    await mailApi.removeAccount(accountId);
    accounts = accounts.filter((account) => account.id !== accountId);
    const { [accountId]: _removed, ...remainingColors } = accountColorOverrides;
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

  async function logout() {
    if (!window.confirm("Log out of Shitou Mail?")) return;
    await api.authLogout();
    isSignedIn = false;
    isDemoMode = false;
    session = null;
    selectedMessage = null;
    selectedMessageIds = [];
    selectionMode = false;
    settingsOpen = false;
  }

  function updateAccountColor(accountId: string, color: string) {
    accountColorOverrides = { ...accountColorOverrides, [accountId]: color };
  }

  async function deleteUserAccount() {
    if (
      !window.confirm(
        "Delete this Shitou Mail account from this device? Local demo session data will be cleared.",
      )
    ) {
      return;
    }
    await api.authLogout();
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
    isDemoMode = false;
    session = null;
  }

  function accountColor(accountId: string) {
    return resolveAccountColor(accountId, accounts, accountColorOverrides);
  }

  function accountLabel(accountId: string) {
    return resolveAccountLabel(accountId, accounts);
  }

  function availablePanelWidth() {
    return (
      (mailShell?.getBoundingClientRect().width ??
        document.documentElement.clientWidth) -
      panelHandleWidth * 2
    );
  }

  function resizeAccountDivider(nextAccountWidth: number) {
    const resized = calculateAccountDividerResize(
      nextAccountWidth,
      accountPanelWidth,
      messageListWidth,
    );
    accountPanelWidth = resized.accountPanelWidth;
    messageListWidth = resized.messageListWidth;
  }

  function resizeMessageDivider(nextMessageListWidth: number) {
    messageListWidth = calculateMessageDividerResize(
      nextMessageListWidth,
      accountPanelWidth,
      availablePanelWidth(),
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
      const resized = calculateAccountDividerResize(
        activeResize.startAccountWidth + delta,
        activeResize.startAccountWidth,
        activeResize.startMessageListWidth,
      );
      accountPanelWidth = resized.accountPanelWidth;
      messageListWidth = resized.messageListWidth;
    } else {
      messageListWidth = calculateMessageDividerResize(
        activeResize.startMessageListWidth + delta,
        activeResize.startAccountWidth,
        availablePanelWidth(),
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

{#if !authReady}
  <div
    class="flex h-screen items-center justify-center bg-zinc-100 text-sm text-zinc-600 dark:bg-zinc-950 dark:text-zinc-300"
  >
    Opening Shitou Mail...
  </div>
{:else if !isSignedIn}
  <AuthScreen
    bind:email
    bind:otp
    {otpSent}
    busy={authBusy}
    {authAction}
    error={authError}
    onSendOtp={sendOtp}
    onVerifyOtp={verifyOtpSignIn}
    onStartDemo={startDemoMode}
  />
{:else}
  <main
    class={[
      "relative h-screen overflow-hidden bg-zinc-100 text-zinc-900 dark:bg-zinc-950 dark:text-zinc-100",
      activeResize ? "cursor-col-resize select-none" : "",
    ]}
  >
    <button
      class="absolute right-4 top-2 z-20 inline-flex h-9 cursor-pointer items-center gap-2 rounded-md px-3 text-sm font-medium text-zinc-950 hover:bg-zinc-200 focus:outline-none focus:ring-2 focus:ring-zinc-500 dark:text-zinc-100 dark:hover:bg-zinc-900"
      type="button"
      onclick={() => openSettings("general")}
    >
      <Settings size={14} />
      Settings
    </button>

    <div
      bind:this={mailShell}
      class="absolute inset-x-0 bottom-0 top-[52px] grid min-h-0 overflow-hidden"
      style:grid-template-columns={mailGridColumns}
    >
      <AccountSidebar
        {unreadTotal}
        {offlineAccounts}
        {appError}
        {appBusy}
        {rootFolders}
        {folders}
        {accounts}
        {selectedFolderId}
        {selectedAccountId}
        {accountColor}
        onLoadRootFolder={loadRootFolder}
        onLoadFolders={loadFolders}
        onLoadMessages={loadMessages}
        onRemoveAccount={removeAccount}
        onSyncAll={syncAll}
      />

      <button
        class="group relative cursor-col-resize bg-gradient-to-b from-zinc-200/0 to-zinc-200 focus:outline-none focus:ring-2 focus:ring-inset focus:ring-zinc-500 dark:from-zinc-800/0 dark:to-zinc-800"
        type="button"
        aria-label="Resize accounts panel. Drag or use left and right arrow keys."
        onpointerdown={(event) => startPanelResize("accounts", event)}
        onkeydown={(event) => handlePanelResizeKey("accounts", event)}
      >
        <span
          class="absolute inset-y-0 left-1/2 w-1 -translate-x-1/2 rounded-full bg-gradient-to-b from-zinc-500/0 to-zinc-500/0 transition-colors group-hover:to-zinc-500 group-focus:to-zinc-500"
        ></span>
      </button>

      <MessageList
        bind:query
        {selectedFolder}
        {selectedAccount}
        accountsCount={accounts.length}
        {messages}
        {selectedMessage}
        {selectedMessageIds}
        {selectionMode}
        {allVisibleSelected}
        {isPermanentDeleteFolder}
        {accountColor}
        {accountLabel}
        onSearch={searchMessages}
        onStartSelection={startSelection}
        onSelectAllVisible={selectAllVisibleMessages}
        onMarkSelectedRead={markSelectedRead}
        onDeleteSelected={deleteSelectedMessages}
        onToggleMessageSelection={toggleMessageSelection}
        onOpenMessage={openMessage}
      />

      <button
        class="group relative cursor-col-resize bg-zinc-200 focus:outline-none focus:ring-2 focus:ring-inset focus:ring-zinc-500 dark:bg-zinc-800"
        type="button"
        aria-label="Resize message list panel. Drag or use left and right arrow keys."
        onpointerdown={(event) => startPanelResize("message", event)}
        onkeydown={(event) => handlePanelResizeKey("message", event)}
      >
        <span
          class="absolute inset-y-0 left-1/2 w-1 -translate-x-1/2 rounded-full bg-transparent transition-colors group-hover:bg-zinc-500 group-focus:bg-zinc-500"
        ></span>
      </button>

      <MessageReader message={selectedMessage} />
    </div>

    <SettingsDialog
      bind:open={settingsOpen}
      bind:tab={settingsTab}
      bind:icloudEmail
      bind:icloudPassword
      {theme}
      {accounts}
      canAddAccounts={!isDemoMode}
      {isDemoMode}
      {accountColor}
      onChangeTheme={changeTheme}
      onLogout={logout}
      onConnectProvider={connectProvider}
      onConnectIcloud={connectIcloud}
      onRemoveAccount={removeAccount}
      onUpdateAccountColor={updateAccountColor}
      onDeleteUserAccount={deleteUserAccount}
    />
  </main>
{/if}

<script lang="ts">
  import AccountSidebar from "../accounts/AccountSidebar.svelte";
  import MessageList from "../messages/MessageList.svelte";
  import MessageReader from "../messages/MessageReader.svelte";
  import NotificationListView from "../notifications/NotificationList.svelte";
  import SettingsDialog from "../settings/SettingsDialog.svelte";
  import { onMount } from "svelte";
  import { RefreshCw, Search } from "@lucide/svelte";
  import {
    accountColor as resolveAccountColor,
    accountLabel as resolveAccountLabel,
  } from "../accounts/account-colors";
  import { providerLabels } from "../accounts/provider";
  import { applyTheme } from "../app/theme";
  import { settingsClient, type MailboxClient } from "$lib/tauri";
  import {
    buildRootFolders,
    isPermanentDeleteFolder as folderAllowsPermanentDelete,
  } from "./folder-model";
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
    NotificationList,
    NotificationSummary,
    Provider,
    ThemeMode,
  } from "$lib/types";

  let {
    client,
    isDemoMode,
    onLogout,
  }: {
    client: MailboxClient;
    isDemoMode: boolean;
    onLogout: () => void | Promise<void>;
  } = $props();

  let accounts = $state.raw<MailAccount[]>([]);
  let folders = $state.raw<Folder[]>([]);
  let foldersByAccount = $state.raw<Record<string, Folder[]>>({});
  let messages = $state.raw<MessageSummary[]>([]);
  let notificationState = $state.raw<NotificationList>({
    items: [],
    unseenCount: 0,
    nextExpiryAt: null,
    enabled: true,
  });
  let notificationAccountFilter = $state("");
  let selectedNotificationId = $state("");
  let selectedMessageIds = $state.raw<string[]>([]);
  let selectedAccountId = $state("");
  let selectedFolderId = $state("");
  let selectedMessage = $state.raw<MessageDetail | null>(null);
  let query = $state("");
  let appBusy = $state(false);
  let appError = $state("");
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
  let notificationRefreshInFlight = false;
  let notificationExpiryTimer: number | undefined;

  let selectedAccount = $derived(
    accounts.find((account) => account.id === selectedAccountId) ?? null,
  );
  let allFolders = $derived(Object.values(foldersByAccount).flat());
  let rootFolders = $derived(
    buildRootFolders(allFolders, notificationState.unseenCount),
  );
  let isNotificationsFolder = $derived(
    selectedFolderId === "root:notifications",
  );
  let visibleNotifications = $derived(
    notificationAccountFilter
      ? notificationState.items.filter(
          (notification) =>
            notification.accountId === notificationAccountFilter,
        )
      : notificationState.items,
  );
  let selectedNotification = $derived(
    notificationState.items.find(
      (notification) => notification.id === selectedNotificationId,
    ) ?? null,
  );
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
  let allVisibleSelected = $derived(
    messages.length > 0 && selectedMessageIds.length === messages.length,
  );
  let mailGridColumns = $derived(
    `${accountPanelWidth}px ${panelHandleWidth}px ${messageListWidth}px ${panelHandleWidth}px minmax(${minMessagePanelWidth}px, 1fr)`,
  );
  onMount(() => {
    applyTheme(theme);
    void loadMailbox().then(() =>
      refreshNotifications({ summarizeSecurity: true, syncMail: true }),
    );
    const notificationInterval = window.setInterval(
      () =>
        void refreshNotifications({ summarizeSecurity: true, syncMail: true }),
      3 * 60 * 60 * 1_000,
    );

    let unlisten: (() => void) | undefined;
    if ("__TAURI_INTERNALS__" in window) {
      void import("@tauri-apps/api/event").then(({ listen }) => {
        void listen<"general" | "accounts" | "advanced">(
          "open-settings",
          (event) => openSettings(event.payload ?? "general"),
        ).then((nextUnlisten) => {
          unlisten = nextUnlisten;
        });
      });
    }

    return () => {
      unlisten?.();
      window.clearInterval(notificationInterval);
      if (notificationExpiryTimer !== undefined) {
        window.clearTimeout(notificationExpiryTimer);
      }
    };
  });

  async function loadMailbox() {
    appBusy = true;
    appError = "";

    try {
      accounts = await client.listAccounts();
      notificationState = await client.listNotifications();
      scheduleNotificationExpiry();
      foldersByAccount = await loadFoldersByAccount(accounts);
      folders = selectedAccountId
        ? (foldersByAccount[selectedAccountId] ?? [])
        : [];
      if (!selectedFolderId) {
        await loadRootFolder("root:inbox");
      } else if (selectedFolderId === "root:notifications") {
        await loadNotifications();
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
          [account.id, await client.listFolders(account.id)] as const,
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
    if (folderId === "root:notifications") {
      selectedFolderId = folderId;
      messages = [];
      selectedMessageIds = [];
      selectedMessage = null;
      selectedNotificationId = "";
      await loadNotifications();
      return;
    }
    selectedNotificationId = "";
    await loadMessages(folderId);
  }

  function scheduleNotificationExpiry() {
    if (notificationExpiryTimer !== undefined) {
      window.clearTimeout(notificationExpiryTimer);
      notificationExpiryTimer = undefined;
    }
    if (!notificationState.enabled || !notificationState.nextExpiryAt) return;
    const delay = Date.parse(notificationState.nextExpiryAt) - Date.now();
    notificationExpiryTimer = window.setTimeout(
      () => void refreshNotifications({}),
      Math.max(0, Math.min(delay, 2_147_483_647)),
    );
  }

  async function loadNotifications(markVisibleSeen = true) {
    notificationState = await client.listNotifications();
    if (markVisibleSeen) await markVisibleNotificationsSeen();
    scheduleNotificationExpiry();
  }

  async function markVisibleNotificationsSeen() {
    const visible = notificationAccountFilter
      ? notificationState.items.filter(
          (notification) =>
            notification.accountId === notificationAccountFilter,
        )
      : notificationState.items;
    const unseenIds = visible
      .filter((notification) => !notification.isSeen)
      .map((notification) => notification.id);
    if (!unseenIds.length) return;
    await client.markNotificationsSeen(unseenIds);
    const seen = new Set(unseenIds);
    notificationState = {
      ...notificationState,
      items: notificationState.items.map((notification) =>
        seen.has(notification.id)
          ? { ...notification, isSeen: true }
          : notification,
      ),
      unseenCount: Math.max(
        0,
        notificationState.unseenCount - unseenIds.length,
      ),
    };
  }

  async function changeNotificationAccountFilter(accountId: string) {
    notificationAccountFilter = accountId;
    selectedNotificationId = "";
    selectedMessage = null;
    await markVisibleNotificationsSeen();
  }

  async function openNotification(notification: NotificationSummary) {
    selectedNotificationId = notification.id;
    selectedMessage = await client.getMessage(notification.messageId);
  }

  async function dismissNotification(notificationId: string) {
    await client.dismissNotification(notificationId);
    if (selectedNotificationId === notificationId) {
      selectedNotificationId = "";
      selectedMessage = null;
    }
    await loadNotifications(false);
  }

  async function restoreNotification(notificationId: string) {
    await client.restoreNotification(notificationId);
    if (selectedNotificationId === notificationId) {
      selectedNotificationId = "";
      selectedMessage = null;
    }
    await Promise.all([refreshFolders(), loadNotifications(false)]);
  }

  async function refreshNotifications({
    summarizeSecurity = false,
    syncMail = false,
    reportErrors = false,
  }: {
    summarizeSecurity?: boolean;
    syncMail?: boolean;
    reportErrors?: boolean;
  }) {
    if (notificationRefreshInFlight) return;
    notificationRefreshInFlight = true;
    try {
      notificationState = await client.processNotifications(summarizeSecurity);
      if (syncMail) {
        try {
          accounts = await client.syncAll();
        } catch (error) {
          if (reportErrors) {
            appError = error instanceof Error ? error.message : "Sync failed.";
          }
        }
        await refreshFolders();
        notificationState =
          await client.processNotifications(summarizeSecurity);
      }
      if (isNotificationsFolder) {
        await markVisibleNotificationsSeen();
      } else if (selectedFolderId) {
        await loadMessages(selectedFolderId);
      }
      scheduleNotificationExpiry();
    } catch (error) {
      if (reportErrors) {
        appError =
          error instanceof Error
            ? error.message
            : "Unable to refresh notifications.";
      }
    } finally {
      notificationRefreshInFlight = false;
    }
  }

  async function loadFolders(accountId: string) {
    selectedAccountId = accountId;
    folders =
      foldersByAccount[accountId] ?? (await client.listFolders(accountId));
    foldersByAccount = { ...foldersByAccount, [accountId]: folders };
    selectedFolderId = folders[0]?.id || "";
    selectedNotificationId = "";
    selectedMessage = null;
    if (selectedFolderId) await loadMessages(selectedFolderId);
  }

  async function loadMessages(folderId: string, preferredMessageId?: string) {
    selectedFolderId = folderId;
    messages = await client.listMessages(folderId, query);
    selectedMessageIds = [];
    selectionMode = false;
    const nextMessage =
      messages.find((message) => message.id === preferredMessageId) ??
      messages[0];
    selectedMessage = nextMessage
      ? await client.getMessage(nextMessage.id)
      : null;
  }

  async function searchMessages() {
    if (!selectedFolderId || isNotificationsFolder) return;
    messages = await client.listMessages(selectedFolderId, query);
    selectedMessageIds = [];
    selectionMode = false;
    selectedMessage = messages[0]
      ? await client.getMessage(messages[0].id)
      : null;
  }

  async function openMessage(messageId: string) {
    const message = messages.find((item) => item.id === messageId);
    if (message?.isUnread) {
      await client.markMessagesRead([messageId]);
      messages = messages.map((item) =>
        item.id === messageId ? { ...item, isUnread: false } : item,
      );
      await refreshFolders();
    }
    const detail = await client.getMessage(messageId);
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
    await client.markMessagesRead(selectedMessageIds);
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

  async function markMessageRead(messageIds: string[]) {
    if (messageIds.length === 0) return;
    const messageIdSet = new Set(messageIds);
    await client.markMessagesRead(messageIds);
    messages = messages.map((message) =>
      messageIdSet.has(message.id) ? { ...message, isUnread: false } : message,
    );
    if (selectedMessage && messageIdSet.has(selectedMessage.id)) {
      selectedMessage = { ...selectedMessage, isUnread: false };
    }
    await refreshFolders();
  }

  async function markMessageUnread(messageIds: string[]) {
    if (messageIds.length === 0) return;
    const messageIdSet = new Set(messageIds);
    await client.markMessagesUnread(messageIds);
    messages = messages.map((message) =>
      messageIdSet.has(message.id) ? { ...message, isUnread: true } : message,
    );
    if (selectedMessage && messageIdSet.has(selectedMessage.id)) {
      selectedMessage = { ...selectedMessage, isUnread: true };
    }
    await refreshFolders();
  }

  function selectMessageAfterRemoval(messageIds: string[]) {
    const removedMessageIds = new Set(messageIds);
    if (selectedMessage && !removedMessageIds.has(selectedMessage.id)) {
      return selectedMessage.id;
    }

    const anchorMessageId =
      selectedMessage && removedMessageIds.has(selectedMessage.id)
        ? selectedMessage.id
        : messageIds[0];
    const anchorIndex = messages.findIndex(
      (message) => message.id === anchorMessageId,
    );
    if (anchorIndex === -1) return undefined;

    for (let index = anchorIndex + 1; index < messages.length; index += 1) {
      if (!removedMessageIds.has(messages[index].id)) {
        return messages[index].id;
      }
    }

    for (let index = anchorIndex - 1; index >= 0; index -= 1) {
      if (!removedMessageIds.has(messages[index].id)) {
        return messages[index].id;
      }
    }

    return undefined;
  }

  async function deleteSelectedMessages() {
    const messageIds =
      selectedMessageIds.length > 0
        ? selectedMessageIds
        : selectedMessage
          ? [selectedMessage.id]
          : [];
    if (messageIds.length === 0) return;
    const nextMessageId = selectMessageAfterRemoval(messageIds);
    if (
      isPermanentDeleteFolder &&
      !window.confirm(
        messageIds.length === 1
          ? "Permanently delete this mail?"
          : "Permanently delete the selected mail?",
      )
    ) {
      return;
    }
    appError = "";
    try {
      await client.deleteMessages(messageIds);
      selectedMessageIds = [];
      await refreshFolders();
      if (selectedFolderId) await loadMessages(selectedFolderId, nextMessageId);
    } catch (error) {
      appError = error instanceof Error ? error.message : "Delete failed.";
    }
  }

  async function moveMessageToTrash(messageIds: string[]) {
    if (messageIds.length === 0) return;
    const messageIdSet = new Set(messageIds);
    const nextMessageId = selectMessageAfterRemoval(messageIds);
    await client.deleteMessages(messageIds);
    selectedMessageIds = selectedMessageIds.filter(
      (id) => !messageIdSet.has(id),
    );
    await refreshFolders();
    if (selectedFolderId) await loadMessages(selectedFolderId, nextMessageId);
  }

  async function markMessageSpam(messageIds: string[]) {
    if (messageIds.length === 0) return;
    const messageIdSet = new Set(messageIds);
    const nextMessageId = selectMessageAfterRemoval(messageIds);
    await client.markMessagesSpam(messageIds);
    selectedMessageIds = selectedMessageIds.filter(
      (id) => !messageIdSet.has(id),
    );
    await refreshFolders();
    if (selectedFolderId) await loadMessages(selectedFolderId, nextMessageId);
  }

  function isTextEditingTarget(target: EventTarget | null) {
    if (!(target instanceof HTMLElement)) return false;
    const tagName = target.tagName.toLowerCase();
    return (
      target.isContentEditable ||
      tagName === "input" ||
      tagName === "textarea" ||
      tagName === "select"
    );
  }

  function handleGlobalKeydown(event: KeyboardEvent) {
    if (event.key !== "Delete" && event.key !== "Backspace") return;
    if (settingsOpen || activeResize) return;
    if (isNotificationsFolder) return;
    if (event.metaKey || event.ctrlKey || event.altKey) return;
    if (isTextEditingTarget(event.target)) return;
    if (selectedMessageIds.length === 0 && !selectedMessage) return;

    event.preventDefault();
    void deleteSelectedMessages();
  }

  async function syncAll() {
    appBusy = true;
    appError = "";

    try {
      await refreshNotifications({ syncMail: true, reportErrors: true });
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
      const account = await client.connectProvider(provider);
      accounts = [
        ...accounts.filter((existing) => existing.id !== account.id),
        account,
      ];
      foldersByAccount = { ...foldersByAccount, [account.id]: [] };
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
    if (isDemoMode) return false;

    appBusy = true;
    appError = "";

    try {
      const account = await client.connectIcloud(icloudEmail, icloudPassword);
      accounts = [
        ...accounts.filter((existing) => existing.id !== account.id),
        account,
      ];
      foldersByAccount = { ...foldersByAccount, [account.id]: [] };
      icloudEmail = "";
      icloudPassword = "";
      try {
        const syncedAccount = await client.syncAccount(account.id);
        accounts = accounts.map((existing) =>
          existing.id === syncedAccount.id ? syncedAccount : existing,
        );
        await refreshFolders();
        await refreshNotifications({});
      } catch (syncError) {
        appError = `iCloud connected. ${
          syncError instanceof Error
            ? syncError.message
            : "Mail sync will be available shortly."
        }`;
      }
      return true;
    } catch (error) {
      appError =
        error instanceof Error ? error.message : "Unable to connect iCloud.";
      return false;
    } finally {
      appBusy = false;
    }
  }

  async function removeAccount(accountId: string) {
    appError = "";
    try {
      await client.removeAccount(accountId);
      accounts = accounts.filter((account) => account.id !== accountId);
      await loadNotifications(false);
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
    } catch (error) {
      appError =
        error instanceof Error ? error.message : "Unable to remove account.";
    }
  }

  async function changeTheme(nextTheme: ThemeMode) {
    theme = nextTheme;
    applyTheme(nextTheme);
    await settingsClient.setTheme(nextTheme);
  }

  async function changeNotificationsEnabled(enabled: boolean) {
    const setting = await client.setNotificationsEnabled(enabled);
    notificationState = {
      ...(await client.listNotifications()),
      enabled: setting.enabled,
    };
    await refreshFolders();
    if (!isNotificationsFolder && selectedFolderId) {
      await loadMessages(selectedFolderId);
    }
    scheduleNotificationExpiry();
  }

  function openSettings(tab: "general" | "accounts" | "advanced" = "general") {
    settingsTab = tab;
    settingsOpen = true;
  }

  async function logout() {
    await onLogout();
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
    await onLogout();
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
  onkeydown={handleGlobalKeydown}
/>

<main
  class={[
    "relative h-screen overflow-hidden bg-zinc-100 text-zinc-900 dark:bg-zinc-950 dark:text-zinc-100",
    activeResize ? "cursor-col-resize select-none" : "",
  ]}
>
  <header
    class="absolute inset-x-0 top-0 z-20 flex h-[52px] items-center justify-between border-b border-zinc-200/80 px-4 dark:border-zinc-900"
  >
    <div class="flex min-w-0 items-center gap-3">
      <img class="size-9 shrink-0" src="/app-icon.png" alt="" />
      <div class="min-w-0">
        <div class="truncate text-sm font-semibold">Shitou Mail</div>
        <div class="truncate text-xs text-zinc-500 dark:text-zinc-400">
          {unreadTotal} unread
        </div>
      </div>
    </div>

    <div class="ml-4 flex min-w-0 flex-1 items-center justify-end gap-2">
      {#if !isNotificationsFolder}
        <form
          class="relative min-w-[180px] max-w-80 flex-1"
          onsubmit={(event) => {
            event.preventDefault();
            void searchMessages();
          }}
        >
          <label class="sr-only" for="mail-header-search"
            >Search offline mail</label
          >
          <Search
            class="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-zinc-400 dark:text-zinc-500"
            size={15}
          />
          <input
            id="mail-header-search"
            class="h-9 w-full rounded-lg border border-zinc-300 bg-white pl-9 pr-3 text-sm font-medium text-zinc-900 shadow-sm outline-none placeholder:text-zinc-400 hover:border-zinc-400 focus:border-sky-600 focus:ring-2 focus:ring-sky-600/20 dark:border-zinc-700 dark:bg-zinc-900 dark:text-zinc-100 dark:placeholder:text-zinc-500 dark:hover:border-zinc-600 dark:focus:border-sky-400 dark:focus:ring-sky-400/20"
            placeholder="Search offline mail"
            bind:value={query}
          />
        </form>
      {/if}

      <button
        class="inline-flex h-9 shrink-0 cursor-pointer items-center gap-2 rounded-lg border border-sky-700 bg-sky-700 px-3 text-sm font-semibold text-white shadow-sm shadow-sky-950/10 hover:border-sky-800 hover:bg-sky-800 focus:outline-none focus:ring-2 focus:ring-sky-600/40 disabled:cursor-not-allowed disabled:opacity-60 dark:border-sky-500/70 dark:bg-sky-500 dark:text-zinc-950 dark:shadow-black/20 dark:hover:border-sky-400 dark:hover:bg-sky-400 dark:focus:ring-sky-400/40"
        type="button"
        onclick={() => void syncAll()}
        disabled={appBusy}
      >
        <RefreshCw size={14} class={appBusy ? "animate-spin" : ""} />
        Sync
      </button>
    </div>
  </header>

  <div
    bind:this={mailShell}
    class="absolute inset-x-0 bottom-0 top-[52px] grid min-h-0 overflow-hidden"
    style:grid-template-columns={mailGridColumns}
  >
    <AccountSidebar
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
      onOpenSettings={() => openSettings("general")}
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

    {#if isNotificationsFolder}
      <NotificationListView
        notifications={visibleNotifications}
        {accounts}
        {selectedNotificationId}
        accountFilter={notificationAccountFilter}
        {accountColor}
        onAccountFilter={changeNotificationAccountFilter}
        onOpen={openNotification}
        onDismiss={dismissNotification}
        onRestore={restoreNotification}
      />
    {:else}
      <MessageList
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
        onStartSelection={startSelection}
        onSelectAllVisible={selectAllVisibleMessages}
        onMarkSelectedRead={markSelectedRead}
        onDeleteSelected={deleteSelectedMessages}
        onMarkMessageRead={markMessageRead}
        onMarkMessageUnread={markMessageUnread}
        onMoveMessageToTrash={moveMessageToTrash}
        onMarkMessageSpam={markMessageSpam}
        onToggleMessageSelection={toggleMessageSelection}
        onOpenMessage={openMessage}
      />
    {/if}

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

    <MessageReader
      message={selectedMessage}
      {isPermanentDeleteFolder}
      canDelete={!isNotificationsFolder}
      expired={isNotificationsFolder &&
        selectedNotification?.status === "expired"}
      onDeleteMessage={deleteSelectedMessages}
    />
  </div>

  <SettingsDialog
    bind:open={settingsOpen}
    bind:tab={settingsTab}
    bind:icloudEmail
    bind:icloudPassword
    {theme}
    {accounts}
    notificationsEnabled={notificationState.enabled}
    {appBusy}
    canAddAccounts={!isDemoMode}
    {isDemoMode}
    {accountColor}
    onChangeTheme={changeTheme}
    onChangeNotificationsEnabled={changeNotificationsEnabled}
    onLogout={logout}
    onConnectProvider={connectProvider}
    onConnectIcloud={connectIcloud}
    onRemoveAccount={removeAccount}
    onUpdateAccountColor={updateAccountColor}
    onDeleteUserAccount={deleteUserAccount}
  />
</main>

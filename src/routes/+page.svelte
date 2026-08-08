<script lang="ts">
  import { onMount } from "svelte";
  import AuthScreen from "../auth/AuthScreen.svelte";
  import MailboxScreen from "../mailbox/MailboxScreen.svelte";
  import {
    authClient,
    demoMailboxClient,
    desktopMailboxClient,
  } from "$lib/tauri";

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

  onMount(() => {
    void restoreSession();
  });

  async function restoreSession() {
    try {
      if ((await authClient.currentSession())?.authenticated) {
        isSignedIn = true;
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
      await authClient.sendEmailOtp(email);
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
      await authClient.verifyEmailOtp(email, otp);
      isDemoMode = false;
      isSignedIn = true;
    } catch (error) {
      authError =
        error instanceof Error ? error.message : "Unable to complete sign-in.";
    } finally {
      authBusy = false;
      authAction = "idle";
    }
  }

  function startDemoMode() {
    authBusy = true;
    authAction = "demo";
    authError = "";
    isDemoMode = true;
    isSignedIn = true;
    authBusy = false;
    authAction = "idle";
  }

  async function logout() {
    if (!isDemoMode) await authClient.logout();
    isSignedIn = false;
    isDemoMode = false;
    otpSent = false;
    otp = "";
  }
</script>

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
  <MailboxScreen
    client={isDemoMode ? demoMailboxClient : desktopMailboxClient}
    {isDemoMode}
    onLogout={logout}
  />
{/if}

<script lang="ts">
  import * as Avatar from "$shared/ui/avatar/index.js";
  import { getSenderAvatarColor, getSenderInitials } from "./sender-avatar";

  let {
    sender,
    avatarUrl,
    size = "md",
  }: {
    sender: string;
    avatarUrl?: string | null;
    size?: "sm" | "md" | "lg";
  } = $props();

  const sizeClasses = {
    sm: "size-9 text-xs",
    md: "size-11 text-sm",
    lg: "size-14 text-base",
  };

  let initials = $derived(getSenderInitials(sender));
  let colorClass = $derived(getSenderAvatarColor(sender));
</script>

<Avatar.Root
  class={["font-semibold ring-1", sizeClasses[size], colorClass]}
  aria-label={`${sender} avatar`}
>
  {#if avatarUrl}
    <Avatar.Image src={avatarUrl} alt="" loading="lazy" />
  {/if}
  <Avatar.Fallback>{initials}</Avatar.Fallback>
</Avatar.Root>

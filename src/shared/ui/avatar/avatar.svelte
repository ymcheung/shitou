<script lang="ts">
  import { Avatar as AvatarPrimitive } from "bits-ui";
  import { cn, type WithoutChildrenOrChild } from "$shared/utils.js";
  import type { Snippet } from "svelte";

  let {
    ref = $bindable(null),
    class: className,
    children,
    ...restProps
  }: WithoutChildrenOrChild<AvatarPrimitive.RootProps> & {
    ref?: HTMLElement | null;
    children?: Snippet;
  } = $props();
</script>

<AvatarPrimitive.Root
  bind:ref
  data-slot="avatar"
  class={cn(
    "relative flex size-8 shrink-0 overflow-hidden rounded-full",
    className,
  )}
  {...restProps}
>
  {@render children?.()}
</AvatarPrimitive.Root>

<script lang="ts">
  import { Avatar as AvatarPrimitive } from "bits-ui";
  import { cn, type WithoutChildrenOrChild } from "$shared/utils.js";
  import type { Snippet } from "svelte";

  let {
    ref = $bindable(null),
    class: className,
    children,
    ...restProps
  }: WithoutChildrenOrChild<AvatarPrimitive.FallbackProps> & {
    ref?: HTMLSpanElement | null;
    children?: Snippet;
  } = $props();
</script>

<AvatarPrimitive.Fallback
  bind:ref
  data-slot="avatar-fallback"
  class={cn(
    "flex size-full items-center justify-center rounded-full bg-muted",
    className,
  )}
  {...restProps}
>
  {@render children?.()}
</AvatarPrimitive.Fallback>

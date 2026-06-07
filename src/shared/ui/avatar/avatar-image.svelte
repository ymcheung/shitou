<script lang="ts">
  import { Avatar as AvatarPrimitive } from "bits-ui";
  import { cn, type WithoutChildrenOrChild } from "$shared/utils.js";
  import type { Snippet } from "svelte";

  let {
    ref = $bindable(null),
    class: className,
    children,
    ...restProps
  }: WithoutChildrenOrChild<AvatarPrimitive.ImageProps> & {
    ref?: HTMLImageElement | null;
    children?: Snippet;
  } = $props();
</script>

<AvatarPrimitive.Image
  bind:ref
  data-slot="avatar-image"
  class={cn("aspect-square size-full object-cover", className)}
  {...restProps}
>
  {@render children?.()}
</AvatarPrimitive.Image>

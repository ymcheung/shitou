<script lang="ts">
  import { ContextMenu as ContextMenuPrimitive } from "bits-ui";
  import type { Snippet } from "svelte";
  import type { WithoutChildrenOrChild } from "$shared/utils.js";

  let {
    ref = $bindable(null),
    children,
    ...restProps
  }: WithoutChildrenOrChild<ContextMenuPrimitive.TriggerProps> & {
    children?: Snippet;
  } = $props();
</script>

<ContextMenuPrimitive.Trigger
  bind:ref
  data-slot="context-menu-trigger"
  {...restProps}
>
  {@render children?.()}
</ContextMenuPrimitive.Trigger>

<script lang="ts">
  import { ContextMenu as ContextMenuPrimitive } from "bits-ui";

  let { children, ...restProps }: ContextMenuPrimitive.RootProps = $props();
</script>

<ContextMenuPrimitive.Root {...restProps}>
  {@render children?.()}
</ContextMenuPrimitive.Root>

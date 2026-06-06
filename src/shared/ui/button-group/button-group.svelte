<script lang="ts" module>
  import { cn, type WithElementRef } from "$shared/utils.js";
  import type { HTMLAttributes } from "svelte/elements";

  export type ButtonGroupProps = WithElementRef<
    HTMLAttributes<HTMLDivElement>
  > & {
    orientation?: "horizontal" | "vertical";
  };
</script>

<script lang="ts">
  let {
    ref = $bindable(null),
    class: className,
    orientation = "horizontal",
    children,
    ...restProps
  }: ButtonGroupProps = $props();
</script>

<div
  bind:this={ref}
  data-slot="button-group"
  data-orientation={orientation}
  role="group"
  class={cn(
    "group/button-group flex w-fit items-center rounded-md shadow-xs data-[orientation=vertical]:flex-col [&>[data-slot=button]]:rounded-none [&>[data-slot=button]]:shadow-none [&>[data-slot=button]:first-child]:rounded-s-md [&>[data-slot=button]:last-child]:rounded-e-md data-[orientation=vertical]:[&>[data-slot=button]:first-child]:rounded-t-md data-[orientation=vertical]:[&>[data-slot=button]:first-child]:rounded-b-none data-[orientation=vertical]:[&>[data-slot=button]:last-child]:rounded-b-md data-[orientation=vertical]:[&>[data-slot=button]:last-child]:rounded-t-none [&>[data-slot=button]:not(:first-child)]:-ms-px data-[orientation=vertical]:[&>[data-slot=button]:not(:first-child)]:-mt-px data-[orientation=vertical]:[&>[data-slot=button]:not(:first-child)]:ms-0",
    className,
  )}
  {...restProps}
>
  {@render children?.()}
</div>

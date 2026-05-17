export const panelHandleWidth = 6;
export const minAccountPanelWidth = 220;
export const maxAccountPanelWidth = 420;
export const minMessageListWidth = 300;
export const maxMessageListWidth = 640;
export const minMessagePanelWidth = 360;

export type ResizeTarget = 'accounts' | 'message';
export type ResizeState = {
  target: ResizeTarget;
  startX: number;
  startAccountWidth: number;
  startMessageListWidth: number;
};

export function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

export function resizeAccountDivider(nextAccountWidth: number, accountPanelWidth: number, messageListWidth: number) {
  const totalLeftWidth = accountPanelWidth + messageListWidth;
  const nextAccountPanelWidth = clamp(
    nextAccountWidth,
    minAccountPanelWidth,
    Math.min(maxAccountPanelWidth, totalLeftWidth - minMessageListWidth)
  );
  return {
    accountPanelWidth: nextAccountPanelWidth,
    messageListWidth: totalLeftWidth - nextAccountPanelWidth
  };
}

export function resizeMessageDivider(nextMessageListWidth: number, accountPanelWidth: number, availablePanelWidth: number) {
  const maxWidth = Math.min(maxMessageListWidth, availablePanelWidth - accountPanelWidth - minMessagePanelWidth);
  return clamp(nextMessageListWidth, minMessageListWidth, Math.max(minMessageListWidth, maxWidth));
}

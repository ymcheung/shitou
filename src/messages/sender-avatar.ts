const avatarColors = [
  "bg-teal-100 text-teal-800 ring-teal-200 dark:bg-teal-950 dark:text-teal-100 dark:ring-teal-800",
  "bg-sky-100 text-sky-800 ring-sky-200 dark:bg-sky-950 dark:text-sky-100 dark:ring-sky-800",
  "bg-violet-100 text-violet-800 ring-violet-200 dark:bg-violet-950 dark:text-violet-100 dark:ring-violet-800",
  "bg-amber-100 text-amber-800 ring-amber-200 dark:bg-amber-950 dark:text-amber-100 dark:ring-amber-800",
  "bg-rose-100 text-rose-800 ring-rose-200 dark:bg-rose-950 dark:text-rose-100 dark:ring-rose-800",
  "bg-emerald-100 text-emerald-800 ring-emerald-200 dark:bg-emerald-950 dark:text-emerald-100 dark:ring-emerald-800",
];

export function getSenderInitials(sender: string) {
  const words = sender
    .replace(/<[^>]+>/g, "")
    .replace(/["']/g, "")
    .split(/[\s._@-]+/)
    .filter(Boolean);

  if (words.length === 0) return "?";

  const initials =
    words.length === 1
      ? words[0].slice(0, 2)
      : `${words[0][0]}${words[words.length - 1][0]}`;

  return initials.toUpperCase();
}

export function getSenderAvatarColor(sender: string) {
  const hash = Array.from(sender).reduce(
    (total, character) => total + character.charCodeAt(0),
    0,
  );

  return avatarColors[hash % avatarColors.length];
}

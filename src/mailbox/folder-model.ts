import type { Folder } from '../shared/mail.types';

type RootFolderDefinition = {
  id: string;
  name: string;
  names: string[];
};

export const rootFolderDefinitions: RootFolderDefinition[] = [
  { id: 'root:inbox', name: 'Inbox', names: ['inbox'] },
  { id: 'root:trash', name: 'Trash', names: ['trash'] },
  { id: 'root:spam', name: 'Spam', names: ['spam', 'junk'] }
] as const;

export function buildRootFolders(folders: Folder[]): Folder[] {
  return rootFolderDefinitions.map((definition) => ({
    id: definition.id,
    accountId: 'root',
    name: definition.name,
    unreadCount: folders
      .filter((folder) => definition.names.includes(folder.name.toLowerCase()))
      .reduce((total, folder) => total + folder.unreadCount, 0)
  }));
}

export function isPermanentDeleteFolder(folder: Folder | null) {
  return folder ? ['trash', 'spam', 'junk'].includes(folder.name.toLowerCase()) : false;
}

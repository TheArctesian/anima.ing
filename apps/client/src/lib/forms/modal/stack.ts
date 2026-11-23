/** The dialogs that are open, oldest first. Only the last one answers keys and focus outside every dialog. */
const openDialogs: object[] = [];

export interface OpenDialog {
  isTopmost(): boolean;
  close(): void;
}

export function openDialog(): OpenDialog {
  const entry = {};
  openDialogs.push(entry);
  return {
    isTopmost: () => openDialogs.at(-1) === entry,
    close: () => {
      const at = openDialogs.indexOf(entry);
      if (at !== -1) openDialogs.splice(at, 1);
    },
  };
}

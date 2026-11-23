const MAC = /Macintosh|Mac OS X/;
/** iPhone and iPod user agents say "like Mac OS X" too; they are not Macs. */
const IOS = /iPhone|iPod|iPad/;

export function isMac(userAgent: string): boolean {
  return MAC.test(userAgent) && !IOS.test(userAgent);
}

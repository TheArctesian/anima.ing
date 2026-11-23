import { checkDocuments, type Document } from "../../src/documents/documents.ts";

export function check(documents: Document[], known: string[] = []): string[] {
  const paths = new Set(documents.map((document) => document.path));
  const exists = (path: string) => paths.has(path) || [...paths].some((other) => other.startsWith(`${path}/`));
  return checkDocuments(documents, exists, new Set(known)).map((v) => `${v.path}: ${v.signal} ${v.detail}`);
}

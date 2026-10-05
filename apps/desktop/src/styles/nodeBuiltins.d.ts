// Narrow stand-ins so the contrast test can read globals.css.
// The desktop package does not depend on @types/node.
declare module "node:fs" {
  export function readFileSync(path: string, encoding: "utf8"): string;
}
declare module "node:path" {
  export function dirname(path: string): string;
  export function join(...paths: string[]): string;
}
declare module "node:url" {
  export function fileURLToPath(url: string | URL): string;
}

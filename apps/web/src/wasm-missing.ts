// Stands in for the generated WASM glue when `tools/build-wasm.sh` has not been run (the alias `@meldframe/wasm` in
// vite.config.ts points here then), so that the web build and the type check do not need the wasm toolchain.
const MESSAGE = 'The WASM bridge is not built. Run `just wasm-build` (it needs wasm-bindgen-cli, see AGENTS.md), then reload.';

export default async function init(): Promise<never> {
  throw new Error(MESSAGE);
}
export function initSync(): never {
  throw new Error(MESSAGE);
}
export function displayText(): never {
  throw new Error(MESSAGE);
}
export class WasmApp {
  constructor() {
    throw new Error(MESSAGE);
  }
}

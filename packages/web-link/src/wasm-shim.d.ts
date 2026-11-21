// Types of the generated WASM glue. `tools/build-wasm.sh` writes the glue to apps/web/.wasm (git-ignored), which
// the Vite alias `@meldframe/wasm` of apps/web points at; this declaration lets the type check run without a wasm build.
// Keep it in step with crates/app/meldframe-wasm (the generated meldframe_wasm.d.ts is the source of truth).
declare module '@meldframe/wasm' {
  export class WasmApp {
    constructor(actor: string, keyPrefix: string, session: number, nodesJson: string, seq: number, variablesJson?: string | null);
    dispatch(actionJson: string): string;
    stateJson(): string;
    nodesJson(): string;
    variablesJson(): string;
    sceneJson(): string;
    takeEventsJson(): string;
    setLayoutOnCanvas(on: boolean): string;
    setMeasurer(measure: (text: string, fontSize: number, fontWeight: number, wrapWidth: number | null) => [number, number]): string;
    free(): void;
  }
  export function initSync(module: { module: BufferSource } | BufferSource): unknown;
  export default function init(input?: unknown): Promise<unknown>;
}

import { loadWasmModule } from "./helpers";

const wasmModule = await loadWasmModule();

if (!["debug", "release"].includes(wasmModule.wasm_build_profile())) {
  throw new Error("Unexpected build profile marker");
}

const code = wasmModule.airac_code_from_date("2025-08-15") as string;
if (code.length !== 4) {
  throw new Error("airac_code_from_date returned an invalid code");
}

const effective = wasmModule.effective_date_from_airac_code(code) as string;
const interval = wasmModule.airac_interval(code) as { begin: string; end: string };

if (interval.begin !== effective) {
  throw new Error("airac interval begin mismatch");
}

const beginDt = new Date(`${interval.begin}T00:00:00Z`);
const endDt = new Date(`${interval.end}T00:00:00Z`);
const days = (endDt.getTime() - beginDt.getTime()) / (24 * 3600 * 1000);
if (days !== 28) {
  throw new Error("airac interval duration is not 28 days");
}

const runResult = wasmModule.run();
if (runResult !== undefined) {
  throw new Error("run() should return void on success");
}

console.log("airac.spec.ts passed");

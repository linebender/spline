import init, { wasm_main } from "./pkg/splinetoy.js";

async function run() {
  await init();
  wasm_main();
}

run();
